// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn run_once(scope: &mut FakeScope<'_>, level: Level, out: &mut Vec<u8>) -> Result<(Report, Vec<&'static str>), Error> {
    let mut options = options();
    options.runs = 1;
    options.trigger_level = level;
    collect(scope, scope.clock, &options, Duration::ZERO, out)
}

#[test]
fn auto_level_measures_the_signal_and_records_its_midpoint() {
    assert!(matches!(options().trigger_level, Level::Auto));
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    let (report, failed) = run_once(&mut scope, Level::Auto, &mut Vec::new()).unwrap();
    assert!(failed.is_empty());
    let json = serde_json::to_value(report).unwrap();
    let level = &json["trigger_level"];
    assert_eq!(level["method"], "measurement_midpoint");
    assert_eq!(level["min_volts"], 0.0);
    assert_eq!(level["max_volts"], 3.3);
    assert_eq!(level["volts"], 1.65);
    assert_eq!(json["views"][0]["samples"][0]["settings"][":TRIG:EDGE:LEV"]["observed"], "1.65");
    assert!(scope.queries.iter().any(|query| query.starts_with(":MEAS:ITEM? VMIN,CHAN2")));
    assert!(scope.queries.iter().any(|query| query.starts_with(":MEAS:ITEM? VMAX,CHAN2")));
}

#[test]
fn unavailable_auto_measurement_prompts_for_explicit_level_and_restores_settings() {
    for fault in [Fault::MissingMeasurement, Fault::SentinelMeasurement] {
        let clock = FakeClock::default();
        let mut scope = FakeScope::new(&clock);
        let initial = scope.settings.clone();
        scope.fault = fault;
        scope.running = false;
        let error = run_once(&mut scope, Level::Auto, &mut Vec::new()).unwrap_err();
        assert!(matches!(error, Error::TriggerLevel));
        assert!(error.to_string().contains("Use --trigger-level <volts>"));
        assert_eq!(scope.probes, 0);
        assert_eq!(scope.settings, initial);
        assert!(!scope.running);
    }
}

#[test]
fn explicit_voltage_accepts_negative_values_and_skips_measurement_queries() {
    let mut args = vec!["--host", "example.invalid", "--label", "base", "--signal-channel", "2", "--out", "results"];
    args.extend(["--trigger-level", "-0.5"]);
    let supplied = BenchmarkOptions::parse(&args).unwrap();
    for invalid in ["NaN", "inf", "1e99"] {
        args[9] = invalid;
        assert!(BenchmarkOptions::parse(&args).is_err());
    }
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.fault = Fault::MissingMeasurement;
    let (report, _) = run_once(&mut scope, supplied.trigger_level, &mut Vec::new()).unwrap();
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["trigger_level"]["volts"], -0.5);
    assert_eq!(json["trigger_level"]["method"], "explicit");
    assert!(!scope.queries.iter().any(|query| query.starts_with(":MEAS:ITEM?")));
}

#[test]
fn acquisition_in_auto_does_not_pass_a_failed_normal_preflight() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.fault = Fault::NoNormalAcquisition;
    let initial = scope.settings.clone();
    let mut output = Vec::new();
    let error = run_once(&mut scope, Level::Auto, &mut output).unwrap_err();
    assert!(error.to_string().contains("no triggers before the first view"));
    assert!(error.to_string().contains("Normal sweep"));
    assert_eq!(scope.probes, 0);
    assert!(output.is_empty());
    assert!(!scope.sends.iter().any(|command| command == ":TIM:MAIN:SCAL 5e-9"));
    assert_eq!(scope.settings, initial);
}

#[test]
fn zero_acquired_or_shown_rate_marks_the_view_and_fails_the_run() {
    for metric in [Metric::AcquiredPerS, Metric::ShownPerS] {
        let clock = FakeClock::default();
        let mut scope = FakeScope::new(&clock);
        scope.fault = Fault::ZeroView(metric);
        let mut output = Vec::new();
        let (report, failed) = run_once(&mut scope, Level::Explicit(1.65), &mut output).unwrap();
        assert!(failed.is_empty());
        assert!(report.check_rates().is_err());
        report.write_summary(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.lines().any(|line| {
            line.starts_with("public-bench: run 1/1 1ch 5ns")
                && line.contains("pictures/s 20.000")
                && line.contains("*OPC? host RTT 40.000 / 60.000 ms")
                && line.ends_with("no triggers")
        }));
        assert!(output.lines().any(|line| line.starts_with("1ch 5ns") && line.ends_with("no triggers")));
        assert!(output.lines().last().unwrap().starts_with("4ch 1us again"));
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["views"][0]["status"], "no triggers");
        assert_eq!(json["views"][1]["status"], "measured");
    }
}

