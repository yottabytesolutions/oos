// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn accepted_upload_replies_follow_the_signed_packages_install_path() {
    for kexec in [false, true] {
        let extras = if kexec { vec![update::kexec::LINE] } else { vec![] };
        let bytes = pkg::build_with(&[9; 32], "build", "kernel", b"image", &extras).unwrap();
        let h = upload_header(&bytes, &[update::ed25519::public_key(&[9; 32])]).unwrap();
        let (refused, kind, bytes) = upload::reply(Ok(upload::Reply::Accepted(upload::accepted(&h.manifest))));
        assert!(!refused);
        assert_eq!(kind, "text/plain");
        let start = if kexec { update::text::KEXEC_START } else { update::text::DIRECT_START };
        let next = if kexec { "restart" } else { "power cycle" };
        assert_eq!(String::from_utf8(bytes).unwrap(), format!("accepted, build build: header signature ok; checking the image before installation. {} {} System > Update and this page show the state; check again after the {}.", start, update::text::trial(), next));
    }
}

#[test]
fn web_page_explains_both_install_paths_and_restore_power_cycles() {
    let page = page("job: <refused>");
    for text in [
        update::text::KEXEC_START,
        update::text::DIRECT_START,
        &update::text::trial(),
        "A failed trial restores the previous image when available; it does not restart.",
        "job: &lt;refused>",
    ] {
        assert!(page.contains(text), "missing {}", text);
    }
    assert!(!page.contains("new build starts on trial at the next power-on"));
}

#[test]
fn panel_distinguishes_pending_running_and_kexec_trials() {
    let mut st = State::parse("current.build b\ncurrent.sha256 0000000000000000000000000000000000000000000000000000000000000000\ncurrent.len 4096\ntrial 0\n");
    for (starts, running, cmdline, first) in [
        (0, Some("old"), None, "Trial pending; power-cycle to start"),
        (1, Some("b\n"), None, "Trial running; confirms automatically"),
        (0, Some("b"), Some("oos.kexec=install"), "Restart refused or pending; power-cycle"),
        (0, Some("b"), None, "Trial pending; power-cycle to start"),
    ] {
        st.trial = Some(starts);
        let lines = upload::trial_lines(&st, running, cmdline);
        assert_eq!(lines, [first, "After 20 s stable UI and startup done", "Stable UI with startup stuck: 180 s"]);
        assert!(lines.iter().all(|l| l.chars().count() <= 44));
    }
}

#[test]
fn panel_trial_guidance_matches_golden_pictures() {
    let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let mut st = State::parse("current.build b\ncurrent.sha256 0000000000000000000000000000000000000000000000000000000000000000\ncurrent.len 4096\ntrial 0\n");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.parent().unwrap().join("target"))
        .join("update-golden");
    std::fs::create_dir_all(&output).unwrap();
    let mut matched = true;
    for (name, starts, running, cmdline) in
        [("pending", 0, "old", None), ("running", 1, "b", None), ("kexec", 0, "b", Some("oos.kexec=install"))]
    {
        st.trial = Some(starts);
        let mut lines = vec![
            "Update".into(),
            format!("Running {}", running),
            "Recovery b (on trial)".into(),
            "Rollback image old".into(),
        ];
        lines.extend(upload::trial_lines(&st, Some(running), cmdline));
        lines.extend([
            "Boot mode one-shot".into(),
            "".into(),
            "Install: web page :8080/update".into(),
            "Tap to close".into(),
        ]);
        ui.dialog = Some(lines);
        let mut cv = crate::gfx::Canvas::new(ui.scr.w, ui.scr.h);
        ui.draw_overlay(&mut cv);
        let png = crate::store::png_of(&cv);
        std::fs::write(output.join(format!("{}.png", name)), &png).unwrap();
        let expected = std::fs::read(root.join(format!("tests/golden/update/{}.png.ref", name))).unwrap_or_default();
        matched &= png == expected;
    }
    assert!(matched, "update panel pictures differ; inspect the actual images in update-golden");
}
