# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

perf-meters is a Rust CLI tool that drives physical analog VU meters over USB serial. It reads system performance stats (CPU, memory, network, disk I/O) and sends PWM values to a microcontroller (firmware: [vumeter-usb](https://github.com/sjm42/vumeter-usb)). Targets Linux and Windows.

## Build Commands

```bash
cargo build              # debug build
cargo build --release    # release build (fat LTO, single codegen unit)
cargo run -- --help      # run with help
cargo run -- -v -p /dev/ttyUSB0   # example: run with verbose on a serial port
cargo run -- --list-ports          # list available serial ports
cargo run -- --calibrate -p /dev/ttyUSB0  # interactive gauge calibration
```

Uses Rust 2024 edition on stable toolchain. No tests or lints are configured.

## Architecture

Single binary (`perf_meters`) with a library crate:

- **`src/bin/perf_meters.rs`** — Main binary. Opens serial port, runs the measurement loop at a configurable sample rate. Maps system stats to PWM values (0-255) across 4 channels: Ch0=CPU, Ch1=Network, Ch2=Disk I/O, Ch3=Memory. Also contains `hello()` (startup sweep animation) and `calibrate()` (interactive arrow-key calibration mode).
- **`src/lib.rs`** — `Channel` enum (Ch0-Ch3) and `Vu` struct. `Vu::set()` sends 4-byte serial commands (`[0xFD, 0x02, 0x30+chan, pwm]`) with delta-smoothing to prevent gauge needle jumps.
- **`src/config.rs`** — `OptsCommon` struct using `clap` derive for CLI args. All PWM min/max/range values and sample rate are configurable. Handles tracing/log level setup.
- **`src/stats.rs`** — `MyStats` wraps `sysinfo` crate for CPU/memory/network stats. `DiskStats` reads `/proc/diskstats` directly for disk I/O rates (Linux-specific; tracks sd* and nvme* devices).
- **`build.rs`** — Embeds git branch, commit, source timestamp, and rustc version via `build-data` crate.

## Formatting

Uses `rustfmt.toml`: max_width=120, crate-level import granularity, std/external/crate import grouping.
