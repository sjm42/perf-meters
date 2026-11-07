// perf_meters.rs

use console::{Key, Term};
use serialport::{DataBits, FlowControl, Parity, StopBits};

use perf_meters::*;

const BAUD_RATE: u32 = 115200;
// Preserve the old 102400 sectors/s scale, using 512-byte Linux accounting sectors.
const DISK_FULL_SCALE_BPS: f64 = 50.0 * 1024.0 * 1024.0;

fn main() -> Result<()> {
    let opts = OptsCommon::parse();
    opts.validate()?;
    opts.start_pgm(env!("CARGO_BIN_NAME"))?;

    if opts.list_ports {
        warn!("Listing available serial ports");
        for p in serialport::available_ports().context("Unable to list serial ports")? {
            warn!("Port name: {:?}", &p.port_name);
            warn!("     type: {:?}", &p.port_type);
        }
        return Ok(());
    }

    let port = opts.port.as_deref().context("No port specified")?;
    info!("Opening serial port {port:?}");
    let mut serial = serialport::new(port, BAUD_RATE)
        .parity(Parity::None)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(time::Duration::from_secs(5))
        .open()
        .with_context(|| format!("Unable to open serial port {port:?}"))?;
    let mut vu = Vu::new(opts.pwm_max_delta)?;
    info!("Vu sez hi (:");
    hello(&mut vu, &mut serial)?;

    if opts.calibrate {
        return calibrate(&mut vu, &mut serial);
    }

    let mut mystats = MyStats::new()?;
    let n_cpu = mystats.n_cpu();
    {
        let sys = mystats.sys();
        info!("CPU core count used for weighting: {n_cpu}");
        sys.cpus()
            .iter()
            .enumerate()
            .for_each(|(i, c)| info!("    cpu#{i} {c:?}"));
        info!(
            "Mem: total {} KiB, used {} KiB, avail {} KiB",
            sys.total_memory() / 1024,
            sys.used_memory() / 1024,
            sys.available_memory() / 1024
        );
        for (name, data) in mystats.networks().iter() {
            info!("NET iface: {name:?}\n    {data:#?}");
        }
    }

    let period = opts.sample_period()?;
    let mut tick = time::Instant::now();
    info!("Starting measure loop at {} Hz", opts.samplerate);
    loop {
        // Subtract durations without truncation; an overrun starts the next sample
        // immediately, with no backlog of missed samples to replay.
        thread::sleep(period.saturating_sub(tick.elapsed()));
        tick = time::Instant::now();
        mystats.refresh()?;

        let cpu_rates = mystats.cpu_usage()?;
        let cpu_gauge = cpu_gauge(&cpu_rates, n_cpu)?;
        let cpu_pwm = pwm(cpu_gauge / 255.0, opts.cpu_pwm_min, opts.cpu_pwm_max)?;
        debug!("CPU gauge: {cpu_gauge:.1}, pwm: {cpu_pwm} -- {cpu_rates:.1?}");

        let net_rate = mystats.net_bits_per_second();
        let net_pwm = network_pwm(net_rate, &opts)?;
        debug!("NET rate: {:.1} kbps, pwm: {net_pwm}", net_rate / 1000.0);

        let disk_io = mystats.disk_io();
        let dsk_pwm = pwm(disk_io / DISK_FULL_SCALE_BPS, 0.0, 255.0)?;
        debug!("DSK rate: {:.1} MiB/s, pwm: {dsk_pwm}", disk_io / (1024.0 * 1024.0));

        let mem_pct = mystats.mem_usage()?;
        let mem_pwm = pwm(mem_pct / 100.0, opts.mem_pwm_min, opts.mem_pwm_max)?;
        debug!("MEM used: {mem_pct:.1}%, pwm: {mem_pwm}");

        vu.set(&mut serial, Channel::Ch0, cpu_pwm)?;
        vu.set(&mut serial, Channel::Ch1, net_pwm)?;
        vu.set(&mut serial, Channel::Ch2, dsk_pwm)?;
        vu.set(&mut serial, Channel::Ch3, mem_pwm)?;
    }
}

fn cpu_gauge(rates: &[f32], cores: usize) -> Result<f64> {
    ensure!(rates.iter().all(|value| value.is_finite()), "Non-finite CPU reading");
    // Keep the original physical-core weighting, but require the readings used by it.
    let gauge = match (cores, rates) {
        (6.., [a, b, c, d, e, f, ..]) => (a + b) / 2.0 + (c + d) / 2.0 + (e + f) / 3.0,
        (4..=5, [a, b, c, d, ..]) => (a + b) / 2.0 + (c + d) * 0.80,
        (2..=3, [a, b, ..]) => (a + b) / 2.0 * 2.56,
        (1, [a, ..]) => a * 2.56,
        _ => bail!("Insufficient CPU readings for {cores} cores: got {}", rates.len()),
    };
    ensure!(gauge.is_finite(), "Non-finite CPU gauge");
    Ok(f64::from(gauge.clamp(0.0, 255.0)))
}

fn pwm(fraction: f64, min: f32, max: f32) -> Result<i16> {
    ensure!(fraction.is_finite(), "Non-finite gauge reading");
    ensure!(
        min.is_finite() && max.is_finite() && (0.0..=255.0).contains(&min) && (0.0..=255.0).contains(&max),
        "Invalid PWM calibration"
    );
    let value = f64::from(min) + fraction.clamp(0.0, 1.0) * (f64::from(max) - f64::from(min));
    Ok(value.round() as i16)
}

