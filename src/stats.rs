// stats.rs

use conv::ValueFrom;
use std::cmp::Ordering;
use sysinfo::*;

#[derive(Debug)]
pub struct MyStats {
    sys: System,
    refresh: RefreshKind,
    n_cpu: usize,
}
impl MyStats {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let refresh = RefreshKind::new()
            .with_cpu(CpuRefreshKind::new().with_cpu_usage())
            .with_memory()
            .with_networks();
        let n_cpu = sys.physical_core_count().unwrap_or(1);

        MyStats {
            sys,
            refresh,
            n_cpu,
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_specifics(self.refresh);
    }

    pub fn sys(&self) -> &System {
        &self.sys
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

    // return number of bits transferred since last sample
    pub fn net_bits(&self) -> i64 {
        let mut rx: i64 = 0;
        let mut tx: i64 = 0;

        for (_iface, data) in self.sys.networks().iter() {
            rx = rx.saturating_add(i64::try_from(data.received()).unwrap_or(0));
            tx = tx.saturating_add(i64::try_from(data.transmitted()).unwrap_or(0));
        }
        rx.saturating_sub(tx).saturating_mul(8)
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
