// stats.rs

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, ensure};
use sysinfo::{CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, RefreshKind, System};

use crate::warn;

#[derive(Debug)]
pub struct DiskStats {
    disks: Disks,
    prev_ts: Instant,
    prev_stats: HashMap<PathBuf, (u64, u64)>,
    rates: Vec<f64>,
}

impl DiskStats {
    pub fn new() -> anyhow::Result<Self> {
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing().with_io_usage())
            .context("Unable to initialize disk statistics")?;
        let prev_stats = Self::snapshot(&disks);
        Ok(Self {
            disks,
            prev_ts: Instant::now(),
            prev_stats,
            rates: Vec::new(),
        })
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        self.disks
            .refresh_specifics(true, DiskRefreshKind::nothing().with_io_usage());
        let now = Instant::now();
        let elapsed = now.duration_since(self.prev_ts);
        let stats = Self::snapshot(&self.disks);
        let mut rates = Vec::with_capacity(stats.len());
        for (name, &current) in &stats {
            // A newly discovered disk needs a baseline, not a since-boot rate.
            let previous = self.prev_stats.get(name).copied().unwrap_or(current);
            rates.push(disk_rate(previous, current, elapsed)?);
        }
        rates.sort_by(|a, b| b.total_cmp(a));
        self.rates = rates;
        self.prev_stats = stats;
        self.prev_ts = now;
        Ok(())
    }

    /// Read + write rates in bytes per second, largest first.
    pub fn rates(&self) -> &[f64] {
        &self.rates
    }

    fn snapshot(disks: &Disks) -> HashMap<PathBuf, (u64, u64)> {
        disks
            .iter()
            .map(|disk| {
                let usage = disk.usage();
                (
                    disk.mount_point().to_path_buf(),
                    (usage.total_read_bytes, usage.total_written_bytes),
                )
            })
            .collect()
    }
}

fn per_second(amount: f64, elapsed: Duration) -> anyhow::Result<f64> {
    ensure!(
        !elapsed.is_zero(),
        "Cannot calculate a rate over a zero sampling interval"
    );
    let rate = amount / elapsed.as_secs_f64();
    ensure!(rate.is_finite(), "Non-finite metric rate");
    Ok(rate)
}

fn disk_rate(previous: (u64, u64), current: (u64, u64), elapsed: Duration) -> anyhow::Result<f64> {
    // Rebaseline after a counter reset or device replacement.
    let (Some(read), Some(written)) = (current.0.checked_sub(previous.0), current.1.checked_sub(previous.1)) else {
        return Ok(0.0);
    };
    per_second(read as f64 + written as f64, elapsed)
}

fn memory_usage(used: u64, total: u64) -> anyhow::Result<f64> {
    ensure!(
        total > 0 && used <= total,
        "Invalid memory statistics: used {used}, total {total}"
    );
    Ok(100.0 * used as f64 / total as f64)
}

#[derive(Debug)]
pub struct MyStats {
    sys: System,
    networks: Networks,
    n_cpu: usize,
    diskstats: DiskStats,
    cpu_ts: Instant,
    network_ts: Instant,
    net_bps: f64,
}

