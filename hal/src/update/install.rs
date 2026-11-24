// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    check_image, check_slot, hash, io, lock, models, sd, short, slot_holds, state, text, Card, Entry, InstallOpt,
    Region, Report, State,
};
use std::path::Path;

/// Install a verified package from the kexec receiver, including compatibility with older senders.
pub fn install(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
) -> Result<Report, String> {
    install_with(card, state_path, package, keys, opt, false)
}

/// Install directly from the running build once its trial is confirmed.
pub fn install_confirmed(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
) -> Result<Report, String> {
    install_with(card, state_path, package, keys, opt, true)
}

fn install_with(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
    from_running_build: bool,
) -> Result<Report, String> {
    let (header, image, mut report) = models::checked_package(package, keys, opt)?;
    validate_image(card, image, &mut report)?;
    let _lock = if opt.execute { Some(lock(state_path)?) } else { None };
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    if from_running_build {
        check_can_start(&state)?;
    }
    let mode = sd::configured(state_path, &state)?;
    let build = &header.manifest.build;
    let Some(plan) = prepare(card, image, build, &state, mode, opt, &mut report)? else {
        return Ok(report);
    };
    if opt.execute {
        apply_plan(card, state_path, plan, build, &mut state, mode, &mut report)?;
    } else {
        report.say("dry run: nothing written (--execute)");
    }
    Ok(report)
}

pub(super) fn apply_plan(
    card: &mut Card,
    state_path: &Path,
    plan: Plan,
    build: &str,
    state: &mut State,
    mode: sd::Mode,
    report: &mut Report,
) -> Result<(), String> {
    // Verify state is writable before touching the card.
    state.save(state_path).map_err(io("writing the update state (nothing written to the card)"))?;
    if let Some((bytes, entry)) = plan.rollback {
        state::write_slot(card, state_path, &bytes, entry, state)?;
        report.say("rollback slot written and read back");
    }
    state.sd_trial = (mode == sd::Mode::Sdr104).then_some(mode);
    if sd::requested(state_path)? == sd::Mode::Hs {
        state.sd_failed = false;
    }
    write_trial(card, state_path, &plan.image, build, state, report)
}

pub(super) fn check_can_start(state: &State) -> Result<(), String> {
    check_trial(state)?;
    if state.kexec.is_some() {
        return Err("an update is already starting by kexec".into());
    }
    Ok(())
}

pub(super) fn check_trial(state: &State) -> Result<(), String> {
    if state.trial.is_some() {
        return Err("the image in recovery is still on trial: wait for confirmation before installing".into());
    }
    Ok(())
}

pub(super) fn validate_image(card: &mut Card, image: &[u8], report: &mut Report) -> Result<(), String> {
    let info = check_image(image)?;
    if info.len != image.len() {
        return Err(format!("image is {} bytes, its sections end at {}", image.len(), info.len));
    }
    report.say(format!("image: '{}', loader id {} ok", info.name, &hash::hex(&info.id)[..16]));
    card.check_layout()
}

pub(super) struct Plan {
    pub(super) image: Vec<u8>,
    pub(super) rollback: Option<(Vec<u8>, Entry)>,
}

pub(super) fn prepare(
    card: &mut Card,
    image: &[u8],
    build: &str,
    state: &State,
    mode: sd::Mode,
    opt: &InstallOpt,
    report: &mut Report,
) -> Result<Option<Plan>, String> {
    check_downgrade(build, state, opt)?;
    let image = sd::image(image, mode)?;
    let recovery = current_recovery(card, build, hash::sha256(&image), opt.force, report)?;
    if report.unchanged {
        return Ok(None);
    }
    let rollback = plan_rollback(card, state, recovery, report)?;
    if rollback.is_some() {
        check_slot(card, state)?;
    } else if state.previous.is_none() {
        if mode == sd::Mode::Sdr104 {
            return Err("SDR104 needs a confirmed HS rollback image: nothing written".into());
        }
        report.say("WARNING: no rollback image; a failed start falls back to stock Android only");
    }
    report.say(format!("write build {} to recovery, on trial; arm OOS for the next power-on", short(build)));
    Ok(Some(Plan { image, rollback }))
}

