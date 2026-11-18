// SPDX-License-Identifier: GPL-2.0-or-later

pub mod support;

use std::process::{Command, Output};
use support::{isolated_inputs, Inputs};

fn success(output: &std::process::Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    // Shadow events and elapsed-time pacing diagnostics share the application's stderr log.
    assert!(
        diagnostics.lines().all(|line| line.starts_with("oos-ui pacing:") || line.starts_with("oos-ui shadow:")),
        "{}",
        diagnostics
    );
    String::from_utf8(output.stdout.clone()).expect("script diagnostics are UTF-8")
}

fn rgb(inputs: &Inputs, name: &str) -> Vec<u8> {
    let bytes = std::fs::read(inputs.path(name)).expect("successful child must write the requested capture");
    let header = b"P6\n1024 600\n255\n";
    assert!(bytes.starts_with(header));
    assert_eq!(bytes.len(), header.len() + 1024 * 600 * 3);
    bytes[header.len()..].to_vec()
}

#[test]
fn child_preserves_steps_capture_names_and_quit() {
    let inputs = isolated_inputs();
    let output = run_host(&inputs, "frames 2\nshot one.ppm\nshot two.png\nstat state.txt\nquit\nshot late.ppm\n", &[]);
    assert_eq!(success(&output), "shot one.ppm\nshot two.png\n");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("oos-ui shadow: row="));
    rgb(&inputs, "one.ppm");
    assert!(std::fs::read(inputs.path("two.png")).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(!inputs.path("late.ppm").exists());
    assert!(inputs.path("two.png.meta").exists());
    let stat = std::fs::read_to_string(inputs.path("state.txt")).unwrap();
    assert!(stat.contains("legacy_observer=retired"), "{stat}");
    assert!(stat.contains("frames 5 records 30"), "{}", stat);
    assert!(stat.contains("t 0.2 "), "{}", stat);
}

#[test]
fn repeated_shadow_events_stay_bounded_on_stderr() {
    let inputs = isolated_inputs();
    let script = format!(
        "{}shot bounded.ppm\n",
        "key runstop\nkey runstop\nmenu Display\nset Grid bright. = 10 %\nset Grid bright. = 20 %\n".repeat(8)
    );
    let output = run_host(&inputs, &script, &[]);
    assert_eq!(success(&output), "shot bounded.ppm\n");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    let run_events: Vec<_> = diagnostics.lines().filter(|line| line.starts_with("oos-ui shadow: row=run ")).collect();
    assert!(run_events.is_empty(), "retired observation must not emit events: {diagnostics}");
    assert_eq!(
        diagnostics.lines().filter(|line| line.starts_with("oos-ui shadow: first field=grid_bright ")).count(),
        0
    );
    rgb(&inputs, "bounded.ppm");
}

#[test]
fn malformed_commands_keep_diagnostics_and_child_errors_propagate() {
    let inputs = isolated_inputs();
    let output = run_host(&inputs, "frames bogus\nframes -1\nset broken\nnot-a-command\nshot ok.ppm\n", &[]);
    assert!(output.status.success());
    let errors = String::from_utf8(output.stderr).unwrap();
    assert!(errors.contains("set <label> = <value>"), "{}", errors);
    assert!(errors.contains("unknown command: not-a-command"), "{}", errors);
    rgb(&inputs, "ok.ppm");
    let output = run_host(&inputs, "shot absent/shot.ppm\n", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("write shot"));
    for args in [&["--hw"][..], &["--gpu", "absent"], &["--scripted-batch", "invalid"], &["--scripted-batch"]] {
        assert_eq!(run_host(&inputs, "", args).status.code(), Some(2), "{:?}", args);
    }
}

#[test]
fn demo_pictures_repeat_in_fresh_processes() {
    let a = isolated_inputs();
    let b = isolated_inputs();
    let script = "frames 4\nshot early.ppm\nkey ch4\nkey ch1\nkey ch1\nframes 3\nshot later.ppm\n";
    success(&run_host(&a, script, &[]));
    success(&run_host(&b, script, &[]));
    for name in ["early.ppm", "later.ppm"] {
        assert_eq!(rgb(&a, name), rgb(&b, name), "{}", name);
    }
    assert_ne!(rgb(&a, "early.ppm"), rgb(&a, "later.ppm"));
}

