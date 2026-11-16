// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::update::{self, Report};

pub(crate) fn installed(build: &str) -> String {
    format!(
        "Update {} installed after its kexec start; requesting an automatic restart into the trial. {}",
        update::short(build),
        update::text::trial()
    )
}

pub(crate) fn no_restart(build: &str, why: &str, trial_ready: bool) -> String {
    let next = if trial_ready {
        "Power-cycle the scope for the trial start."
    } else {
        "Trial start is not verified; check boot selection and trial state with oos-update status before restarting."
    };
    format!(
        "Update {} installed; no automatic restart ({}). {} {}",
        update::short(build),
        why,
        next,
        update::text::trial()
    )
}

/// Keep the HAL's actual arming result, including a warning or an already-restored image.
pub(crate) fn restore_result(context: &str, r: &Report) -> String {
    let last = r.lines.last().map_or("Check update status before restarting.", String::as_str);
    let restart = if last.contains("No automatic restart") { "" } else { "No automatic restart. " };
    format!("{}. {}{}", context, restart, last)
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn failed_trial(build: &str, why: &str, r: &Report) -> String {
    restore_result(&format!("Update {} failed its first start ({})", update::short(build), why), r)
}

pub(crate) fn restore_failed(why: &str) -> String {
    format!("rollback FAILED: {}. No automatic restart; boot selection is unverified. Inspect update status before restarting.", why)
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn trial_restore_failed(why: &str, error: &str) -> String {
    format!("Update failed its first start ({}), {}", why, restore_failed(error))
}

pub(crate) fn not_installed(why: &str) -> String {
    format!("Update started by kexec, but {}: not installed. No automatic restart: power-cycle to return to the previous build (or stock in one-shot mode)", why)
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn failed_before_start(build: &str, why: &str) -> String {
    format!("Update {}: the kexec start failed ({}); nothing installed, this build keeps running", build, why)
}

pub(crate) fn no_restore(began: bool) -> &'static str {
    if began {
        "No rollback image kept. No automatic restart; inspect update status before restarting."
    } else {
        "No automatic restart: power-cycle to return to the previous build (or stock in one-shot mode)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kexec_success_requests_the_automatic_trial_and_gives_watch_timing() {
        assert_eq!(
            installed("b"),
            format!(
                "Update b installed after its kexec start; requesting an automatic restart into the trial. {}",
                update::text::trial()
            )
        );
        assert_eq!(no_restart("b", "reboot returned", true), format!("Update b installed; no automatic restart (reboot returned). Power-cycle the scope for the trial start. {}", update::text::trial()));
    }

    #[test]
    fn a_refused_restart_does_not_promise_an_unarmed_trial() {
        assert_eq!(no_restart("b", "not OOSK", false), format!("Update b installed; no automatic restart (not OOSK). Trial start is not verified; check boot selection and trial state with oos-update status before restarting. {}", update::text::trial()));
    }

    #[test]
    fn failures_distinguish_returning_to_the_old_session_from_an_incomplete_install() {
        assert_eq!(
            failed_before_start("b", "load failed"),
            "Update b: the kexec start failed (load failed); nothing installed, this build keeps running"
        );
        for why in ["/oos is not there", "its package did not arrive"] {
            assert_eq!(not_installed(why), format!("Update started by kexec, but {}: not installed. No automatic restart: power-cycle to return to the previous build (or stock in one-shot mode)", why));
        }
        assert_eq!(
            no_restore(true),
            "No rollback image kept. No automatic restart; inspect update status before restarting."
        );
        assert_eq!(
            no_restore(false),
            "No automatic restart: power-cycle to return to the previous build (or stock in one-shot mode)"
        );
    }

    #[test]
    fn restore_notices_preserve_arming_warnings_instead_of_promising_the_old_build() {
        for (last, guidance) in [
            (update::text::restored("old"), update::text::restored("old")),
            ("WARNING: arm failed".into(), "No automatic restart. WARNING: arm failed".into()),
            (
                "recovery already holds build old".into(),
                "No automatic restart. recovery already holds build old".into(),
            ),
        ] {
            let r = Report { lines: vec![last.clone()], ..Report::default() };
            assert_eq!(restore_result("Update failed", &r), format!("Update failed. {}", guidance));
            assert_eq!(restore_result("Update failed", &r).matches("No automatic restart").count(), 1);
            assert_eq!(
                failed_trial("new", "UI crashed", &r),
                format!("Update new failed its first start (UI crashed). {}", guidance)
            );
        }
        assert_eq!(
            restore_result("Update failed", &Report::default()),
            "Update failed. No automatic restart. Check update status before restarting."
        );
        assert_eq!(
            trial_restore_failed("UI crashed", "write error"),
            format!("Update failed its first start (UI crashed), {}", restore_failed("write error"))
        );
        assert_eq!(restore_failed("write error"), "rollback FAILED: write error. No automatic restart; boot selection is unverified. Inspect update status before restarting.");
    }
}
