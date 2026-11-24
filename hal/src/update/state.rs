// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    card::Region,
    hash::{hex, unhex},
    io, lock, sd, short, Card, Entry, State, SLOT_WRITING,
};
use std::path::Path;

pub(super) fn write_slot(
    card: &mut Card,
    state_path: &Path,
    image: &[u8],
    entry: Entry,
    state: &mut State,
) -> Result<(), String> {
    // While the slot is being written it holds no valid image.
    state.previous = None;
    state.staged = None;
    state.slot_incomplete = true;
    state.event = SLOT_WRITING.into();
    state.save(state_path).map_err(io("writing the update state"))?;
    card.write_image(Region::Slot, image)?;
    state.previous = Some(entry);
    state.slot_incomplete = false;
    state.save(state_path).map_err(io("writing the update state"))
}

impl State {
    pub fn parse(text: &str) -> State {
        let mut s = State::default();
        let mut cur = (String::new(), None, 0u64);
        let mut prev = (String::new(), None, 0u64);
        for l in text.lines() {
            let Some((k, v)) = l.split_once(' ') else { continue };
            let v = v.trim();
            match k {
                "current.build" => cur.0 = v.into(),
                "current.sha256" => cur.1 = unhex(v).and_then(|b| b.try_into().ok()),
                "current.len" => cur.2 = v.parse().unwrap_or(0),
                "previous.build" => prev.0 = v.into(),
                "previous.sha256" => prev.1 = unhex(v).and_then(|b| b.try_into().ok()),
                "previous.len" => prev.2 = v.parse().unwrap_or(0),
                "trial" => s.trial = v.parse().ok(),
                "sd.failed" => s.sd_failed = v == "yes",
                "sd.trial" => {
                    s.sd_trial = match v {
                        "hs" => Some(sd::Mode::Hs),
                        "sdr104" => Some(sd::Mode::Sdr104),
                        _ => None,
                    }
                }
                "mode" => s.persist = v == "persist",
                "staged" => s.staged = staged_entry(v),
                "slot.incomplete" => s.slot_incomplete = v == "yes",
                "kexec" if !v.is_empty() => s.kexec = Some(v.into()),
                "kexec.bytes" => s.kexec_payload = v.parse().ok(),
                "event" => s.event = v.into(),
                _ => {}
            }
        }
        let entry = |(build, sha, len): (String, Option<[u8; 32]>, u64)| {
            sha.filter(|_| len > 0).map(|sha256| Entry { build, sha256, len })
        };
        s.current = entry(cur);
        s.previous = entry(prev);
        if s.staged.is_some() {
            s.previous = None;
            s.slot_incomplete = true;
        }
        if s.kexec.is_none() {
            s.kexec_payload = None;
        }
        // Older updates recorded an interrupted slot write only in the event.
        s.slot_incomplete |= s.previous.is_none() && s.event == SLOT_WRITING;
        s
    }

    pub fn text(&self) -> String {
        let mut t = String::from("# OOS update state (oos_hal::update, docs/update.md)\n");
        for (n, e) in [("current", &self.current), ("previous", &self.previous)] {
            if let Some(e) = e {
                t.push_str(&format!("{0}.build {1}\n{0}.sha256 {2}\n{0}.len {3}\n", n, e.build, hex(&e.sha256), e.len));
            }
        }
        if let Some(e) = &self.staged {
            t.push_str(&format!("staged {} {} {}\n", e.build, hex(&e.sha256), e.len));
        }
        if self.slot_incomplete {
            t.push_str("slot.incomplete yes\n");
        }
        if let Some(n) = self.trial {
            t.push_str(&format!("trial {}\n", n));
        }
        if self.sd_failed {
            t.push_str("sd.failed yes\n");
        }
        if let Some(mode) = self.sd_trial {
            t.push_str(if mode == sd::Mode::Hs { "sd.trial hs\n" } else { "sd.trial sdr104\n" });
        }
        t.push_str(if self.persist { "mode persist\n" } else { "mode oneshot\n" });
        if let Some(b) = &self.kexec {
            t.push_str(&format!("kexec {}\n", b.replace('\n', " ")));
            if let Some(bytes) = self.kexec_payload {
                t.push_str(&format!("kexec.bytes {bytes}\n"));
            }
        }
        if !self.event.is_empty() {
            t.push_str(&format!("event {}\n", self.event.replace('\n', " ")));
        }
        t
    }

    pub(super) fn take_kexec_payload(&mut self) -> String {
        self.kexec_payload.take().map_or_else(String::new, |bytes| format!(" ({bytes} bytes kexec payload)"))
    }