#[test]
fn batches_repeat_and_account_for_every_frame() {
    let script = "frames 4\nshot batch.ppm\n";
    for kind in ["raw", "codes", "pairs", "deep-pairs"] {
        let a = isolated_inputs();
        let b = isolated_inputs();
        let args = ["--scripted-batch", kind];
        let log = success(&run_host(&a, script, &args));
        assert_eq!(log, success(&run_host(&b, script, &args)), "{}", kind);
        assert!(
            log.contains("batch 6 submitted 86 deposited 86 delivered 6 dropped 0 newest 86 route workers"),
            "{}",
            log
        );
        assert_eq!(rgb(&a, "batch.ppm"), rgb(&b, "batch.ppm"), "{}", kind);
    }
}

#[test]
fn average_zoom_and_slow_timebases_drain_records_without_a_time_cutoff() {
    for commands in ["menu Acquire\nset Mode = Average\nset Averages = 16\n", "knob hscale push\n", "knob hscale -9\n"]
    {
        let a = isolated_inputs();
        let b = isolated_inputs();
        let script = format!("{}frames 2\nshot records.ppm\n", commands);
        let args = ["--scripted-batch", "raw"];
        let log = success(&run_host(&a, &script, &args));
        assert_eq!(log, success(&run_host(&b, &script, &args)));
        assert!(log.contains("route records"), "{}", log);
        assert!(
            log.contains("batch 4 submitted 46 deposited 27 delivered 1 dropped 0 newest 46 route records"),
            "{}",
            log
        );
        assert_eq!(rgb(&a, "records.ppm"), rgb(&b, "records.ppm"));
    }
}

#[test]
fn codes_keep_four_channel_and_dots_pictures_repeatable() {
    for commands in
        ["key ch2\nkey ch3\nkey ch4\n", "menu Display\nset Dots = On\n", "menu Horizontal\nset Time/div = 200 ns\n"]
    {
        let a = isolated_inputs();
        let b = isolated_inputs();
        let script = format!("{}frames 2\nshot fallback.ppm\n", commands);
        let args = ["--scripted-batch", "codes"];
        let log = success(&run_host(&a, &script, &args));
        assert_eq!(log, success(&run_host(&b, &script, &args)));
        assert!(
            log.contains("batch 4 submitted 46 deposited 46 delivered 4 dropped 0 newest 46 route workers"),
            "{}",
            log
        );
        assert!(log.contains("newest 46 route workers decode codes"), "{}", log);
        assert_eq!(rgb(&a, "fallback.ppm"), rgb(&b, "fallback.ppm"));
    }
}

#[test]
fn scripted_batches_reject_measurements_enabled_by_commands() {
    for kind in ["raw", "codes", "pairs", "deep-pairs"] {
        let inputs = isolated_inputs();
        let output = run_host(
            &inputs,
            "menu Measure\ndo Channel pair\ndo Delay rise-rise\nshot forbidden.ppm\n",
            &["--scripted-batch", kind],
        );
        assert_eq!(output.status.code(), Some(2), "{}: {:?}", kind, output);
        assert!(String::from_utf8_lossy(&output.stderr).contains("scripted batches require measurements off until M10"));
        assert!(!inputs.path("forbidden.ppm").exists());
    }
}

#[test]
fn single_captures_one_post_arm_record_and_stops() {
    let inputs = isolated_inputs();
    let log = success(&run_host(
        &inputs,
        "key single\nframes 3\nshot single.ppm\nstat state.txt\n",
        &["--scripted-batch", "raw"],
    ));
    assert!(log.contains("batch 2 submitted 28 deposited 27 delivered 1 dropped 0 newest 28 route records"), "{}", log);
    let state = std::fs::read_to_string(inputs.path("state.txt")).unwrap();
    assert!(state.contains("run Stop"), "{}", state);
    rgb(&inputs, "single.ppm");
}

pub fn run_host(inputs: &Inputs, script: &str, extra: &[&str]) -> Output {
    std::fs::write(inputs.path("script.txt"), script).expect("isolated script must be writable");
    let mut command = Command::new(env!("CARGO_BIN_EXE_oos-ui"));
    command
        .env_clear()
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .env("HOME", inputs.path("home"))
        .current_dir(inputs.path(""))
        .arg("--headless-script")
        .arg("--script")
        .arg(inputs.path("script.txt"))
        .arg("--store-dir")
        .arg(inputs.path("store"))
        .args(extra);
    support::run_command(inputs, command, 120)
}

#[test]
fn direct_codes_report_the_observed_worker_route() {
    let inputs = isolated_inputs();
    let script = "menu Horizontal\nset Time/div = 5 µs\nframes 2\nshot direct.ppm\n";
    let log = success(&run_host(&inputs, script, &["--scripted-batch", "codes"]));
    assert!(log.contains("newest 46 route workers decode codes"), "{}", log);
}
