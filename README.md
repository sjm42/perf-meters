# perf-meters

A tiny program to control analog VU meters hooked up with USB.

It works on Linux and Windows 7/10/11 (tested, binary was built in Windows 10).

Other OSes are supported as per what sysinfo crate is able to support.

The firmware for the microcontroller can be found here: <https://github.com/sjm42/vumeter-usb>

## Channels

The program drives 4 analog gauge channels over a serial connection at 115200 baud:

| Channel | Metric | Description |
|---------|--------|-------------|
| Ch0 | CPU | Weighted average of the busiest cores |
| Ch1 | Network | Net bit rate (RX - TX), optionally absolute |
| Ch2 | Disk I/O | Sector read+write rate of the most active disk |
| Ch3 | Memory | Used memory as percentage of total |

Each channel maps its metric to a PWM value in the 0-255 range with configurable min/max bounds.

## Serial Protocol

Commands are sent as 4-byte packets: `[0xFD, 0x02, 0x30 + channel, pwm_value]`.

The `Vu` struct applies delta-smoothing to limit how fast the gauge needle moves per update,
controlled by `--pwm-max-delta` (default 32). This prevents the needle from jumping erratically.

## Disk I/O (Linux)

Disk statistics are read directly from `/proc/diskstats`, tracking `sd?` (SCSI/SATA) and
`nvme???` (NVMe) block devices. The rate is calculated as sectors read+written per second
for the most active disk.

On Windows, the disk I/O channel will not produce readings since `/proc/diskstats` is not available.

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
| `-s, --samplerate <HZ>` | Measurement loop frequency | 5.0 |
| `--pwm-max-delta <N>` | Max gauge movement per sample (smoothing) | 32 |
| `--cpu-pwm-min / --cpu-pwm-max` | CPU channel PWM range | 0 / 255 |
| `--net-gauge-abs` | Use absolute network rate (ignore direction) | false |
| `--net-gauge-mbps <MBPS>` | Network rate that maps to full scale | 100 |
| `--net-pwm-min / --net-pwm-zero / --net-pwm-max` | Network channel PWM range | 0 / 128 / 255 |
| `--mem-pwm-min / --mem-pwm-max` | Memory channel PWM range | 0 / 255 |
| `-v, --verbose` | Info-level logging | |
| `-d, --debug` | Debug-level logging | |
| `-t, --trace` | Trace-level logging | |

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
