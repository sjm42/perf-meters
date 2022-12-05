// startup.rs

use log::*;
use structopt::StructOpt;

#[derive(Debug, Default, StructOpt)]
pub struct OptsCommon {
    #[structopt(short, long)]
    pub verbose: bool,
    #[structopt(short, long)]
    pub debug: bool,
    #[structopt(short, long)]
    pub trace: bool,

    #[structopt(short, long)]
    pub list_ports: bool,
    #[structopt(short, long)]
    pub calibrate: bool,

    #[structopt(short, long, default_value = "/dev/VUmeter")]
    pub port: String,
    #[structopt(short, long, default_value = "5")]
    pub samplerate: f32,

    #[structopt(long, default_value = "32")]
    pub pwm_max_delta: i16,
    #[structopt(long, default_value = "0")]
    pub cpu_pwm_min: f32,
    #[structopt(long, default_value = "255")]
    pub cpu_pwm_max: f32,

    #[structopt(long)]
    pub net_gauge_abs: bool,
    #[structopt(long, default_value = "100")]
    pub net_gauge_mbps: f32,
    #[structopt(long, default_value = "0")]
    pub net_pwm_min: f32,
    #[structopt(long, default_value = "128")]
    pub net_pwm_zero: f32,
    #[structopt(long, default_value = "255")]
    pub net_pwm_max: f32,

    #[structopt(long, default_value = "0")]
    pub mem_pwm_min: f32,
    #[structopt(long, default_value = "255")]
    pub mem_pwm_max: f32,
}

impl OptsCommon {
    pub fn get_loglevel(&self) -> LevelFilter {
        if self.trace {
            LevelFilter::Trace
        } else if self.debug {
            LevelFilter::Debug
        } else if self.verbose {
            LevelFilter::Info
        } else {
            LevelFilter::Warn
        }
    }

    pub fn start_pgm(&self, name: &str) {
        env_logger::Builder::new()
            .filter_module(env!("CARGO_PKG_NAME"), self.get_loglevel())
            .filter_module(name, self.get_loglevel())
            .format_timestamp_secs()
            .init();
        info!("Starting up {name} v{}...", env!("CARGO_PKG_VERSION"));
        debug!("Git branch: {}", env!("GIT_BRANCH"));
        debug!("Git commit: {}", env!("GIT_COMMIT"));
        debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
        debug!("Compiler version: {}", env!("RUSTC_VERSION"));
    }
}

// EOF
