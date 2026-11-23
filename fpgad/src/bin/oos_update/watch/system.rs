// SPDX-License-Identifier: GPL-2.0-or-later

use super::{publish_to, watch, Outcome, Ports};
use crate::oos_update::messages;
use crate::{notice, put, state_path, tell, ui_pid, Args, STDOUT_CLOSED};
use oos_hal::bootcard;
use oos_hal::update::{self, kexec, Health, State, Verdict};
use std::path::Path;

pub(crate) fn run(args: &Args) {
    let path = state_path(args);
    let state = State::load(&path).unwrap_or_default();
    let kexec = kexec::flags(&kexec::running_cmdline()).is_some();
    if kexec {
        tell(
            "kexec start: not judged; an installed trial needs an ordinary restart. Follow the install/restart notice",
        );
    }
    let running = (!kexec).then(update::running_build).flatten();
    watch(&mut System { args, path: &path }, &state, kexec, running.as_deref());
}

struct System<'a> {
    args: &'a Args,
    path: &'a Path,
}

impl Ports for System<'_> {
    fn verdict(&mut self) -> Outcome {
        let status = self.args.val("--status").unwrap_or("/tmp/oos-boot.status");
        await_verdict(status, Path::new("/etc/oos-autoacq").exists())
    }

    fn confirm(&mut self, state: &State, sha: [u8; 32]) {
        confirm(self.path, state, sha);
    }

    fn arm(&mut self) -> bool {
        arm(self.args, self.path)
    }

    fn rollback(&mut self, state: &State, sha: [u8; 32], why: &str) {
        rollback(self.args, self.path, state, sha, why);
    }

    fn unhealthy(&mut self, why: &str) {
        tell(&format!("start not clean ({why}); OOS not armed again, the next power-on boots stock"));
    }

    fn publish(&mut self, outcome: &str) {
        if let Err(error) = publish_to(Path::new(update::JUDGED_PATH), outcome) {
            tell(&format!("write start-up judgment: {error}"));
        }
    }
}

fn await_verdict(status: &str, autoacq: bool) -> Outcome {
    let t0 = std::time::Instant::now();
    let (mut pid, mut since, mut restarts, mut seen) = (None::<u32>, 0.0f64, 0u32, false);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let now = t0.elapsed().as_secs_f64();
        if Path::new("/tmp/oos-ui.off").exists() {
            tell("oos-ui stopped by hand (/tmp/oos-ui.off): start not judged, trial left open (oos-update confirm)");
            return Outcome::UiOff;
        }
        let cur = ui_pid(pid);
        if cur != pid {
            if cur.is_some() {
                if seen {
                    restarts += 1;
                }
                seen = true;
                since = now;
            }
            pid = cur;
        }
        let startup_done = !autoacq
            || std::fs::read_to_string(status)
                .is_ok_and(|t| t.lines().any(|l| matches!(l.split('\t').nth(1), Some("end") | Some("abort"))));
        let h = Health {
            elapsed: now,
            ui_restarts: restarts,
            ui_up: if pid.is_some() { now - since } else { 0.0 },
            startup_done,
        };
        match update::judge(&h) {
            Verdict::Wait => continue,
            Verdict::Good => return Outcome::Good,
            Verdict::Bad(why) => return Outcome::Bad(why),
        }
    }
}

fn confirm(path: &Path, st: &State, sha: [u8; 32]) {
    let observed = bootcard::read_state(Path::new(bootcard::STATE_PATH)).mode();
    match update::confirm_with_sd_mode(path, sha, observed) {
        Ok(true) => tell(&format!(
            "build {} confirmed (trial passed)",
            st.current.as_ref().map_or("?", |c| update::short(&c.build))
        )),
        Ok(false) => tell("start was good, but the update state changed meanwhile: nothing confirmed"),
        Err(e) => tell(&e),
    }
}

fn arm(args: &Args, path: &Path) -> bool {
    match args.open_card(true).and_then(|mut card| update::arm_checked(&mut card, path)) {
        Ok(changed) => {
            if changed {
                tell("persist mode: OOS armed for the next power-on");
            }
            true
        }
        Err(e) => {
            tell(&format!("persist mode: {e}"));
            false
        }
    }
}

fn rollback(a: &Args, path: &Path, st: &State, sha: [u8; 32], why: &str) {
    match a.open_card(true).and_then(|mut card| update::rollback(&mut card, path, why, true, Some(sha))) {
        Ok(r) => {
            r.lines.iter().for_each(|l| out!("{}", l));
            notice(&messages::failed_trial(st.current.as_ref().map_or("?", |c| c.build.as_str()), why, &r));
        }
        Err(e) => notice(&messages::trial_restore_failed(why, &e)),
    }
}
