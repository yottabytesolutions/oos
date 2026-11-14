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

#[derive(Default)]
pub(super) struct UpdateUi {
    job: Option<(std::process::Child, String)>,
    /// an uploaded package being checked and stored (`upload_store`) before the install job starts; its build
    upload: Option<(std::thread::JoinHandle<Result<(), String>>, String)>,
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
    fn update_state(&self) -> Option<State> {
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
    /// ~31 MB), and `update_poll` starts the install job from there. Err: refused, nothing kept.
    pub fn update_upload(&mut self, body: Vec<u8>) -> Result<String, String> {
        if self.update_busy() {
            return Err("an update job is running".into());
        }
        let keys = update::trusted_keys();
        let h = upload_header(&body, &keys)?;
        let b = update::short(&h.manifest.build).to_string();
        let t = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || upload_store(&body, &keys, Path::new(UPLOAD)))
            .map_err(|e| format!("no thread for the check ({}), nothing installed", e))?;
        self.upd.upload = Some((t, b.clone()));
        self.upd.last = format!("package of build {} received, checking it", b);
        Ok(if update::kexec::announced(&h.manifest) {
            format!("accepted, build {}: header signature ok; the image is checked now, then the scope restarts into it by kexec and installs it from there (about a minute; System > Update and this page show the result). Then power-cycle the scope: that start is its trial", b)
        } else {
            format!("accepted, build {}: header signature ok; the image is checked now, then installed (System > Update and this page show the result)", b)
        })
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
        if self.upd.upload.as_ref().is_some_and(|u| u.0.is_finished()) {
            let (t, b) = self.upd.upload.take().unwrap();
            match t.join().unwrap_or_else(|_| Err("the package check stopped".into())) {
                Ok(()) => {
                    if !self.update_spawn(&["install", UPLOAD, "--execute"], "Install") {
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

/// The checks of an uploaded package that the frame loop can afford: header signature against `keys`, target and the
/// image length the header names (a truncated upload fails here). The image hash is `upload_store`'s.
fn upload_header(body: &[u8], keys: &[[u8; 32]]) -> Result<pkg::Header, String> {
    let h = pkg::verify_header(body, keys)?;
    let m = &h.manifest;
    if m.target != pkg::TARGET {
        return Err(format!("package is for '{}', this unit takes '{}'", m.target, pkg::TARGET));
    }
    let img = body.len().saturating_sub(h.data_offset) as u64;
    if img != m.size {
        return Err(format!("image is {} bytes, the header says {} (truncated upload?)", img, m.size));
    }
    Ok(h)
}

/// The whole check (`pkg::verify`: also the image hash), then the package to `path` for `oos-update install`. Off the
/// frame loop. Err: nothing written.
fn upload_store(body: &[u8], keys: &[[u8; 32]], path: &Path) -> Result<(), String> {
    pkg::verify(body, keys)?;
    std::fs::write(path, body).map_err(|e| format!("{}: {}", path.display(), e))
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

/// The web page /update: state and an upload form.
fn page(last: &str) -> String {
    let status = std::process::Command::new(BIN)
        .args(["status", "--quick"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
        .unwrap_or_else(|e| format!("{}: {}", BIN, e));
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OOS update</title>
<style>{}</style></head>
<body><header><b>OOS update</b><nav><a href="/">Remote</a></nav></header>
<pre>{}</pre>
<p>Package (<code>.oosupd</code>, signed): <input type="file" id="f" accept=".oosupd"> <button onclick="up()">Install</button></p>
<pre id="r">{}</pre>
<p>The new build starts on trial at the next power-on; a failed start writes the previous build back.
Command line: <code>curl --max-time 300 --data-binary @oos-dev.oosupd http://&lt;scope&gt;:8080/update</code>
(the answer comes once the package has arrived and its header is checked; the image check and the install follow, their result shows here).</p>
<script>
function up(){{const f=document.getElementById('f').files[0];if(!f)return;const r=document.getElementById('r');
r.textContent='uploading '+f.size+' bytes ...';
fetch('/update',{{method:'POST',body:f}}).then(x=>x.text()).then(t=>{{r.textContent=t;setTimeout(()=>location.reload(),15000)}})}}
</script></body></html>"#,
        crate::http::STYLE,
        esc(&status),
        esc(last)
    )
}

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
    fn upload_refuses_unsigned_packages() {
        let mut u = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let e = u.update_upload(b"OOS-UPDATE 1\ntarget dho900-recovery\n\nxx".to_vec()).unwrap_err();
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
