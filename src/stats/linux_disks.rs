// linux_disks.rs

use crate::*;

// Linux diskstats sectors always represent 512 bytes, regardless of device sector size.
const SECTOR_BYTES: u64 = 512;

pub(super) fn snapshot() -> Result<HashMap<PathBuf, (u64, u64)>> {
    let stats = procfs::diskstats().context("Unable to read /proc/diskstats")?;
    let mut devices = HashSet::new();
    // /sys/block contains whole devices, independently of filesystem mounts. Skip
    // virtual devices (dm, md, loop, etc.) so stacked storage is measured at its
    // physical backing disks, including the members of ZFS pools.
    for entry in fs::read_dir("/sys/block").context("Unable to enumerate /sys/block")? {
        let entry = entry.context("Unable to read a /sys/block entry")?;
        let target = fs::canonicalize(entry.path())
            .with_context(|| format!("Unable to resolve block device {:?}", entry.file_name()))?;
        if !target.starts_with("/sys/devices/virtual/block") {
            devices.insert(entry.file_name());
        }
    }
    counters(stats, &devices)
}

fn counters(stats: Vec<procfs::DiskStat>, devices: &HashSet<OsString>) -> Result<HashMap<PathBuf, (u64, u64)>> {
    let mut counters = HashMap::new();
    for stat in stats {
        if !devices.contains(OsStr::new(&stat.name)) {
            continue;
        }
        let read = stat
            .sectors_read
            .checked_mul(SECTOR_BYTES)
            .with_context(|| format!("Read counter overflow for disk {:?}", stat.name))?;
        let written = stat
            .sectors_written
            .checked_mul(SECTOR_BYTES)
            .with_context(|| format!("Write counter overflow for disk {:?}", stat.name))?;
        counters.insert(PathBuf::from(stat.name), (read, written));
    }
    Ok(counters)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(name: &str, read: u64, written: u64) -> procfs::DiskStat {
        procfs::DiskStat::from_line(&format!("259 0 {name} 1 0 {read} 0 1 0 {written} 0 0 0 0")).unwrap()
    }

    #[test]
    fn backing_disks_are_counted_without_mounted_filesystems_or_name_patterns() {
        let names = ["nvme0n1", "nvme10n1", "sdaa", "vda"];
        let devices = names.iter().map(|name| OsString::from(*name)).collect();
        let mut stats: Vec<_> = names.iter().map(|name| stat(name, 100, 200)).collect();
        // Partitions and virtual devices are not in the physical whole-device set.
        stats.extend([
            stat("nvme0n1p4", 100, 200),
            stat("dm-0", 100, 200),
            stat("loop0", 100, 200),
        ]);
        let snapshot = counters(stats, &devices).unwrap();
        assert_eq!(snapshot.len(), names.len());
        for name in names {
            assert_eq!(snapshot.get(&PathBuf::from(name)), Some(&(51_200, 102_400)));
        }
    }

    #[test]
    fn sector_conversion_reports_overflow_instead_of_wrapping() {
        let devices = HashSet::from([OsString::from("sda")]);
        for disk in [stat("sda", u64::MAX, 0), stat("sda", 0, u64::MAX)] {
            assert!(counters(vec![disk], &devices).is_err());
        }
    }
}

// EOF
