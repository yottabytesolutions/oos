// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Signed updates in the GUI (docs/update.md): System > Update shows the update state, System > Roll back update
//! writes the kept image back (two taps within 5 s), the web page /update takes a package (http.rs). The work is
//! done by /bin/oos-update in a child process (the card writes take a few seconds); its result comes as a dialog.
//! A package from the web page is checked in two steps: its header (signature, target, length) in the frame loop,
//! which answers the upload at once; the image hash and the copy to /tmp in a thread of their own, then the child.
//! A notice from the start-up watcher (/tmp/oos-update.notice: rolled back after a failed start) is shown once.

use super::*;
use oos_hal::update::{self, pkg, State};
use std::path::Path;

const BIN: &str = "/bin/oos-update";
const NOTICE: &str = "/tmp/oos-update.notice";
const UPLOAD: &str = "/tmp/oos-update.upload";
const JOB_LOG: &str = "/tmp/oos-update.job";

mod upload;
use upload::{page, upload_header, upload_store};

struct PendingUpload {
    thread: std::thread::JoinHandle<Result<(), String>>,
    build: String,
    any_unit: bool,
}

#[derive(Default)]
pub(super) struct UpdateUi {
    job: Option<(std::process::Child, String)>,
    /// Immutable image identity, resolved once and injectable by host tests, including read errors.
    running_unit: Option<Result<String, String>>,
    /// an uploaded package being checked and stored (`upload_store`) before the install job starts; its build
    upload: Option<PendingUpload>,
    /// first tap on Roll back: the second must come before this time
    arm_until: f64,
    next_check: f64,
    notice_shown: String,
    /// output of the last job (web page)
    pub(super) last: String,
}

/// The update state: None without the update directory (`dir`), else the state file's text, the defaults while no
/// state file was written (`State::load`).
fn state(dir: bool, text: Option<String>) -> Option<State> {
    dir.then(|| text.map_or_else(State::default, |t| State::parse(&t)))
}

impl Ui {
    /// The update state through the host.
    pub(super) fn update_state(&self) -> Option<State> {
        let path = Path::new(update::STATE_PATH);
        state(path.parent().is_some_and(|d| self.host.is_dir(d)), self.host.text(update::STATE_PATH))
    }

    /// Dialog lines of System > Update.
    pub(super) fn update_lines(&self) -> Vec<String> {
        let mut d = vec!["Update".to_string()];
        if let Some(b) = self.host.text("/etc/oos-build") {
            d.push(format!("Running {}", update::short(b.trim())));
        }
        match self.update_state() {
            None => d.push("No update installed yet".into()),
            Some(st) => {
                let b = |e: &Option<update::Entry>| {
                    e.as_ref().map_or("-".to_string(), |e| update::short(&e.build).to_string())
                };
                d.push(format!("Recovery {}{}", b(&st.current), if st.trial.is_some() { " (on trial)" } else { "" }));
                d.push(format!("Rollback image {}", b(&st.previous)));
                if st.trial.is_some() {
                    d.extend(upload::trial_lines(
                        &st,
                        self.host.text("/etc/oos-build").as_deref(),
                        self.host.text("/proc/cmdline").as_deref(),
                    ));
                }
                d.push(format!("Boot mode {}", if st.persist { "persist" } else { "one-shot" }));
                if !st.event.is_empty() {
                    // the event may quote a build name from a package: any characters
                    let mut e: String = st.event.chars().take(43).collect();
                    if e.len() < st.event.len() {
                        e.push('…');
                    }
                    d.push(e);
                }
            }
        }
        if self.upd.upload.is_some() {
            d.push("Checking the package ...".into());
        }
        if self.upd.job.is_some() {
            d.push("Working ...".into());
        }
        d.extend(["".into(), "Install: web page :8080/update".into(), "Tap to close".into()]);
        d
    }

    /// The web page /update.
    pub fn update_page(&self) -> String {
        page(&self.upd.last)
    }