fn network_pwm(rate: f64, opts: &OptsCommon) -> Result<i16> {
    let fraction = rate / (f64::from(opts.net_gauge_mbps) * 1_000_000.0);
    if opts.net_gauge_abs {
        pwm(fraction.abs(), opts.net_pwm_min, opts.net_pwm_max)
    } else if fraction < 0.0 {
        pwm(-fraction, opts.net_pwm_zero, opts.net_pwm_min)
    } else {
        pwm(fraction, opts.net_pwm_zero, opts.net_pwm_max)
    }
}

fn hello(vu: &mut Vu, ser: &mut Box<dyn SerialPort>) -> Result<()> {
    for i in (0i16..=255)
        .chain((128..=255).rev())
        .chain(128..=255)
        .chain((0..=255).rev())
    {
        for c in [Channel::Ch0, Channel::Ch1, Channel::Ch2] {
            vu.set(ser, c, i)?;
        }
        thread::sleep(time::Duration::new(0, 3_000_000));
    }
    Ok(())
}

fn calibrate(vu: &mut Vu, ser: &mut Box<dyn SerialPort>) -> Result<()> {
    let mut chan = Channel::Ch0;
    let mut gauges: [i16; N_CHANS] = [1; _];
    warn!("Entering calibration mode.\r\nUse arrow keys left/right to change channel.\r\nUse up/down to move gauge.");
    warn!("Press Esc to quit.");
    let term = Term::stdout();
    let mut stderr = io::stderr().lock();
    loop {
        write!(stderr, "\rChan: {chan:?} gauges: {gauges:?}")?;
        stderr.flush()?;
        let chan_i = chan as usize;
        vu.set(ser, chan, gauges[chan_i])?;

        let k = term.read_key()?;
        match k {
            Key::ArrowRight => {
                chan = chan.next();
            }
            Key::ArrowLeft => {
                chan = chan.prev();
            }
            Key::ArrowUp => {
                gauges[chan_i] += 1;
            }
            Key::ArrowDown => {
                gauges[chan_i] -= 1;
            }
            Key::Escape => {
                warn!("Exiting calibration mode.");
                return Ok(());
            }
            _ => {}
        }
        gauges[chan_i] = gauges[chan_i].clamp(1, 255);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_cpu_weighting_is_preserved() {
        let rates = [10.0, 8.0, 6.0, 4.0, 2.0, 1.0];
        for (cores, expected) in [(1, 25.6), (2, 23.04), (3, 23.04), (4, 17.0), (6, 15.0), (64, 15.0)] {
            assert!((cpu_gauge(&rates, cores).unwrap() - expected).abs() < 0.0001);
        }
        assert_eq!(cpu_gauge(&[100.0; 6], 6).unwrap(), 255.0);
    }

    #[test]
    fn missing_or_invalid_cpu_readings_return_errors() {
        for (cores, rates) in [
            (1, &[][..]),
            (2, &[10.0][..]),
            (4, &[10.0; 3][..]),
            (6, &[10.0; 5][..]),
            (0, &[10.0][..]),
        ] {
            assert!(cpu_gauge(rates, cores).is_err());
        }
        assert!(cpu_gauge(&[f32::NAN], 1).is_err());
        assert!(cpu_gauge(&[f32::INFINITY], 1).is_err());
    }

    #[test]
    fn pwm_reaches_endpoints_and_supports_inverted_gauges() {
        assert_eq!(pwm(0.0, 12.0, 220.0).unwrap(), 12);
        assert_eq!(pwm(1.0, 12.0, 220.0).unwrap(), 220);
        assert_eq!(pwm(10.0, 12.0, 220.0).unwrap(), 220);
        assert_eq!(pwm(-10.0, 12.0, 220.0).unwrap(), 12);
        assert_eq!(pwm(0.0, 220.0, 12.0).unwrap(), 220);
        assert_eq!(pwm(1.0, 220.0, 12.0).unwrap(), 12);
        assert_eq!(pwm(0.5, 0.0, 255.0).unwrap(), 128);
        assert!(pwm(f64::NAN, 0.0, 255.0).is_err());
        assert!(pwm(f64::INFINITY, 0.0, 255.0).is_err());
        assert!(pwm(0.0, f32::NAN, 255.0).is_err());
    }

    #[test]
    fn network_maps_zero_both_directions_and_absolute_mode() {
        let mut opts = OptsCommon::parse_from([
            "perf_meters",
            "--port",
            "unused",
            "--net-gauge-mbps=40",
            "--net-pwm-min=12",
            "--net-pwm-zero=113",
            "--net-pwm-max=220",
        ]);
        for (rate, expected) in [(0.0, 113), (-40e6, 12), (40e6, 220), (-80e6, 12), (80e6, 220)] {
            assert_eq!(network_pwm(rate, &opts).unwrap(), expected);
        }
        opts.net_pwm_min = 220.0;
        opts.net_pwm_max = 12.0;
        assert_eq!(network_pwm(-40e6, &opts).unwrap(), 220);
        assert_eq!(network_pwm(40e6, &opts).unwrap(), 12);
        opts.net_gauge_abs = true;
        assert_eq!(network_pwm(0.0, &opts).unwrap(), 220);
        assert_eq!(network_pwm(-40e6, &opts).unwrap(), 12);
        assert_eq!(network_pwm(40e6, &opts).unwrap(), 12);
        assert!(network_pwm(f64::NAN, &opts).is_err());
    }
}

// EOF
