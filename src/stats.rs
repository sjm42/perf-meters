// stats.rs

use conv::ValueFrom;
use std::{cmp::Ordering, collections::HashMap, fs::File, io::{self, BufRead}, time};
use sysinfo::*;

use crate::*;

const DISK_STATS: &str = "/proc/diskstats";


#[derive(Debug)]
pub struct DiskStats {
    prev_ts: time::Instant,
    prev_stats: HashMap<String, (i64, i64)>,
    rates: Vec<f64>,
}

impl DiskStats {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            prev_ts: time::Instant::now(),
            prev_stats: Self::read_diskstats()?,
            rates: Vec::new(),
        })
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        self.rates = self.diskrates()?;
        Ok(())
    }

    pub fn rates(&self) -> &Vec<f64> {
        &self.rates
    }

    fn diskrates(&mut self) -> anyhow::Result<Vec<f64>> {
        let us = self.prev_ts.elapsed().as_micros();
        self.prev_ts = time::Instant::now();

        let stats = Self::read_diskstats()?;
        let mut rates = Vec::with_capacity(stats.len());

        for (k, v) in &stats {
            match self.prev_stats.get(k) {
                None => continue,
                Some(prev) => {
                    let sect_rd = v.0 - prev.0;
                    let sect_wrt = v.1 - prev.1;
                    rates.push((sect_rd + sect_wrt) as f64 * 1_000_000.0 / us as f64);
                }
            }
        }
        // Rust refuses to just sort() f64, because NaN, Inf etc.
        rates.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
        self.prev_stats = stats;
        Ok(rates)
    }

    // https://www.kernel.org/doc/Documentation/ABI/testing/procfs-diskstats
    fn read_diskstats() -> anyhow::Result<HashMap<String, (i64, i64)>> {
        let mut stats = HashMap::with_capacity(32);
        for line in io::BufReader::new(File::open(DISK_STATS)?).lines() {
            let line = line?;
            let items = line.split_ascii_whitespace().collect::<Vec<&str>>();
            let devname = items[2];
            // collect sectors read and sectors written from "sd?" and "nvme???"
            if devname.starts_with("sd") && devname.len() == 3
                || devname.starts_with("nvme") && devname.len() == 7
            {
                let sect_rd = items[5].parse::<i64>()?;
                let sect_wrt = items[9].parse::<i64>()?;
                stats.insert(devname.into(), (sect_rd, sect_wrt));
            }
        }
        Ok(stats)
    }
}


#[derive(Debug)]
pub struct MyStats {
    sys: System,
    refresh: RefreshKind,
    networks: Networks,
    n_cpu: usize,
    diskstats: DiskStats,
}

impl MyStats {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let refresh = RefreshKind::new()
            .with_cpu(CpuRefreshKind::new().with_cpu_usage())
            .with_memory(MemoryRefreshKind::everything().without_swap());
        let networks = Networks::new_with_refreshed_list();
        let n_cpu = sys.physical_core_count().unwrap_or(1);
        let diskstats = DiskStats::new().expect("Unable to get disk statistics");

        MyStats {
            sys,
            refresh,
            networks,
            n_cpu,
            diskstats,
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_specifics(self.refresh);
        self.networks.refresh();
        if let Err(e) = self.diskstats.refresh() {
            error!("Error refreshing diskstats: {e} (ignored)");
        }
    }

    pub fn sys(&self) -> &System {
        &self.sys
    }

    pub fn networks(&self) -> &Networks {
        &self.networks
    }

    // number of cpu threads/cores
    pub fn n_cpu(&self) -> usize {
        self.n_cpu
    }

    // return a sorted vector (largest first) of cpu cores usage
    pub fn cpu_usage(&self) -> Vec<f32> {
        let mut usages = Vec::with_capacity(self.n_cpu);
        for c in self.sys.cpus().iter() {
            usages.push(c.cpu_usage());
        }
        // Rust refuses to just sort() f32/f64, because NaN etc.
        usages.sort_by(|a, b| {
            b.partial_cmp(a).unwrap_or_else(|| {
                // to end up here, either or both of a/b must be NaN
                if b.is_nan() {
                    if a.is_nan() {
                        // both are NaN
                        Ordering::Equal
                    } else {
                        // only b is NaN
                        Ordering::Less
                    }
                } else {
                    // only a is NaN
                    Ordering::Greater
                }
            })
        });
        usages
    }

    // return number of bits transferred
    pub fn net_bits(&self) -> i64 {
        let mut rx: i64 = 0;
        let mut tx: i64 = 0;

        for (_iface, data) in self.networks.iter() {
            rx = rx.saturating_add(i64::try_from(data.received()).unwrap_or(0));
            tx = tx.saturating_add(i64::try_from(data.transmitted()).unwrap_or(0));
        }
        rx.saturating_add(tx).saturating_mul(8)
    }

    // return sectors read+written on the most active disk
    pub fn disk_io(&self) -> f64 {
        match self.diskstats.rates.first()
        {
            None => 0.0,
            Some(r) => *r
        }
    }


    // return used memory as percentage
    pub fn mem_usage(&self) -> f32 {
        let used = f64::value_from(self.sys.used_memory()).unwrap_or(0.0);
        let total = f64::value_from(self.sys.total_memory()).unwrap_or(0.0);
        100.0 * ((used / total) as f32)
    }
}

impl Default for MyStats {
    fn default() -> Self {
        Self::new()
    }
}

// EOF