    /// System > Boot mode.
    pub(super) fn boot_mode(&self) -> &'static str {
        if self.update_state().is_some_and(|s| s.persist) {
            "Persist"
        } else {
            "One-shot"
        }
    }

    /// Persist keeps OOS as the system from now on, One-shot makes the next power-on start stock.
    pub(super) fn boot_mode_toggle(&mut self) {
        if self.update_busy() {
            return self.toast("An update job is running");
        }
        let to = if self.update_state().is_some_and(|s| s.persist) { "oneshot" } else { "persist" };
        self.update_spawn(&["mode", to], "Boot mode");
    }

    /// System > Back to stock: says what the next power-on really does.
    pub(super) fn stock_tap(&mut self) {
        if self.update_state().is_some_and(|s| s.persist) {
            self.toast("Boot mode is Persist: set it to One-shot, then power-cycle");
        } else {
            self.toast("Power-cycle the scope: stock boots next");
        }
    }

    /// System > Roll back update: first tap asks, second (within 5 s) runs it.
    pub(super) fn update_rollback_tap(&mut self) {
        if self.update_busy() {
            return self.toast("An update job is running");
        }
        let Some(st) = self.update_state() else {
            return self.toast("No rollback image (no update installed)");
        };
        let Some(prev) = st.previous else { return self.toast("No rollback image kept") };
        if st.current.as_ref().is_some_and(|c| c.sha256 == prev.sha256) {
            return self.toast("Recovery already holds the rollback image");
        }
        if self.now > self.upd.arm_until {
            self.upd.arm_until = self.now + 5.0;
            return self.toast(&format!("Tap again to roll back to {}", update::short(&prev.build)));
        }
        self.upd.arm_until = 0.0;
        self.update_spawn(&["rollback", "--execute"], "Roll back");
    }

    /// A package from the web page (http.rs, after its whole body arrived). The frame loop checks only its header
    /// (`upload_header`) and answers at once; the image hash and the copy to /tmp run in a thread (`upload_store`,
    /// ~31 MB), and `update_poll` starts the install job from there. Mismatch without override: ask for confirmation,
    /// nothing kept. Err: refused, nothing kept.
    pub fn update_upload_reply(&mut self, body: Vec<u8>, query: &str) -> (bool, &'static str, Vec<u8>) {
        let result = upload::any_unit(query).and_then(|any| self.update_upload(body, any));
        upload::reply(result)
    }

    fn update_upload(&mut self, body: Vec<u8>, any_unit: bool) -> Result<upload::Reply, String> {
        if self.update_busy() {
            return Err("an update job is running".into());
        }
        let keys = update::trusted_keys();
        let h = upload_header(&body, &keys)?;
        let running = self.upd.running_unit.get_or_insert_with(update::unit::running).clone()?;
        if let Some(confirmation) = upload::confirmation(&h.manifest, &running, any_unit)? {
            return Ok(confirmation);
        }
        let warning = update::unit::check(&h.manifest, &running, any_unit)?;
        let b = update::short(&h.manifest.build).to_string();
        let t = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || upload_store(&body, &keys, Path::new(UPLOAD)))
            .map_err(|e| format!("no thread for the check ({}), nothing installed", e))?;
        self.upd.upload = Some(PendingUpload { thread: t, build: b.clone(), any_unit });
        self.upd.last = format!("package of build {} received, checking it", b);
        let accepted = upload::accepted(&h.manifest);
        Ok(upload::Reply::Accepted(warning.map_or(accepted.clone(), |w| format!("{}\n{}", w, accepted))))
    }

    /// A job of ours is running, or another process (the start-up watcher) holds the update lock: the card is being
    /// written, so no second job and no power-off from here.
    pub(super) fn update_busy(&self) -> bool {
        self.upd.job.is_some() || self.upd.upload.is_some() || update::busy(std::path::Path::new(update::STATE_PATH))
    }

    /// Starts `oos-update`; false when that failed (toast).
    fn update_spawn(&mut self, args: &[&str], what: &str) -> bool {
        let log = match std::fs::File::create(JOB_LOG) {
            Ok(f) => f,
            Err(e) => {
                self.toast(&format!("{}: {}", JOB_LOG, e));
                return false;
            }
        };
        let err = log.try_clone();
        let mut c = std::process::Command::new(BIN);
        c.args(args).stdout(log).stdin(std::process::Stdio::null());
        if let Ok(e) = err {
            c.stderr(e);
        }
        match c.spawn() {
            Ok(ch) => {
                self.upd.job = Some((ch, what.into()));
                self.toast(&format!("{} ...", what));
                true
            }
            Err(e) => {
                self.toast(&format!("{}: {}", BIN, e));
                false
            }
        }
    }

    /// From `step`: the running job, and the watcher's notice (twice a second).
    pub(super) fn update_poll(&mut self) {
        if self.now < self.upd.next_check {
            return;
        }
        self.upd.next_check = self.now + 0.5;
        if self.upd.upload.as_ref().is_some_and(|u| u.thread.is_finished()) {
            let Some(PendingUpload { thread: t, build: b, any_unit }) = self.upd.upload.take() else { return };
            match t.join().unwrap_or_else(|_| Err("the package check stopped".into())) {
                Ok(()) => {
                    if !self.update_spawn(&upload::install_args(any_unit), "Install") {
                        let _ = std::fs::remove_file(UPLOAD);
                        self.upd.last = format!("build {}: {} could not be started, nothing installed", b, BIN);
                    }
                }
                Err(e) => {
                    self.upd.last = format!("REFUSED: build {}: {}", b, e);
                    self.dialog = Some(vec!["Install REFUSED".into(), e, "Tap to close".into()]);
                }
            }
        }
        if let Some((ch, what)) = &mut self.upd.job {
            if let Ok(Some(st)) = ch.try_wait() {
                let what = what.clone();
                self.upd.job = None;
                let out = std::fs::read_to_string(JOB_LOG).unwrap_or_default();
                self.upd.last = out.clone();
                let mut d = vec![format!("{} {}", what, if st.success() { "done" } else { "FAILED" })];
                d.extend(wrap_lines(&out, 44).into_iter().rev().take(9).collect::<Vec<_>>().into_iter().rev());
                d.push("Tap to close".into());
                self.dialog = Some(d);
                if what == "Install" {
                    let _ = std::fs::remove_file(UPLOAD);
                }
            }
        }
        // Read notices from the host supplied at construction.
        if let Some(n) = self.host.text(NOTICE) {
            let n = n.trim().to_string();
            if !n.is_empty() && n != self.upd.notice_shown && self.dialog.is_none() && !self.start_up_active() {
                self.upd.notice_shown = n.clone();
                let mut d = vec!["Update".to_string()];
                d.extend(wrap_lines(&n, 44));
                d.push("Tap to close".into());
                self.dialog = Some(d);
            }
        }
    }
}

