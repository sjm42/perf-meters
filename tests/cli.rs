use std::process::Command;

#[test]
fn invalid_options_fail_before_opening_hardware() {
    for (args, expected) in [
        (vec!["--samplerate=0"], "--samplerate"),
        (vec!["--samplerate=-1"], "--samplerate"),
        (vec!["--samplerate=NaN"], "--samplerate"),
        (vec!["--samplerate=inf"], "--samplerate"),
        (vec!["--samplerate=101"], "--samplerate"),
        (vec!["--samplerate=1e-40"], "--samplerate"),
        (vec!["--net-gauge-mbps=0"], "--net-gauge-mbps"),
        (vec!["--net-gauge-mbps=-1"], "--net-gauge-mbps"),
        (vec!["--net-gauge-mbps=NaN"], "--net-gauge-mbps"),
        (vec!["--net-gauge-mbps=inf"], "--net-gauge-mbps"),
        (vec!["--pwm-max-delta=0"], "--pwm-max-delta"),
        (vec!["--pwm-max-delta=-32768"], "--pwm-max-delta"),
        (vec!["--pwm-max-delta=256"], "--pwm-max-delta"),
        (vec!["--cpu-pwm-min=-1"], "--cpu-pwm-min"),
        (vec!["--cpu-pwm-max=256"], "--cpu-pwm-max"),
        (vec!["--mem-pwm-min=NaN"], "--mem-pwm-min"),
        (vec!["--mem-pwm-max=inf"], "--mem-pwm-max"),
        (vec!["--net-pwm-zero=NaN"], "--net-pwm-zero"),
        (
            vec!["--net-pwm-min=12", "--net-pwm-max=220", "--net-pwm-zero=250"],
            "--net-pwm-zero",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_perf_meters"))
            .args(["--port", "unused", "--verbose"])
            .args(&args)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
        assert!(!stdout.contains("Opening serial port"), "{args:?}: {stdout}");
        assert!(!stderr.contains("Unable to open serial port"), "{args:?}: {stderr}");
    }
}

#[test]
fn missing_port_is_an_error_and_help_needs_no_hardware() {
    let output = Command::new(env!("CARGO_BIN_EXE_perf_meters")).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No port specified"));
    let output = Command::new(env!("CARGO_BIN_EXE_perf_meters"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--list-ports"));
}
