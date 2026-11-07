// lib.rs

pub use std::io::Write;

use anyhow::{Context, ensure};

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
    pub fn new(max_delta: i16) -> anyhow::Result<Self> {
        ensure!((1..=255).contains(&max_delta), "Maximum PWM delta must be in 1..=255");
        Ok(Self {
            last_val: [0; _],
            max_delta,
        })
    }

    pub fn set<W: Write + ?Sized>(&mut self, ser: &mut W, channel: Channel, pwm: i16) -> anyhow::Result<()> {
        let ch_i = channel as usize;

        // limit to gauge values between 0..255
        let pwm = pwm.clamp(0, 255);

        // do some smoothing -- only move the gauge MAX_DELTA at once
        let delta = pwm - self.last_val[ch_i];
        let delta_sig = delta.signum();
        let delta_trunc = delta.abs().min(self.max_delta);
        let new_value = self.last_val[ch_i] + delta_sig * delta_trunc;
        let cmd_value = new_value.clamp(0, 255) as u8;
        let cmd_buf: [u8; 4] = [0xFD, 0x02, 0x30 + channel as u8, cmd_value];
        ser.write_all(&cmd_buf)
            .with_context(|| format!("Unable to set {channel:?} to PWM {cmd_value}"))?;
        self.last_val[ch_i] = new_value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn smoothing_limits_movement_and_clamps_pwm() {
        let mut vu = Vu::new(32).unwrap();
        let mut output = Vec::new();
        vu.set(&mut output, Channel::Ch0, i16::MAX).unwrap();
        vu.set(&mut output, Channel::Ch0, i16::MAX).unwrap();
        vu.set(&mut output, Channel::Ch0, i16::MIN).unwrap();
        vu.set(&mut output, Channel::Ch1, 10).unwrap();
        assert_eq!(
            output,
            [
                0xFD, 2, 0x30, 32, 0xFD, 2, 0x30, 64, 0xFD, 2, 0x30, 32, 0xFD, 2, 0x31, 10
            ]
        );
        let mut unsmoothed = Vu::new(255).unwrap();
        let mut output = Vec::new();
        unsmoothed.set(&mut output, Channel::Ch3, i16::MAX).unwrap();
        unsmoothed.set(&mut output, Channel::Ch3, i16::MIN).unwrap();
        assert_eq!(output, [0xFD, 2, 0x33, 255, 0xFD, 2, 0x33, 0]);
    }

    #[test]
    fn invalid_smoothing_limits_are_errors() {
        for value in [i16::MIN, -1, 0, 256, i16::MAX] {
            assert!(Vu::new(value).is_err());
        }
    }

    struct ShortWriter {
        bytes: Vec<u8>,
        fail_after: Option<usize>,
    }

    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_after == Some(self.bytes.len()) {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "disconnected"));
            }
            let Some(&byte) = bytes.first() else {
                return Ok(0);
            };
            self.bytes.push(byte);
            Ok(1)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn partial_writes_complete_packets_and_failures_preserve_state() {
        let mut vu = Vu::new(32).unwrap();
        let mut output = ShortWriter {
            bytes: Vec::new(),
            fail_after: None,
        };
        vu.set(&mut output, Channel::Ch0, 255).unwrap();
        assert_eq!(output.bytes, [0xFD, 2, 0x30, 32]);
        let mut disconnected = ShortWriter {
            bytes: Vec::new(),
            fail_after: Some(2),
        };
        let error = vu.set(&mut disconnected, Channel::Ch0, 255).unwrap_err();
        assert!(format!("{error:#}").contains("disconnected"));
        assert_eq!(vu.last_val[0], 32);
    }
}
// EOF