/// Text wrapped at word boundaries to `w` characters.
fn wrap_lines(t: &str, w: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in t.lines().filter(|l| !l.trim().is_empty()) {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > w {
                out.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

#[cfg(test)]
mod text_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_needs_the_update_directory_not_the_state_file() {
        assert!(state(false, None).is_none());
        assert!(state(false, Some("persist = 1\n".into())).is_none());
        let st = state(true, None).expect("the directory alone gives the default state");
        assert!(st.current.is_none() && st.previous.is_none() && !st.persist);
    }

    #[test]
    fn wraps_at_words() {
        let l =
            wrap_lines("Update 20261105-170000 failed its first start (oos-ui crashed 3 times). Build x restored.", 30);
        assert!(l.iter().all(|s| s.chars().count() <= 30), "{:?}", l);
        assert_eq!(
            l.join(" "),
            "Update 20261105-170000 failed its first start (oos-ui crashed 3 times). Build x restored."
        );
    }

    #[test]
    fn foreign_unit_upload_needs_confirmation_before_spawning_a_job() {
        let mut u = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let seed_text = include_str!("../../../keys/community-signing.key");
        let seed_line = seed_text.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).unwrap();
        let seed: [u8; 32] = update::hash::unhex(seed_line.trim()).unwrap().try_into().unwrap();
        let body = pkg::build_with(&seed, "20261115-120000", "k", b"image", &[("unit", "other-unit")]).unwrap();
        u.upd.running_unit = Some(Ok("fake-unit".into()));
        let (refused, kind, reply) = u.update_upload_reply(body, "");
        assert!(refused);
        assert_eq!(kind, "application/json");
        let reply = String::from_utf8(reply).unwrap();
        let reply = crate::json::parse(&reply).unwrap();
        assert_eq!(reply.get("package_unit").and_then(|v| v.str()), Some("other-unit"));
        assert_eq!(reply.get("running_unit").and_then(|v| v.str()), Some("fake-unit"));
        assert_eq!(reply.get("refused"), Some(&crate::json::J::Bool(true)));
        assert!(u.upd.upload.is_none() && u.upd.job.is_none());
    }

    #[test]
    fn unreadable_running_identity_refuses_even_an_overridden_upload() {
        let mut u = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        u.upd.running_unit = Some(Err("identity read failed".into()));
        let text = include_str!("../../../keys/community-signing.key");
        let seed: [u8; 32] = update::hash::unhex(text.lines().find(|l| !l.is_empty() && !l.starts_with('#')).unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let body = pkg::build(&seed, "b", "k", b"image").unwrap();
        let (refused, kind, reply) = u.update_upload_reply(body, "any-unit=1");
        assert!(refused);
        assert_eq!(kind, "text/plain");
        assert!(String::from_utf8(reply).unwrap().contains("identity read failed"));
        assert!(u.upd.upload.is_none() && u.upd.job.is_none());
    }

    #[test]
    fn upload_refuses_unsigned_packages() {
        let mut u = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let e = u.update_upload(b"OOS-UPDATE 1\ntarget dho900-recovery\n\nxx".to_vec(), false).unwrap_err();
        assert!(e.contains("signature") || e.contains("header"), "{}", e);
        assert!(u.upd.job.is_none() && u.upd.upload.is_none());
    }

    // Break it catches: the image hash left to nobody once it moved out of the frame loop, a truncated upload taken by
    // the header check, a refused package still written for the install job.
    #[test]
    fn upload_checks_header_first_and_image_off_the_frame_loop() {
        let seed = [9u8; 32];
        let keys = [update::ed25519::public_key(&seed)];
        let image: Vec<u8> = (0..4096u32).map(|i| (i * 7) as u8).collect();
        let good = pkg::build(&seed, "20261114-150000", "6.12.115+", &image).unwrap();
        let dir = std::env::temp_dir().join(format!("oos-upload-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("upload");
        // whole and signed: header ok, stored
        assert!(upload_header(&good, &keys).is_ok());
        upload_store(&good, &keys, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), good);
        std::fs::remove_file(&path).unwrap();
        // one image byte changed: the header still checks, the image hash refuses it, nothing written
        let mut bad = good.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(upload_header(&bad, &keys).is_ok());
        assert!(upload_store(&bad, &keys, &path).unwrap_err().contains("sha256") || !path.exists());
        assert!(!path.exists());
        // cut short: the header check already refuses it
        assert!(upload_header(&good[..good.len() - 10], &keys).unwrap_err().contains("truncated"));
        // another key
        assert!(upload_header(&good, &[update::ed25519::public_key(&[1; 32])]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
