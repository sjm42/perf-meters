# Repository Guidelines

## Project Structure & Module Organization

This Rust 2024 crate drives four USB-connected analog VU meters from host performance metrics.

- `src/lib.rs` exposes shared types (`Channel`, `Vu`) and re-exports the config and stats modules.
- `src/config.rs` defines CLI options and logging setup with `clap` and `tracing`.
- `src/stats.rs` gathers CPU, memory, network, and Linux disk statistics.
- `src/bin/perf_meters.rs` is the executable entry point and serial-port control loop.
- `build.rs` embeds build metadata. `bin/` contains prebuilt Windows binaries; avoid changing them unless updating a release artifact.
- There is no `tests/` directory currently; add unit tests beside the module or integration tests under `tests/` when behavior grows.

## Build, Test, and Development Commands

- `cargo fmt` formats the project using `rustfmt.toml`.
- `cargo check` validates the crate quickly without producing an optimized binary.
- `cargo test` runs all unit and integration tests.
- `cargo clippy --all-targets --all-features` runs lints across binaries and tests.
- `cargo outdated --root-deps-only` checks whether direct dependencies can be upgraded.
- `cargo build --release` builds the optimized executable.
- `cargo run -- --list-ports` lists serial ports.
- `cargo run -- --port /dev/ttyUSB0 -v` runs the meter loop on Linux. On Windows use a COM port, for example `--port COM8`; `run.bat` shows a calibrated Windows command.

## Coding Style & Naming Conventions

Follow standard Rust naming: `snake_case` for functions, variables, modules, and CLI fields; `UpperCamelCase` for types and enum variants; `SCREAMING_SNAKE_CASE` for constants. Keep formatting under `max_width = 120`, with crate-grouped imports as configured in `rustfmt.toml`. Prefer `anyhow::Result` for fallible application paths and `tracing` macros for runtime diagnostics.

## Testing Guidelines

Run `cargo test` before submitting changes. For pure logic, add `#[cfg(test)]` unit tests in the same file. For CLI behavior, add integration tests under `tests/`. Isolate hardware-dependent serial behavior where possible so smoothing, clamping, and metric mapping can be tested without a device.

## Commit & Pull Request Guidelines

Recent history uses short, imperative, lowercase subjects such as `cargo update`; keep commit titles concise and focused. Pull requests should describe the behavior change, list validation commands run, and call out platform or hardware assumptions. Include screenshots or terminal output only when they clarify user-facing CLI behavior.

## Security & Configuration Tips

Do not hard-code private serial device names, host-specific paths, or calibration values into source. Keep local calibration examples in scripts such as `run.bat` or in documentation.