fn check_downgrade(build: &str, state: &State, opt: &InstallOpt) -> Result<(), String> {
    if let Some(current) = state.current.as_ref().filter(|c| !c.build.starts_with("untracked")) {
        if current.build_key() > short(build) && !opt.downgrade {
            return Err(format!(
                "build {} is older than the installed {} (give --downgrade to install it anyway)",
                short(build),
                short(&current.build)
            ));
        }
    }
    Ok(())
}

type RecoveryImage = (Vec<u8>, [u8; 32]);

fn current_recovery(
    card: &mut Card,
    build: &str,
    candidate_hash: [u8; 32],
    force: bool,
    report: &mut Report,
) -> Result<Option<RecoveryImage>, String> {
    let Some(header) = card.header(Region::Recovery).map_err(io("reading recovery"))? else {
        report.say("recovery holds no OOS image: nothing to keep");
        return Ok(None);
    };
    let (bytes, sha) = card.read_image(Region::Recovery, header.len).map_err(io("reading recovery"))?;
    if sha == candidate_hash {
        // Rewriting the same image ordinarily keeps the existing rollback image.
        if !force {
            report
                .say(format!("recovery already holds build {}: nothing to do (--force writes it again)", short(build)));
            report.unchanged = true;
        }
        return Ok(None);
    }
    match check_image(&bytes) {
        Ok(_) => Ok(Some((bytes, sha))),
        Err(e) => {
            report.say(format!("recovery holds a damaged image ({}): not kept", e));
            Ok(None)
        }
    }
}

fn plan_rollback(
    card: &mut Card,
    state: &State,
    recovery: Option<RecoveryImage>,
    report: &mut Report,
) -> Result<Option<(Vec<u8>, Entry)>, String> {
    let Some((bytes, sha)) = recovery else { return Ok(None) };
    if let Some(starts) = state.trial {
        report.say(format!(
            "the image in recovery is on trial ({} starts, not confirmed): not kept, the slot keeps {}",
            starts,
            state.previous.as_ref().map_or("nothing", |p| short(&p.build))
        ));
        return Ok(None);
    }
    let saved = sd::image(&bytes, sd::Mode::Hs)?;
    let entry = Entry {
        build: match &state.current {
            Some(current) if current.sha256 == sha => current.build.clone(),
            _ => format!("untracked-{}", &hash::hex(&sha)[..12]),
        },
        sha256: hash::sha256(&saved),
        len: saved.len() as u64,
    };
    if state.previous.as_ref().is_some_and(|p| p.sha256 == entry.sha256) && slot_holds(card, &entry)? {
        report.say(format!("rollback slot already holds build {}", short(&entry.build)));
        return Ok(None);
    }
    report.say(format!("keep build {} ({} bytes) in the {}", short(&entry.build), entry.len, Region::Slot.name()));
    Ok(Some((saved, entry)))
}

pub(super) fn write_trial(
    card: &mut Card,
    state_path: &Path,
    img: &[u8],
    build: &str,
    st: &mut State,
    r: &mut Report,
) -> Result<(), String> {
    let old = st.current.clone();
    // stock for the next power-on while recovery is not a valid image (`Card::disarm_oos`); armed again below
    card.disarm_oos()?;
    // recorded before the write: a failed or cut write leaves a state that names the new image on trial (a rollback or a
    // new install repairs it)
    st.current = Some(Entry { build: build.into(), sha256: hash::sha256(img), len: img.len() as u64 });
    st.trial = Some(0);
    st.staged = None;
    st.event = format!("writing build {}", short(build));
    st.save(state_path).map_err(io("writing the update state"))?;
    card.write_image(Region::Recovery, img)?;
    r.written = true;
    // this was the install a kexec start was for (or one that replaces it); a cut one keeps the mark, which the next
    // ordinary start reports
    st.kexec = None;
    let payload = st.take_kexec_payload();
    st.event = format!(
        "installed {} over {}{payload}",
        short(build),
        old.as_ref().map_or("an untracked image".into(), |o| short(&o.build).to_string())
    );
    st.save(state_path).map_err(io("writing the update state"))?;
    r.say("recovery written and read back");
    match card.arm_oos() {
        Ok(_) => r.say(text::installed(build)),
        Err(e) => r.say(format!("WARNING: {}; start OOS with oos-boot-oos-again", e)),
    }
    Ok(())
}
