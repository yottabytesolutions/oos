// SPDX-License-Identifier: GPL-2.0-or-later

//! Shared user guidance for the caller-controlled start and watcher-controlled trial.

/// Direct installation writes and arms the image; it never restarts the scope.
pub const DIRECT_START: &str = "No automatic restart: power-cycle the scope to start the installed build on trial.";

/// The kexec session installs, then requests an ordinary warm restart before judging the trial.
pub const KEXEC_START: &str = "The scope restarts by kexec to install, then restarts automatically into the trial; no power cycle is normally needed. If the restart is refused or fails, follow the panel notice.";

/// `judge` measures UI uptime and watcher elapsed time, not time since upload.
pub fn trial() -> String {
    format!(
        "The trial confirms automatically after the UI runs {:.0} s continuously and startup finishes; if startup stays incomplete, a stable UI is accepted at {:.0} s of watching. Three UI restarts or no stable UI by then triggers failure handling.",
        super::UI_STABLE_S,
        super::START_TIMEOUT_S
    )
}

/// Used by both direct installs and the installer running inside a kexec start.
pub fn installed(build: &str) -> String {
    format!(
        "build {} installed and armed (OOSK); its next ordinary start is the trial. {}",
        super::short(build),
        trial()
    )
}

/// A rollback writes the old image but leaves the running kernel unchanged.
pub fn restored(build: &str) -> String {
    format!(
        "build {} restored and armed (OOSK). No automatic restart: power-cycle the scope to start it.",
        super::short(build)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_guidance_pins_the_watchers_timing_and_failure_conditions() {
        assert_eq!(trial(), "The trial confirms automatically after the UI runs 20 s continuously and startup finishes; if startup stays incomplete, a stable UI is accepted at 180 s of watching. Three UI restarts or no stable UI by then triggers failure handling.");
        assert_eq!(super::super::UI_RESTARTS_BAD, 3);
    }
}
