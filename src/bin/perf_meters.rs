// bin/perf-meters.rs

use std::{thread, time};

use console::{Key, Term};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use perf_meters::*;

const BAUD_RATE: u32 = 115200;

fn main() -> anyhow::Result<()> {
    let opts = OptsCommon::parse();
    opts.start_pgm(env!("CARGO_BIN_NAME"));

    if opts.list_ports {
        warn!("Listing available serial ports");
        for p in serialport::available_ports()? {
            warn!("Port name: {}", &p.port_name);
            warn!("     type: {:?}", &p.port_type);
        }
        return Ok(());
    }

    let port = match opts.port {
        None => anyhow::bail!("No port specified"),
        Some(p) => p,
    };

    info!("Opening serial port {}", port);
    let mut serial = serialport::new(port, BAUD_RATE)
        .parity(Parity::None)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(time::Duration::new(5, 0))
        .open()?;
    let mut vu = Vu::new(opts.pwm_max_delta);
    info!("Vu sez hi (:");
    hello(&mut vu, &mut serial)?;

    if opts.calibrate {
        return calibrate(&mut vu, &mut serial);
    }

    let mut mystats = MyStats::new();
    let n_cpu = mystats.n_cpu();

    {
        let sys = mystats.sys();
        info!("CPU core count: {n_cpu}");
        sys.cpus()
            .iter()
            .enumerate()
            .for_each(|(i, c)| info!("    cpu#{i} {c:?}",));

        info!(
            "Mem: total {} KB, used {} KB, avail {} KB",
            sys.total_memory() / 1024,
            sys.used_memory() / 1024,
            sys.available_memory() / 1024
        );

        for (name, data) in mystats.networks().iter() {
            info!("NET iface: {name}\n    {data:#?}");
        }
    }

    let mut elapsed_ns = 0;
    let sleep_ns: u32 = (1_000_000_000.0 / opts.samplerate) as u32;
    debug!("Sleeping {} ms in each loop", sleep_ns / 1_000_000);

    info!("Starting measure loop");

    let cpu_pwm_min = opts.cpu_pwm_min;
    let cpu_pwm_range = opts.cpu_pwm_max - cpu_pwm_min;

    let net_pwm_min = opts.net_pwm_min;
    let net_pwm_zero = opts.net_pwm_zero;
    let net_pwm_max = opts.net_pwm_max;
    let net_pwm_frange = net_pwm_max - net_pwm_min;
    let net_pwm_nrange = net_pwm_zero - net_pwm_min;
    let net_pwm_prange = net_pwm_max - net_pwm_zero;

    let mem_pwm_min = opts.mem_pwm_min;
    let mem_pwm_range = opts.mem_pwm_max - mem_pwm_min;

    loop {
        thread::sleep(time::Duration::new(0, sleep_ns - elapsed_ns));
        let start = time::Instant::now();

        debug!("Last elapsed: {} µs", elapsed_ns / 1000);
        mystats.refresh();

        // CHAN0 - CPU stats + gauge, rates are sorted largest first
        let cpu_rates = mystats.cpu_usage();
        let mut cpu_gauge = if n_cpu >= 2 {
            (cpu_rates[0] + cpu_rates[1]) / 2.0
        } else {
            cpu_rates[0]
        };

        if n_cpu >= 6 {
            cpu_gauge += (cpu_rates[2] + cpu_rates[3]) / 2.0;
            cpu_gauge += (cpu_rates[4] + cpu_rates[5]) / 3.0;
        } else if n_cpu >= 4 {
            cpu_gauge += (cpu_rates[2] + cpu_rates[3]) * 0.80;
        } else {
            cpu_gauge *= 2.56;
        }
        // deliberately print out cpu gauge without clamping yet
        cpu_gauge = cpu_gauge.clamp(0.0, 255.0);
        let cpu_pwm = (cpu_pwm_min + (cpu_gauge * cpu_pwm_range / 256.0)).clamp(0.0, 255.0);
        debug!(
            "CPU gauge: {cpu_gauge:.1}, pwm: {cpu_pwm:.0} -- {list}",
            list = cpu_rates
                .iter()
                .take(4)
                .map(|a| format!("{a:.1}"))
                .collect::<Vec<String>>()
                .join(" ")
                .as_str()
        );

        // CHAN1 - NET stats + gauge
        let mut net_rate = mystats.net_bits();
        if opts.net_gauge_abs {
            net_rate = net_rate.abs();
        }
        let mut net_gauge = 256.0 * (((net_rate as f32) / 1_000_000.0) / opts.net_gauge_mbps);
        net_gauge = net_gauge.clamp(-255.0, 255.0);
        let net_pwm = if opts.net_gauge_abs {
            net_pwm_min + (net_gauge * net_pwm_frange / 256.0)
        } else {
            let range = if net_gauge < 0.0 {
                net_pwm_nrange
            } else {
                net_pwm_prange
            };
            net_pwm_zero + net_gauge * range / 256.0
        }
            .clamp(0.0, 255.0);
        debug!(
            "NET rate: {rate} kbps, gauge: {net_gauge:.0}, pwm: {net_pwm:.0}",
            rate = net_rate / 1000,
        );

        // CHAN2 - disk IO
        let disk_io = mystats.disk_io();
        let dsk_pwm = 256.0 * ((disk_io as f32) / 102_400.0).clamp(0.0, 255.0);
        debug!("DSK pwm: {dsk_pwm:.0}");

        // CHAN3 - MEM stats + gauge
        let mem_pct = mystats.mem_usage();
        let mut mem_gauge = 2.56 * mem_pct;
        mem_gauge = mem_gauge.clamp(0.0, 255.0);
        let mem_pwm = (mem_pwm_min + (mem_gauge * mem_pwm_range / 256.0)).clamp(0.0, 255.0);
        debug!("MEM used: {mem_pct:.1}%, gauge: {mem_gauge:.0}, pwm: {mem_pwm:.0}");

        vu.set(&mut serial, Channel::Ch0, cpu_pwm as i16)?;
        vu.set(&mut serial, Channel::Ch1, net_pwm as i16)?;
        vu.set(&mut serial, Channel::Ch2, dsk_pwm as i16)?;
        vu.set(&mut serial, Channel::Ch3, mem_pwm as i16)?;

        // keep the sample rate from drifting
        elapsed_ns = start.elapsed().as_nanos() as u32;
    }
}

fn hello(vu: &mut Vu, ser: &mut Box<dyn SerialPort>) -> anyhow::Result<()> {
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

fn calibrate(vu: &mut Vu, ser: &mut Box<dyn SerialPort>) -> anyhow::Result<()> {
    let mut chan = Channel::Ch0;
    let mut gauges: [i16; N_CHANS] = [1; _];
    warn!("Entering calibration mode.\r\nUse arrow keys left/right to change channel.\r\nUse up/down to move gauge.");
    warn!("Press Esc to quit.");
    let term = Term::stdout();
    loop {
        eprint!("\rChan: {chan:?} gauges: {gauges:?}");
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

// EOF
