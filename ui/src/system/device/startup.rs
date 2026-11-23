// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;

pub(super) fn prepare(
    ui: &mut Ui,
    disp: &mut drm::Display,
    args: &[String],
    startup: &Option<store::startup::Startup>,
) -> input::Inputs {
    use board::Board;
    ui.persistence_status(startup.as_ref().is_some_and(store::startup::Startup::enabled));
    ui.measure_in_background();
    if let Some(s) = ui::Screen::of_size(disp.width, disp.height) {
        ui.scr = s;
    }
    let mut inp = input::Inputs::open(args);
    inp.send(&board::board().panel_cmd(board::PanelCmd::UiLoaded));
    splash(ui, disp, args);
    if let Some(m) = startup.as_ref().and_then(store::startup::Startup::restored_model) {
        ui.m = m;
        ui.sync_full();
    }
    if args.iter().any(|a| a == "--safe") {
        ui.toast("Safe start: default settings");
    }
    ui.load_system(sysconf::default_path());
    ui.set_ssh_dir(ssh::dir());
    ui.info = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| format!("Linux {}", s.trim()))
        .unwrap_or_default();
    input::install_signal_handlers();
    inp
}
fn splash(ui: &mut Ui, disp: &mut drm::Display, args: &[String]) {
    // start-up screen (ui/startup.rs) while the acquisition hardware starts; the own picture is its background
    let start_up = arg(args, "--start-up");
    if start_up.is_some() {
        let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
        let splash =
            SPLASH.iter().find_map(|p| std::fs::read(p).ok()).and_then(|b| store::ppm_into(&b, &mut cv)).map(|_| cv.px);
        ui.start_up_begin(splash);
    }
    // own picture at power-on: without the start-up screen, the first start of a boot shows a binary
    // PPM for 2 s
    if start_up.is_none() && !std::path::Path::new("/tmp/oos-splash.done").exists() {
        let _ = std::fs::write("/tmp/oos-splash.done", b"");
        let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
        if let Some(p) = SPLASH.iter().find_map(|p| std::fs::read(p).ok()).and_then(|b| store::ppm_into(&b, &mut cv)) {
            disp.present(&cv);
            eprintln!("oos-ui: power-on picture {}x{}", p.0, p.1);
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}
