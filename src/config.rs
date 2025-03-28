// startup.rs

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

    pub fn start_pgm(&self, name: &str) {
        tracing_subscriber::fmt()
            .with_max_level(self.get_loglevel())
            .with_target(false)
            .init();

        info!("Starting up {name} v{}...", env!("CARGO_PKG_VERSION"));
        debug!("Git branch: {}", env!("GIT_BRANCH"));
        debug!("Git commit: {}", env!("GIT_COMMIT"));
        debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
        debug!("Compiler version: {}", env!("RUSTC_VERSION"));
    }
}
// EOF
