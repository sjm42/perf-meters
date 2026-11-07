// startup.rs

use std::time::Duration;

use anyhow::{Context, ensure};

use crate::*;

#[derive(Debug, Default, Parser)]
pub struct OptsCommon {
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(short, long)]
    pub debug: bool,
    #[arg(short, long)]
    pub trace: bool,

    #[arg(short, long)]
    pub list_ports: bool,
    #[arg(short, long)]
    pub calibrate: bool,

    #[arg(short, long)]
    pub port: Option<String>,
    #[arg(short, long, default_value_t = 5.0)]
    pub samplerate: f32,

    #[arg(long, default_value_t = 32)]
    pub pwm_max_delta: i16,
    #[arg(long, default_value_t = 0.0)]
    pub cpu_pwm_min: f32,
    #[arg(long, default_value_t = 255.0)]
    pub cpu_pwm_max: f32,

    #[arg(long)]
    pub net_gauge_abs: bool,
    #[arg(long, default_value_t = 100.0)]
    pub net_gauge_mbps: f32,
    #[arg(long, default_value_t = 0.0)]
    pub net_pwm_min: f32,
    #[arg(long, default_value_t = 128.0)]
    pub net_pwm_zero: f32,
    #[arg(long, default_value_t = 255.0)]
    pub net_pwm_max: f32,

    #[arg(long, default_value_t = 0.0)]
    pub mem_pwm_min: f32,
    #[arg(long, default_value_t = 255.0)]
    pub mem_pwm_max: f32,
}

impl OptsCommon {
    /// Check options before opening or moving any hardware.
    pub fn validate(&self) -> anyhow::Result<()> {
        self.sample_period()?;
        ensure!(
            (1..=255).contains(&self.pwm_max_delta),
            "--pwm-max-delta must be in 1..=255"
        );
        ensure!(
            self.net_gauge_mbps.is_finite() && self.net_gauge_mbps > 0.0,
            "--net-gauge-mbps must be finite and positive"
        );
        for (name, value) in [
            ("cpu-pwm-min", self.cpu_pwm_min),
            ("cpu-pwm-max", self.cpu_pwm_max),
            ("net-pwm-min", self.net_pwm_min),
            ("net-pwm-zero", self.net_pwm_zero),
            ("net-pwm-max", self.net_pwm_max),
            ("mem-pwm-min", self.mem_pwm_min),
            ("mem-pwm-max", self.mem_pwm_max),
        ] {
            ensure!(
                value.is_finite() && (0.0..=255.0).contains(&value),
                "--{name} must be finite and in 0..=255"
            );
        }
        // Reversed endpoints are valid for gauges wired in the opposite direction.
        if !self.net_gauge_abs {
            ensure!(
                (self.net_pwm_min.min(self.net_pwm_max)..=self.net_pwm_min.max(self.net_pwm_max))
                    .contains(&self.net_pwm_zero),
                "--net-pwm-zero must lie between --net-pwm-min and --net-pwm-max"
            );
        }
        ensure!(
            self.list_ports || self.port.as_ref().is_some_and(|port| !port.is_empty()),
            "No port specified; use --port or --list-ports"
        );
        Ok(())
    }

    pub fn sample_period(&self) -> anyhow::Result<Duration> {
        // 100 Hz keeps serial output comfortably below the 115200-baud link capacity.
        ensure!(
            self.samplerate.is_finite() && self.samplerate > 0.0 && self.samplerate <= 100.0,
            "--samplerate must be finite, positive, and at most 100 Hz"
        );
        Duration::try_from_secs_f64(1.0 / f64::from(self.samplerate))
            .context("--samplerate is too small to represent its sampling interval")
    }

    pub fn get_loglevel(&self) -> Level {
        if self.trace {
            Level::TRACE
        } else if self.debug {
            Level::DEBUG
        } else if self.verbose {
            Level::INFO
        } else {
            Level::ERROR
        }
    }

    pub fn start_pgm(&self, name: &str) -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_max_level(self.get_loglevel())
            .with_target(false)
            .try_init()
            .map_err(|error| anyhow::anyhow!("Unable to initialize logging: {error}"))?;

        info!("Starting up {name} v{}...", env!("CARGO_PKG_VERSION"));
        debug!("Git branch: {}", env!("GIT_BRANCH"));
        debug!("Git commit: {}", env!("GIT_COMMIT"));
        debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
        debug!("Compiler version: {}", env!("RUSTC_VERSION"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(args: &[&str]) -> OptsCommon {
        OptsCommon::try_parse_from(
            ["perf_meters", "--port", "unused"]
                .into_iter()
                .chain(args.iter().copied()),
        )
        .unwrap()
    }

    #[test]
    fn defaults_and_reversed_calibration_are_valid() {
        let defaults = options(&[]);
        assert!(defaults.validate().is_ok());
        assert_eq!(defaults.sample_period().unwrap(), Duration::from_millis(200));
        let reversed = options(&[
            "--cpu-pwm-min=255",
            "--cpu-pwm-max=0",
            "--net-pwm-min=255",
            "--net-pwm-max=0",
            "--net-pwm-zero=128",
            "--mem-pwm-min=255",
            "--mem-pwm-max=0",
        ]);
        assert!(reversed.validate().is_ok());
        assert!(
            options(&["--net-gauge-abs", "--net-pwm-min=200", "--net-pwm-max=220"])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn sampling_supports_long_intervals_and_rejects_unrepresentable_ones() {
        assert!(options(&["--samplerate=0.1"]).sample_period().unwrap() > Duration::from_secs(9));
        assert!(options(&["--samplerate=1e-40"]).sample_period().is_err());
    }

    #[test]
    fn logging_initialization_reports_duplicate_subscribers() {
        let opts = options(&[]);
        assert!(opts.start_pgm("test").is_ok());
        assert!(opts.start_pgm("test").is_err());
    }
}
// EOF
