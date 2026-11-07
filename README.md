# perf-meters

A tiny program to control analog VU meters hooked up with USB.

The application targets Linux and Windows, using `sysinfo` for host metrics.

Other platforms depend on support provided by `sysinfo` and `serialport`.

The firmware for the microcontroller can be found here: <https://github.com/sjm42/vumeter-usb>

## Channels

The program drives 4 analog gauge channels over a serial connection at 115200 baud:

| Channel | Metric | Description |
|---------|--------|-------------|
| Ch0 | CPU | Weighted average of the busiest cores |
| Ch1 | Network | Net bit rate (RX - TX), optionally absolute |
| Ch2 | Disk I/O | Read+write byte rate of the busiest reported disk |
| Ch3 | Memory | Used memory as percentage of total |

Each channel maps its metric to a PWM value in the 0-255 range with configurable min/max bounds.

## Serial Protocol

Commands are sent as 4-byte packets: `[0xFD, 0x02, 0x30 + channel, pwm_value]`.

The `Vu` struct applies delta-smoothing to limit how fast the gauge needle moves per update,
controlled by `--pwm-max-delta` (default 32). This prevents the needle from jumping erratically.

## Disk I/O

On Linux, disk statistics use `procfs` to read `/proc/diskstats`. Whole physical block
devices are selected through `/sys/block`, independently of mounted filesystems. This
includes the backing disks of ZFS pools, LVM volumes, and other storage stacks. Partitions
and virtual block devices (such as device-mapper, software RAID, and loop devices) are
excluded; their physical backing disks are counted instead. No device-name patterns or
ZFS-specific tools are needed. This measures device traffic, so cached reads and buffered
writes may not immediately move the needle.

Windows and other platforms continue to use `sysinfo` disk statistics. On all platforms,
the gauge shows the largest read+write byte rate among the selected disks/volumes, with
full scale at 50 MiB/s (equivalent to the previous 102400 sectors/s scale). Newly discovered
disks and reset counters establish a new baseline without producing a spike. No reported
disks means an idle gauge.

Network and disk rates use actual time between samples, so their scales do not depend on
the chosen sampling frequency. Faster output cadences reuse the latest CPU reading until
`sysinfo` allows another CPU refresh.

Reported collection or serial errors terminate the program with an error message. Some
underlying refresh failures are concealed by `sysinfo` and cannot be detected reliably.

## Usage

```
perf_meters [OPTIONS]
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-p, --port <PORT>` | Serial port device (e.g. `/dev/ttyUSB0`, `COM8`) | required |
| `-l, --list-ports` | List available serial ports and exit | |
| `-c, --calibrate` | Enter interactive calibration mode | |
| `-s, --samplerate <HZ>` | Measurement loop frequency, finite and positive, up to 100 Hz | 5.0 |
| `--pwm-max-delta <N>` | Max gauge movement per sample, 1–255 (smoothing) | 32 |
| `--cpu-pwm-min / --cpu-pwm-max` | CPU channel PWM range | 0 / 255 |
| `--net-gauge-abs` | Use absolute network rate (ignore direction) | false |
| `--net-gauge-mbps <MBPS>` | Network rate that maps to full scale | 100 |
| `--net-pwm-min / --net-pwm-zero / --net-pwm-max` | Network channel PWM range | 0 / 128 / 255 |
| `--mem-pwm-min / --mem-pwm-max` | Memory channel PWM range | 0 / 255 |
| `-v, --verbose` | Info-level logging | |
| `-d, --debug` | Debug-level logging | |
| `-t, --trace` | Trace-level logging | |

Numeric options are validated before any hardware is opened. PWM calibration endpoints
must be finite and in 0–255; reversed endpoints are supported for inverted gauges. In signed
network mode, the zero point must lie between the two endpoints. Network full scale must
be finite and positive. PWM mapping reaches the configured endpoints and rounds to the
nearest integer. The startup sweep and CPU weighting are unchanged.

For library users, `MyStats::new`, `MyStats::refresh`, `MyStats::cpu_usage`,
`MyStats::mem_usage`, `Vu::new`, and `OptsCommon::start_pgm` now return `Result`.
`MyStats` no longer implements `Default`; `net_bits_per_second` replaces `net_bits`,
and disk rates are now in bytes per second.

## Build and Development

This repository uses the stable Rust toolchain configured in `rust-toolchain.toml`.

```bash
cargo fmt                             # format with rustfmt.toml
cargo check                           # fast compile check
cargo test                            # run unit and integration tests
cargo clippy --all-targets --all-features
cargo outdated --root-deps-only       # check direct dependency updates
cargo build --release                 # optimized binary
```

For local hardware testing, list ports first:

```bash
cargo run -- --list-ports
cargo run -- -v --port /dev/ttyUSB0
```

### Calibration Mode

With `--calibrate`, use arrow keys to adjust individual channel PWM values interactively:
- Left/Right: select channel
- Up/Down: adjust gauge value
- Esc: exit

### Example

```bash
perf_meters \
  --cpu-pwm-min 0 --cpu-pwm-max 226 \
  --net-gauge-mbps 40 \
  --net-pwm-min 12 --net-pwm-zero 113 --net-pwm-max 220 \
  --mem-pwm-min 0 --mem-pwm-max 228 \
  --pwm-max-delta 16 --samplerate 4 \
  -v --port /dev/ttyUSB0
```