impl MyStats {
    pub fn new() -> anyhow::Result<Self> {
        let refresh = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything().without_swap());
        let sys = System::new_with_specifics(refresh).context("Unable to initialize system statistics")?;
        let cpu_ts = Instant::now();
        ensure!(!sys.cpus().is_empty(), "No CPU readings available");
        memory_usage(sys.used_memory(), sys.total_memory())?;
        let n_cpu = match System::physical_core_count() {
            Ok(count) if count > 0 => count,
            _ => {
                warn!("Physical CPU count unavailable; using logical CPU count");
                sys.cpus().len()
            }
        };
        let networks = Networks::new_with_refreshed_list().context("Unable to initialize network statistics")?;
        let network_ts = Instant::now();
        let diskstats = DiskStats::new()?;
        Ok(Self {
            sys,
            networks,
            n_cpu,
            diskstats,
            cpu_ts,
            network_ts,
            net_bps: 0.0,
        })
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        // Faster output cadences can reuse the last CPU reading.
        if self.cpu_ts.elapsed() >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            self.sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_usage());
            self.cpu_ts = Instant::now();
        }
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::everything().without_swap());
        self.networks.refresh(true);
        let now = Instant::now();
        let bytes: f64 = self
            .networks
            .values()
            .map(|data| data.received() as f64 - data.transmitted() as f64)
            .sum();
        self.net_bps = per_second(bytes * 8.0, now.duration_since(self.network_ts))?;
        self.network_ts = now;
        self.diskstats.refresh().context("Unable to refresh disk statistics")?;
        Ok(())
    }

    pub fn sys(&self) -> &System {
        &self.sys
    }

    pub fn networks(&self) -> &Networks {
        &self.networks
    }

    /// Physical core count (logical count when physical topology is unavailable).
    pub fn n_cpu(&self) -> usize {
        self.n_cpu
    }

    /// Logical CPU readings, largest first.
    pub fn cpu_usage(&self) -> anyhow::Result<Vec<f32>> {
        let mut usages: Vec<_> = self.sys.cpus().iter().map(|cpu| cpu.usage()).collect();
        ensure!(!usages.is_empty(), "No CPU readings available");
        ensure!(usages.iter().all(|value| value.is_finite()), "Non-finite CPU reading");
        usages.sort_by(|a, b| b.total_cmp(a));
        Ok(usages)
    }

    /// Net receive rate (RX minus TX), in bits per second.
    pub fn net_bits_per_second(&self) -> f64 {
        self.net_bps
    }

    /// Read + write rate of the busiest reported disk, in bytes per second.
    pub fn disk_io(&self) -> f64 {
        self.diskstats.rates().first().copied().unwrap_or(0.0)
    }

    pub fn mem_usage(&self) -> anyhow::Result<f64> {
        memory_usage(self.sys.used_memory(), self.sys.total_memory())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_use_actual_elapsed_time_and_preserve_direction() {
        for (amount, elapsed) in [
            (2_500_000.0, Duration::from_millis(200)),
            (12_500_000.0, Duration::from_secs(1)),
        ] {
            assert_eq!(per_second(amount * 8.0, elapsed).unwrap(), 100_000_000.0);
            assert_eq!(per_second(-amount * 8.0, elapsed).unwrap(), -100_000_000.0);
        }
        assert!(per_second(1.0, Duration::ZERO).is_err());
        assert!(per_second(f64::NAN, Duration::from_secs(1)).is_err());
    }

    #[test]
    fn disk_counters_handle_resets_and_large_values() {
        let second = Duration::from_secs(1);
        assert_eq!(disk_rate((100, 200), (150, 350), second).unwrap(), 200.0);
        assert_eq!(disk_rate((100, 200), (10, 350), second).unwrap(), 0.0);
        assert_eq!(disk_rate((100, 200), (150, 10), second).unwrap(), 0.0);
        assert_eq!(disk_rate((50, 60), (50, 60), second).unwrap(), 0.0);
        assert_eq!(
            disk_rate((0, 0), (u64::MAX, u64::MAX), second).unwrap(),
            2.0 * u64::MAX as f64
        );
        assert!(disk_rate((0, 0), (1, 1), Duration::ZERO).is_err());
    }

    #[test]
    fn memory_requires_valid_totals() {
        assert_eq!(memory_usage(0, 100).unwrap(), 0.0);
        assert_eq!(memory_usage(50, 100).unwrap(), 50.0);
        assert_eq!(memory_usage(100, 100).unwrap(), 100.0);
        assert!(memory_usage(0, 0).is_err());
        assert!(memory_usage(101, 100).is_err());
    }
}
