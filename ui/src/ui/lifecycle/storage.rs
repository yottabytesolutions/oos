// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-side storage admission and ordered completion delivery.
use super::super::{MathFamily, MathOp, Ui, NREF};
use crate::store::durable::Disk;
use crate::store::startup::{
    worker::{Done, Worker},
    Startup,
};
use std::io;

pub(crate) fn get(ui: &mut Ui, startup: Option<Startup>) -> io::Result<&mut Worker> {
    if ui.host.runtime().storage.is_none() {
        ui.host.runtime_mut().storage = Some(Worker::spawn(Disk, startup)?);
    }
    ui.host.runtime_mut().storage.as_mut().ok_or_else(|| io::Error::other("storage unavailable"))
}

pub(in crate::ui) fn save(ui: &mut Ui) {
    let path = ui.setup_path();
    let text = format!("{}{}", crate::store::setup_text(&ui.m), ui.ref_setup_text());
    if let Err(error) = get(ui, None).and_then(|worker| worker.save(path.clone(), text)) {
        ui.toast_saved(&path, Err(error));
    }
}

pub(in crate::ui) fn poll(ui: &mut Ui, wait: bool) -> io::Result<()> {
    let mut first = Ok(());
    while let Some(done) = ui.host.runtime_mut().storage.as_mut().and_then(|worker| worker.poll(wait)) {
        let error = match &done {
            Done::Save(_, result) | Done::Automatic(result) => result.as_ref().err(),
            Done::Recall(_, result) => result.as_ref().err(),
        };
        if let (Ok(()), Some(error)) = (&first, error) {
            first = Err(io::Error::new(error.kind(), error.to_string()));
        }
        match done {
            Done::Save(path, result) => ui.toast_saved(&path, result),
            Done::Recall(path, result) => {
                crate::ui::change::intake(ui, |ui| apply_recalled_setup(ui, &path, result));
            }
            Done::Automatic(Err(error)) => eprintln!("oos-ui: persist: {}", error),
            Done::Automatic(Ok(())) => {}
        }
    }
    first
}

pub(crate) fn finish(ui: &mut Ui, now: std::time::Duration, power: bool) -> io::Result<()> {
    if ui.exit_code() != 0 {
        return get(ui, None)?.flush();
    }
    let pending = poll(ui, true);
    let text = crate::store::setup_text(ui.setup_model());
    let finished = get(ui, None)?.finish(text, now, power);
    pending.and(finished)
}

pub(in crate::ui) fn recall(ui: &mut Ui) {
    let path = ui.setup_path();
    if let Some(worker) = ui.host.runtime_mut().storage.as_mut().filter(|worker| worker.busy()) {
        if let Err(error) = worker.recall(path) {
            ui.toast(&format!("Recall failed: {}", error));
        }
        return;
    }
    let text = std::fs::read_to_string(&path);
    apply_recalled_setup(ui, &path, text);
}

fn apply_recalled_setup(ui: &mut Ui, p: &std::path::Path, text: std::io::Result<String>) {
    // what this build cannot take (a decoder it does not have, a key of a newer build) is left out and named
    let text = text.map_err(|e| e.to_string());
    let parsed = text.as_ref().map_err(|e| e.clone()).map(|t| crate::store::parse_setup_lenient(t));
    match parsed {
        Ok((mut m, bad)) => {
            for e in &bad {
                eprintln!("oos-ui: recall {}: left out: {}", p.display(), e);
            }
            // a set-up is a live one, as with the Default key: a reference or SIM session ends first, so a Live
            // afterwards does not bring back the set-up from before it over the recalled one
            if ui.record_mode() {
                ui.leave_session();
            }
            if let Ok(t) = &text {
                ui.apply_ref_looks(&crate::store::parse_ref_looks(t, NREF));
            }
            (m.run, m.status) = (ui.m.run, ui.m.status);
            // the text size is a preference of this screen, not part of a set-up
            m.text_size = ui.m.text_size;
            if m.mask_src != ui.m.mask_src {
                ui.mask = None;
                ui.mask_ref = None;
            }
            ui.m = m;
            ui.reset_sweep_history();
            // what Bus switched on belongs to the set-up before: leaving Bus leaves the recalled one as it is
            ui.bus_auto = None;
            ui.math_state.borrow_mut().forget_fit();
            ui.math_fam = if ui.m.math == MathOp::Off { MathFamily::Arith } else { ui.m.math.family() };
            ui.sync_full();
            if ui.mask.is_none() {
                ui.m.mask_on = false;
            }
            ui.build_mask();
            ui.relayout();
            match bad.first() {
                None => ui.toast(&format!("Recalled {}", crate::store::file_name(p))),
                Some(e) => ui.toast(&format!(
                    "Recalled {}, {} line{} left out ({})",
                    crate::store::file_name(p),
                    bad.len(),
                    if bad.len() == 1 { "" } else { "s" },
                    e
                )),
            }
        }
        Err(e) => ui.toast(&format!("Recall failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::HwShared;
    use crate::model::{RunState, TrigMode};
    use crate::wave::Source;

    #[test]
    fn recalled_single_setup_does_not_restore_the_previous_sessions_sweep() {
        for previous in [TrigMode::Normal, TrigMode::Auto] {
            let mut ui = Ui::for_tests(Source::Hw { shared: HwShared::default(), seen: 0 });
            ui.set_sweep(previous);
            ui.single_sweep();
            apply_recalled_setup(&mut ui, std::path::Path::new("single.setup"), Ok("trig_mode = Single\n".into()));
            assert_eq!(ui.m.trig_mode, TrigMode::Single);
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            remote.exec_line(&mut ui, &crate::gfx::Canvas::new(1, 1), ":RUN");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, TrigMode::Auto));
        }
    }
}
