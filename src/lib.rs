// lib.rs

pub use std::io::Write;

pub use clap::Parser;
pub use serialport::SerialPort;
pub use tracing::*;

pub use config::*;
pub use stats::*;

mod config;
mod stats;

pub const N_CHANS: usize = 4;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum Channel {
    Ch0 = 0,
    Ch1,
    Ch2,
    Ch3,
}
impl Channel {
    pub fn next(self) -> Channel {
        match self {
            Channel::Ch0 => Channel::Ch1,
            Channel::Ch1 => Channel::Ch2,
            Channel::Ch2 => Channel::Ch3,
            Channel::Ch3 => Channel::Ch0,
        }
    }
    pub fn prev(self) -> Channel {
        match self {
            Channel::Ch0 => Channel::Ch3,
            Channel::Ch1 => Channel::Ch0,
            Channel::Ch2 => Channel::Ch1,
            Channel::Ch3 => Channel::Ch2,
        }
    }
}

pub struct Vu {
    last_val: [i16; N_CHANS],
    max_delta: i16,
}
impl Vu {
    pub fn new(max_delta: i16) -> Self {
        Self {
            last_val: [0; _],
            max_delta,
        }
    }

    pub fn set(&mut self, ser: &mut Box<dyn SerialPort>, channel: Channel, pwm: i16) -> anyhow::Result<()> {
        let ch_i = channel as usize;

        // limit to gauge values between 0..255
        let pwm = pwm.clamp(0, 255);

        // do some smoothing -- only move the gauge MAX_DELTA at once
        let delta = pwm - self.last_val[ch_i];
        let delta_sig = delta.signum();
        let delta_trunc = delta.abs().min(self.max_delta);
        let new_value = self.last_val[ch_i] + delta_sig * delta_trunc;
        self.last_val[ch_i] = new_value;

        let cmd_value = new_value.clamp(0, 255) as u8;
        let cmd_buf: [u8; 4] = [0xFD, 0x02, 0x30 + channel as u8, cmd_value];
        Ok(ser.write_all(&cmd_buf)?)
    }
}
// EOF
