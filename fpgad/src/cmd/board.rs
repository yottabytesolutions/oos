// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

//! Board subcommands: temperatures, beeper, over-temperature guard (oos_hal::dho::board, docs/protocols/board.md).

#[cfg(target_os = "linux")]
use oos_hal::dho::board;
use oos_hal::dho::board::Zone;
use oos_hal::dho::panel;

/// FPGA die temperature and VCCINT, if BAR1 is there and the FPGA answers.
pub(super) fn fpga_xadc() -> Option<(f32, f32)> {
    #[cfg(target_os = "linux")]
    {
        let mut bar = oos_hal::dho::bus::Bar1::open().ok()?;
        board::read_xadc(&mut bar)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// One line of readings.
pub(super) fn line(zones: &[Zone], fpga: Option<(f32, f32)>) -> String {
    let mut s: Vec<String> = zones.iter().map(|z| format!("{} {:.1} C", z.name, z.temp)).collect();
    match fpga {
        Some((t, v)) => s.push(format!("fpga {:.1} C, vccint {:.3} V", t, v)),
        None => s.push("fpga -".into()),
    }
    s.join(", ")
}

/// Send both thermal power-off frames before attempting any storage work.
pub(super) fn power_off() -> Result<(), String> {
    use std::io::Write;

    let mut panel = None;
    thermal_power_off(
        || {
            let port = match &mut panel {
                Some(port) => port,
                None => panel.insert(std::fs::OpenOptions::new().write(true).open("/dev/ttyS0")?),
            };
            port.write_all(&panel::POWER_OFF)
        },
        || {
            #[cfg(target_os = "linux")]
            if let Err(error) = std::process::Command::new("/bin/sync").stdin(std::process::Stdio::null()).spawn() {
                eprintln!("oos-fpgad guard: could not start sync: {error}");
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        },
        |message| {
            #[cfg(target_os = "linux")]
            oos_hal::system::report_power_failure(message);
            #[cfg(not(target_os = "linux"))]
            eprintln!("{message}");
        },
    )
}

fn thermal_power_off(
    mut write: impl FnMut() -> std::io::Result<()>,
    wait_for_power_loss: impl FnOnce(),
    report: impl FnOnce(&str),
) -> Result<(), String> {
    let first = write();
    let second = write();
    let failure = match (first, second) {
        (Err(first), Err(second)) => format!("Thermal power-off failed: panel UART writes failed ({first}; {second})."),
        _ => {
            wait_for_power_loss();
            "Thermal power-off failed: the scope is still running.".into()
        }
    };
    report(&failure);
    Err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermal_power_retries_before_any_sync_and_reports_a_still_running_scope() {
        use std::cell::RefCell;
        let calls = RefCell::new(Vec::new());
        let mut writes = [Err(std::io::Error::other("first write")), Ok(())].into_iter();
        let error = thermal_power_off(
            || {
                calls.borrow_mut().push("MCU power");
                writes.next().unwrap()
            },
            || calls.borrow_mut().push("start sync and wait"),
            |notice| {
                assert!(notice.contains("still running"));
                calls.borrow_mut().push("report");
            },
        )
        .unwrap_err();
        assert!(error.contains("Thermal power-off failed"));
        assert_eq!(*calls.borrow(), ["MCU power", "MCU power", "start sync and wait", "report"]);
    }

    #[test]
    fn thermal_power_reports_both_write_errors_immediately() {
        let mut writes = ["first write", "second write"].into_iter();
        let error = thermal_power_off(
            || Err(std::io::Error::other(writes.next().unwrap())),
            || panic!("both writes failed"),
            |notice| assert!(notice.contains("panel UART writes failed")),
        )
        .unwrap_err();
        assert!(error.contains("first write") && error.contains("second write"));
    }

    #[test]
    fn readings_line() {
        let z = vec![Zone { name: "cpu-thermal".into(), temp: 45.25 }];
        assert_eq!(line(&z, Some((51.0, 0.998))), "cpu-thermal 45.2 C, fpga 51.0 C, vccint 0.998 V");
        assert_eq!(line(&[], None), "fpga -");
    }
}
