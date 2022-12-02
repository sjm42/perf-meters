// bin/perf-vumeter.rs

// #![allow(unreachable_code)]
// #![allow(dead_code)]

use anyhow::bail;
use log::*;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{io::Write, thread, time};
use structopt::StructOpt;
use sysinfo::*;

use perf_meters::*;

const BAUD_RATE: u32 = 115200;

fn main() -> anyhow::Result<()> {
    let opts = OptsCommon::from_args();
    opts.start_pgm(env!("CARGO_BIN_NAME"));

    if opts.list_ports {
        warn!("Listing available serial ports");
        for p in serialport::available_ports()? {
            warn!("Port name: {}", &p.port_name);
            warn!("     type: {:?}", &p.port_type);
        }
        return Ok(());
    }

    let mut mystats = MyStats::new();
    let n_cpu = mystats.n_cpu();

    {
        let sys = mystats.sys();
        info!("CPU core count: {}", sys.physical_core_count().unwrap_or(0));
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

        for (name, data) in sys.networks().iter() {
            info!("NET iface: {name}\n    {data:#?}");
        }

        for comp in sys.components() {
            info!("Component: {comp:#?}");
        }

        for disk in sys.disks() {
            info!("disk: {disk:#?}");
        }
    }

    info!("Opening serial port {}", &opts.port);
    let mut ser = serialport::new(opts.port, BAUD_RATE)
        .parity(Parity::None)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(time::Duration::new(5, 0))
        .open()?;

    info!("Vu sez hi (:");
    hello(&mut ser)?;

    let mut elapsed_ns = 0;
    let sleep_ns: u32 = (1_000_000_000.0 / opts.samplerate) as u32;
    debug!("Sleeping {} ms in each loop", sleep_ns / 1_000_000);

    info!("Starting measure loop");
    loop {
        thread::sleep(time::Duration::new(0, sleep_ns - elapsed_ns));
        let start = time::Instant::now();

        debug!("Last elapsed: {} µs", elapsed_ns / 1000);
        mystats.refresh();

        // CPU stats + gauge, rates are sorted largest first
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
        debug!(
            "CPU gauge: {cpu_gauge:.1} -- {list}",
            list = cpu_rates
                .iter()
                .map(|a| format!("{a:.1}"))
                .collect::<Vec<String>>()
                .join(" ")
                .as_str()
        );
        cpu_gauge = cpu_gauge.clamp(0.0, 255.0);

        // NET stats + gauge
        let net_rate = mystats.net_bits().abs();
        let net_gauge =
            ((256.0 * ((net_rate as f32) / 1_000_000.0)) / opts.max_mbps).clamp(0.0, 255.0);
        debug!(
            "NET gauge: {net_gauge:.1} rate: {rate} kbps",
            rate = net_rate / 1000,
        );

        // MEM stats + gauge
        let mem_pct = mystats.mem_usage();
        let mem_gauge = (2.56 * mem_pct).clamp(0.0, 255.0);
        debug!("MEM gauge: {mem_gauge:.1} used: {mem_pct:.1} %");

        set_vu(&mut ser, 1, cpu_gauge as i16)?;
        set_vu(&mut ser, 2, net_gauge as i16)?;
        set_vu(&mut ser, 3, mem_gauge as i16)?;

        // keep the sample rate from drifting
        elapsed_ns = start.elapsed().as_nanos() as u32;
    }
}

const CHANNELS_NUM: usize = 192; // Remember: channel cmd byte has offset 0x30
const MAX_DELTA: i16 = 96;

fn set_vu(ser: &mut Box<dyn SerialPort>, channel: u8, mut gauge: i16) -> anyhow::Result<()> {
    static mut LAST_VAL: [i16; CHANNELS_NUM] = [0; CHANNELS_NUM];

    let ch_i = channel as usize;
    if ch_i >= CHANNELS_NUM {
        bail!(
            "Channel number too large: {ch_i} (maximum {}",
            CHANNELS_NUM - 1
        );
    }

    // limit to gauge values between 0..255
    gauge = gauge.clamp(0, 255);

    // do some smoothing -- only move the gauge MAX_DELTA at once
    let delta = unsafe { gauge - LAST_VAL[ch_i] };
    let delta_sig = delta.signum();
    let delta_trunc = delta.abs().min(MAX_DELTA);
    let new_value = unsafe { LAST_VAL[ch_i] + delta_sig * delta_trunc };
    unsafe {
        LAST_VAL[ch_i] = new_value;
    }
    let cmd_value = new_value.clamp(0, 255) as u8;

    let cmd_buf: [u8; 4] = [0xFD, 0x02, 0x30 + channel, cmd_value];
    Ok(ser.write_all(&cmd_buf)?)
}

fn hello(ser: &mut Box<dyn SerialPort>) -> anyhow::Result<()> {
    for i in (0i16..=255)
        .chain((128..=255).rev())
        .chain(128..=255)
        .chain((0..=255).rev())
    {
        for c in 1u8..=3 {
            set_vu(ser, c, i)?;
        }
        thread::sleep(time::Duration::new(0, 3_000_000));
    }
    Ok(())
}
// EOF