    /// Text explaining why the slot cannot supply a rollback image, when one is unavailable.
    pub fn rollback_slot_status(&self) -> Option<String> {
        self.previous.is_none().then(|| self.missing_rollback())
    }

    pub(super) fn missing_rollback(&self) -> String {
        if let Some(candidate) = &self.staged {
            let build = short(&candidate.build);
            if self.kexec.is_some() {
                format!("slot holds a staged candidate for build {build}, no rollback image")
            } else {
                format!("slot reserved for staged candidate {build}, no rollback image")
            }
        } else if self.slot_incomplete {
            "no rollback image (the slot was used for an update)".into()
        } else {
            "no rollback image (nothing was kept by an update)".into()
        }
    }

    pub fn load(path: &Path) -> std::io::Result<State> {
        match std::fs::read_to_string(path) {
            Ok(t) => Ok(State::parse(&t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(e),
        }
    }

    /// Atomic: temp file, fsync, rename.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("new");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(self.text().as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        if let Some(d) = path.parent() {
            if let Ok(f) = std::fs::File::open(d) {
                let _ = f.sync_all();
            }
        }
        Ok(())
    }
    pub(super) fn fail_sd_trial(&mut self) {
        if self.sd_trial == Some(sd::Mode::Sdr104) {
            self.sd_failed = true;
        }
        self.sd_trial = None;
    }

    pub(super) fn sd_fallback(&self) -> Option<&'static str> {
        self.sd_failed.then_some("SDR104 trial was not confirmed: falling back to hs; select hs before retrying")
    }
    pub(super) fn missed_sd_trial(&mut self) -> &'static str {
        if self.sd_trial == Some(sd::Mode::Sdr104) {
            self.fail_sd_trial();
            "; SDR104 was not confirmed: falling back to hs"
        } else {
            ""
        }
    }
}

fn staged_entry(text: &str) -> Option<Entry> {
    let (build_and_hash, len) = text.rsplit_once(' ')?;
    let (build, sha) = build_and_hash.rsplit_once(' ')?;
    let len = len.parse().ok()?;
    let sha256 = unhex(sha)?.try_into().ok()?;
    (!build.is_empty() && len > 0).then(|| Entry { build: build.into(), sha256, len })
}

/// The image passed its trial. Returns true when it was on trial. `expect`: only when the state still names this
/// image as current.
pub fn confirm(state_path: &Path, expect: Option<[u8; 32]>) -> Result<bool, String> {
    confirm_running(state_path, expect, None)
}

/// `confirm`, and with `running` (the build of the running image) only if that is the build on trial: an image that was
/// written but never started is not confirmed.
pub fn confirm_running(state_path: &Path, expect: Option<[u8; 32]>, running: Option<&str>) -> Result<bool, String> {
    confirm_start(state_path, expect, running, None)
}

/// Confirm a watched start and include the observed card mode in the SDR104 trial event.
pub fn confirm_with_sd_mode(state_path: &Path, expect: [u8; 32], mode: crate::bootcard::Mode) -> Result<bool, String> {
    confirm_start(state_path, Some(expect), None, Some(mode))
}

fn confirm_start(
    state_path: &Path,
    expect: Option<[u8; 32]>,
    running: Option<&str>,
    observed: Option<crate::bootcard::Mode>,
) -> Result<bool, String> {
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    if st.trial.is_none() {
        return Ok(false);
    }
    if st.sd_trial.is_some() && st.trial == Some(0) {
        return Err("SD mode trial has not booted yet: not confirmed".into());
    }
    if let Some(run) = running {
        let cur = st.current.as_ref().map(|c| c.build.trim().to_string()).unwrap_or_default();
        if run.trim() != cur {
            return Err(format!(
                "the running build {} is not the one on trial ({}): it has not started yet",
                run.trim(),
                cur
            ));
        }
    }
    if let Some(x) = expect {
        if st.current.as_ref().map(|c| c.sha256) != Some(x) {
            return Ok(false);
        }
    }
    st.trial = None;
    st.event = format!("confirmed {}", st.current.as_ref().map_or("?", |c| short(&c.build)));
    if let Some(mode) = observed.filter(|_| st.sd_trial == Some(sd::Mode::Sdr104)) {
        st.event.push_str(" (SDR104 trial, observed ");
        st.event.push_str(mode.as_str());
        st.event.push(')');
    }
    st.sd_trial = None;
    st.save(state_path).map_err(io("writing the update state"))?;
    Ok(true)
}