#[test]
fn restores_the_original_setup_and_acquisition_state_after_success() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.running = false;
    let initial = scope.settings.clone();
    let (report, failed) = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap();
    assert!(failed.is_empty());
    assert!(report.check_rates().is_ok());
    assert_eq!(scope.settings, initial);
    assert!(!scope.running);
    assert!(scope.sends.iter().any(|command| command == ":STOP"));
    assert!(!scope.sends.iter().any(|command| command.starts_with(":TIM:DEL:ENAB ")));
}

#[test]
fn leaves_an_existing_single_sweep_alone() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.settings.insert("trig_mode".into(), "Single".into());
    scope.running = false;
    let error = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap_err();
    assert!(error.to_string().contains("leave Single sweep"));
    assert_eq!(scope.sends, ["*CLS"]);
}

#[test]
fn restores_the_original_setup_and_acquisition_state_after_an_error() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    scope.running = false;
    scope.fault = Fault::InvalidProbe;
    let initial = scope.settings.clone();
    let error = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap_err();
    assert!(matches!(error, Error::InvalidReply(OPC)));
    assert_eq!(scope.settings, initial);
    assert!(!scope.running);
}

#[test]
fn restores_after_interruption_while_waiting_for_a_view() {
    let clock = FakeClock { interrupt_at: Some(Duration::from_secs(10)), ..FakeClock::default() };
    let mut scope = FakeScope::new(&clock);
    let initial = scope.settings.clone();
    let error = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap_err();
    assert!(matches!(error, Error::Interrupted(130)));
    assert!(scope.sends.iter().any(|command| command == ":CHAN1:DISP OFF"));
    assert_eq!(scope.settings, initial);
    assert!(scope.running);
}

#[test]
fn restores_after_interruption_between_trigger_exchanges() {
    let clock = FakeClock { interrupt_at: Some(Duration::from_secs(1)), ..FakeClock::default() };
    let mut scope = FakeScope::new(&clock);
    let initial = scope.settings.clone();
    scope.running = false;
    scope.fault = Fault::InterruptAfterTriggerMode;
    let error = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap_err();
    assert!(matches!(error, Error::Interrupted(130)));
    assert_eq!(error.exit_code(), ExitCode::from(130));
    assert_eq!(scope.sends, ["*CLS", ":SYST:SET trig_type = Edge", ":SYST:SET trig_type = Pulse"]);
    assert_eq!(scope.settings, initial);
    assert!(!scope.running);
}

#[test]
fn keeps_the_interrupt_exit_code_when_a_setting_cannot_be_restored() {
    let clock = FakeClock { interrupt_at: Some(Duration::from_secs(10)), ..FakeClock::default() };
    let mut scope = FakeScope::new(&clock);
    scope.fault = Fault::RejectRestore;
    let error = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap_err();
    assert_eq!(error.exit_code(), ExitCode::from(130));
    assert!(error.to_string().ends_with("could not restore or verify settings: trig_level"));
}

#[test]
fn names_an_unrestored_setting_and_verifies_the_rest() {
    let clock = FakeClock::default();
    let mut scope = FakeScope::new(&clock);
    let mut initial = scope.settings.clone();
    scope.fault = Fault::RejectRestore;
    let (_, failed) = run_once(&mut scope, Level::Explicit(1.65), &mut Vec::new()).unwrap();
    let error = with_restoration(Ok(()), failed).unwrap_err();
    assert!(error.to_string().ends_with("could not restore or verify settings: trig_level"));
    initial.insert("trig_level".into(), "1.65".into());
    assert_eq!(scope.settings, initial);
}
