// SPDX-License-Identifier: GPL-2.0-or-later

use super::small_tests::{demo, read_rows, strings};
use super::*;

fn check_rows(u: &Ui, menu: MenuId, title: &str, rows: &[(Item, Kind, &str, String, Vec<String>)]) {
    let open = Item::Open(menu);
    assert_eq!(Ui::kind(open), Kind::Action);
    assert_eq!(Ui::label(open), title);
    assert_eq!(u.value(open), "");
    assert!(u.options(open).is_empty());
    read_rows(u, rows);
}

#[test]
fn spectrum_reads_cover_every_row() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.view = AView::Spectrum;
    u.m.spec_src = WaveId::Ch(1);
    u.m.spec_center = 1250.0;
    u.m.spec_span = 500.0;
    u.m.spec_rbw = 20.0;
    u.m.spec_ref = -12.0;
    u.m.spec_acpr_w = 250.0;
    u.m.spec_acpr_s = 750.0;
    u.m.spec_margin = 6.0;
    let rows = [
        (SpecOn, Choice, "View", "On".into(), strings(&["Off", "On"])),
        (SpecSrc, Choice, "Source", "CH2".into(), u.wave_names()),
        (SpecCenter, Value, "Centre", si_short(1250.0, "Hz"), vec![]),
        (SpecSpan, Value, "Span", si_short(500.0, "Hz"), vec![]),
        (SpecHarm, Choice, "Harmonics", panes::harm_names()[u.m.spec_harm].clone(), panes::harm_names()),
        (SpecRbw, Value, "RBW", si_short(20.0, "Hz"), vec![]),
        (
            SpecWin,
            Choice,
            "Window",
            u.m.spec_win.name().into(),
            crate::math::WINDOWS.iter().map(|w| w.name().into()).collect(),
        ),
        (SpecTrace, Choice, "Trace", "Normal".into(), strings(&["Normal", "Average", "Max hold", "Min hold"])),
        (SpecRef, Value, "Ref level", "-12 dBV".into(), vec![]),
        (SpecWf, Choice, "Waterfall", "Off".into(), strings(&["Off", "On"])),
        (SpecDist, Choice, "THD / SNR", "Off".into(), strings(&["Off", "On"])),
        (SpecAcprW, Value, "Ch width", si_short(250.0, "Hz"), vec![]),
        (SpecAcprS, Value, "Ch space", si_short(750.0, "Hz"), vec![]),
        (SpecPeak, Action, "Centre on peak", "".into(), vec![]),
        (SpecMargin, Value, "Limit margin", "6 dB".into(), vec![]),
        (SpecLimitSet, Action, "Limit from trace", "".into(), vec![]),
        (SpecLimitClear, Action, "Clear limit", "".into(), vec![]),
    ];
    check_rows(&u, MenuId::Spectrum, "Spectrum", &rows);
    assert_eq!(u.menu_items(MenuId::Spectrum), rows.iter().map(|r| r.0).collect::<Vec<_>>());
}

#[test]
fn eye_reads_cover_every_row() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.view = AView::Eye;
    u.m.eye_place = crate::model::EyePlace::Under;
    u.m.eye_src = WaveId::Ch(1);
    u.m.eye_rate = 2500.0;
    u.m.eye_from_record = false;
    u.m.eye_ber = 1e-9;
    u.m.eye_b = Some(WaveId::Ch(0));
    u.m.eye_hp = true;
    u.m.eye_pn_lo = 30.0;
    u.m.eye_pn_hi = 1500.0;
    u.m.eye_npj = true;
    u.m.eye_rail = Some(WaveId::Ch(1));
    u.m.eye_notch = 60.0;
    let rows = [
        (EyeAct, Action, "Auto Eye/Jitter", "".into(), vec![]),
        (EyeOn, Choice, "View", "On".into(), strings(&["Off", "On"])),
        (EyePlace, Choice, "Place", "Under".into(), strings(&["Full", "Side", "Under"])),
        (EyeSrc, Choice, "Source", "CH2".into(), u.wave_names()),
        (EyeRate, Value, "Bit rate", si_short(2500.0, "b/s"), vec![]),
        (EyeBer, Value, "BER", "1e-9".into(), vec![]),
        (EyeB, Choice, "Clock", "CH1".into(), u.wave_names_opt()),
        (EyeHp, Choice, "Highpass", "On".into(), strings(&["Off", "On"])),
        (EyePnLo, Value, "Pn low", si_short(30.0, "Hz"), vec![]),
        (EyePnHi, Value, "Pn high", si_short(1500.0, "Hz"), vec![]),
        (EyeNpj, Choice, "NPJ", "On".into(), strings(&["Off", "On"])),
        (EyeRail, Choice, "Rail", "CH2".into(), u.wave_names_opt()),
        (EyeNotch, Value, "Notch", si_short(60.0, "Hz"), vec![]),
        (EyeClear, Action, "Clear eye", "".into(), vec![]),
    ];
    check_rows(&u, MenuId::Eye, "Eye / jitter", &rows);
    assert_eq!(u.menu_items(MenuId::Eye), rows.iter().map(|r| r.0).collect::<Vec<_>>());
}

#[test]
fn power_reads_cover_every_row() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.view = AView::Power;
    u.m.pw_v = WaveId::Ch(0);
    u.m.pw_i = WaveId::Ch(1);
    u.m.pw_mode = PowerMode::Rail;
    u.m.pw_fline = 50.0;
    u.m.pw_fsw = 1000.0;
    u.m.pw_vin = Some(WaveId::Ch(1));
    u.m.pw_ref = Some(3.3);
    u.m.pw_clk = Some(WaveId::Ch(0));
    check_rows(
        &u,
        MenuId::Power,
        "Power analysis",
        &[
            (PowOn, Choice, "View", "On".into(), strings(&["Off", "On"])),
            (PowV, Choice, "Voltage", "CH1".into(), u.wave_names()),
            (PowI, Choice, "Current", "CH2".into(), u.wave_names()),
            (
                PowMode,
                Choice,
                "Page",
                "Rail".into(),
                strings(&["Quality", "Harmonics", "Switching", "Ripple", "Cin", "Rail"]),
            ),
            (PowFline, Value, "Line f", si_short(50.0, "Hz"), vec![]),
            (PowFsw, Value, "Switch f", si_short(1000.0, "Hz"), vec![]),
            (PowVin, Choice, "Input", "CH2".into(), u.wave_names_opt()),
            (PowRef, Value, "Reference", si_short(3.3, "V"), vec![]),
            (PowClk, Choice, "Clock", "CH1".into(), u.wave_names_opt()),
        ],
    );
}

#[test]
fn generator_reads_cover_every_row() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.gen_sel = 0;
    u.m.afg_on = true;
    u.m.afg_wave = 3;
    u.m.afg_freq = 1250.0;
    u.m.afg_ampl = 2.5;
    u.m.afg_offset = -0.25;
    u.m.afg_duty = 37.0;
    u.m.afg_symmetry = 62.0;
    u.m.afg_mod = 1;
    u.m.afg_mod_rate = 250.0;
    u.m.afg_mod_depth = 45.0;
    u.m.afg_mod_shape = 1;
    u.m.afg_arb_src = 1;
    check_rows(
        &u,
        MenuId::Afg,
        "Generator",
        &[
            (AfgSel, Choice, "Generator", "Gen 1".into(), (1..=gen_count()).map(|n| format!("Gen {n}")).collect()),
            (AfgOn, Choice, "Output", "On".into(), strings(&["Off", "On"])),
            (AfgWave, Choice, "Wave", "Pulse".into(), AFG_WAVES.iter().map(|s| s.to_string()).collect()),
            (AfgFreq, Value, "Frequency", si_short(1250.0, "Hz"), vec![]),
            (
                AfgFreqEntry,
                Choice,
                "Enter frequency",
                "Keypad".into(),
                FREQ_KEYS.iter().map(|s| s.to_string()).collect(),
            ),
            (AfgAmpl, Value, "Amplitude", si_short(2.5, "Vpp"), vec![]),
            (AfgOffset, Value, "Offset", si(-0.25, "V"), vec![]),
            (AfgArbSrc, Choice, "Capture from", "CH2".into(), ch_names()),
            (AfgArbGrab, Action, "Capture the screen", "".into(), vec![]),
            (AfgArbFile, Action, "Load arb.csv", "".into(), vec![]),
            (AfgDuty, Value, "Duty", "37 %".into(), vec![]),
            (AfgSym, Value, "Symmetry", "62 %".into(), vec![]),
            (AfgMod, Choice, "Modulation", "AM".into(), AFG_MODS.iter().map(|s| s.to_string()).collect()),
            (AfgModRate, Value, "Mod. rate", si_short(250.0, "Hz"), vec![]),
            (AfgModDepth, Value, "Depth / dev.", "45 %".into(), vec![]),
            (
                AfgModShape,
                Choice,
                "Mod. shape",
                AFG_MOD_SHAPES[1].into(),
                AFG_MOD_SHAPES.iter().map(|s| s.to_string()).collect(),
            ),
        ],
    );
}

#[test]
fn bode_reads_cover_every_row() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.view = AView::Bode;
    u.m.bode_mode = oos_dsp::bode::Mode::Psrr;
    u.m.bode_gen = 1;
    u.m.bode_start = 25.0;
    u.m.bode_stop = 2500.0;
    u.m.bode_points = 32;
    u.m.bode_amp = 0.25;
    u.m.bode_in = WaveId::Ch(0);
    u.m.bode_out = WaveId::Ch(1);
    u.m.bode_addr = "example.invalid".into();
    u.m.bode_usb = true;
    u.m.bode_sw_time = 0.5;
    u.m.bode_sw_log = true;
    u.m.bode_sync = 1;
    u.m.bode_sync_slope = Slope::Fall;
    u.m.bode_sync_level = 0.5;
    u.m.bode_ref = -3.0;
    check_rows(
        &u,
        MenuId::Bode,
        "Bode",
        &[
            (BodeOn, Choice, "View", "On".into(), strings(&["Off", "On"])),
            (BodeMode, Choice, "Mode", "PSRR".into(), strings(&["Loop", "PSRR", "Z"])),
            (BodeRun, Action, "Start / stop sweep", "".into(), vec![]),
            (BodeStart, Value, "Start freq", si_short(25.0, "Hz"), vec![]),
            (BodeStop, Value, "Stop freq", si_short(2500.0, "Hz"), vec![]),
            (BodePoints, Value, "Points", "32".into(), vec![]),
            (BodeAmp, Value, "Amplitude", si_short(0.25, "Vpp"), vec![]),
            (BodeIn, Choice, "Input (ref)", "CH1".into(), u.wave_names()),
            (BodeRef, Value, "Ref level", "-3.0 dBV".into(), vec![]),
            (BodeOut, Choice, "Output", "CH2".into(), u.wave_names()),
            (
                BodeGen,
                Choice,
                "Generator",
                "APPL".into(),
                super::super::bode::GENERATORS.iter().map(|g| g.name.into()).collect(),
            ),
            (BodeSync, Choice, "Sync", "CH2".into(), ch_names()),
            (BodeSyncEdge, Choice, "Sync edge", "Falling".into(), strings(&["Rising", "Falling"])),
            (BodeSyncLvl, Value, "Sync level", si_short(0.5, "V"), vec![]),
            (BodeSwTime, Value, "Sweep time", si_short(0.5, "s"), vec![]),
            (BodeSwLog, Choice, "Sweep", "Log".into(), strings(&["Lin", "Log"])),
            (BodeCur, Value, "Cursor", "-".into(), vec![]),
            (BodeLink, Choice, "Connection", "USB".into(), strings(&["LAN", "USB"])),
            (BodeAddr, Value, "Gen address", "example.invalid".into(), vec![]),
        ],
    );
}

#[test]
fn power_visibility_covers_every_page_and_source_injection() {
    use Item::*;
    let mut u = demo();
    u.m.pw_v = WaveId::Ch(0);
    u.m.pw_i = WaveId::Ch(1);
    u.m.ch[0].on = false;
    u.m.ch[1].on = false;
    for (mode, title) in [
        (PowerMode::Quality, "Quality"),
        (PowerMode::Harmonics, "Harmonics"),
        (PowerMode::Switching, "Switching"),
        (PowerMode::Ripple, "Ripple"),
        (PowerMode::Capacitance, "Cin"),
        (PowerMode::Rail, "Rail"),
    ] {
        u.m.pw_mode = mode;
        let mut expected = vec![PowOn, PowV, PowI, PowMode];
        match mode {
            PowerMode::Ripple => expected.extend([PowFline, PowFsw, PowVin]),
            PowerMode::Rail => expected.extend([PowRef, PowVin, PowClk]),
            _ => {}
        }
        assert_eq!(u.value(PowMode), title);
        assert_eq!(u.menu_items(MenuId::Power), expected);
        assert_eq!(
            u.off_sources(MenuId::Power),
            if matches!(mode, PowerMode::Ripple | PowerMode::Rail) { vec![0] } else { vec![0, 1] }
        );
        expected.push(SrcOn);
        assert_eq!(u.items(MenuId::Power), expected);
    }
}

#[test]
fn generator_visibility_covers_every_wave_and_modulation() {
    use Item::*;
    let mut u = demo();
    for (wave, wave_name) in AFG_WAVES.iter().enumerate() {
        u.m.afg_wave = wave;
        for (modulation, modulation_name) in AFG_MODS.iter().enumerate() {
            u.m.afg_mod = modulation;
            let mut expected = Vec::new();
            if gen_count() > 1 {
                expected.push(AfgSel);
            }
            expected.extend([AfgOn, AfgWave]);
            match wave {
                5 => expected.push(AfgOffset),
                6 => expected.extend([AfgFreq, AfgAmpl, AfgOffset, AfgArbSrc, AfgArbGrab, AfgArbFile]),
                4 => expected.extend([AfgAmpl, AfgOffset]),
                _ => expected.extend([AfgFreq, AfgAmpl, AfgOffset]),
            }
            match wave {
                1 | 3 => expected.push(AfgDuty),
                2 => expected.push(AfgSym),
                _ => {}
            }
            if wave <= 2 {
                expected.push(AfgMod);
                if modulation > 0 {
                    expected.extend([AfgModRate, AfgModDepth, AfgModShape]);
                }
            }
            assert_eq!(u.menu_items(MenuId::Afg), expected, "wave {wave}, modulation {modulation}");
            assert_eq!(u.items(MenuId::Afg), expected);
            assert_eq!(u.value(AfgWave), *wave_name);
            assert_eq!(u.value(AfgMod), *modulation_name);
        }
    }
}

#[test]
fn bode_visibility_covers_every_generator_connection_input_and_cursor() {
    use Item::*;
    let mut u = demo();
    for generator in 0..=super::super::bode::GENERATORS.len() {
        u.m.bode_gen = generator;
        for usb in [false, true] {
            u.m.bode_usb = usb;
            for input in 0..=n_ch() {
                u.m.bode_sw_in = input;
                for points in [false, true] {
                    u.bode.points = if points { vec![(100.0, -3.0, 45.0)] } else { vec![] };
                    let mut expected = if generator == super::super::bode::FREE {
                        let mut rows =
                            vec![BodeOn, BodeRun, BodeStart, BodeStop, BodeSwTime, BodeSwLog, BodePoints, BodeIn];
                        if input == 0 {
                            rows.push(BodeRef);
                        }
                        rows.extend([BodeOut, BodeGen, BodeSync, BodeSyncEdge, BodeSyncLvl]);
                        assert_eq!(u.value(BodeIn), if input == 0 { "Off".into() } else { format!("CH{input}") });
                        assert_eq!(u.options(BodeIn), opt_ch_names("Off"));
                        rows
                    } else {
                        assert_eq!(u.value(BodeIn), u.wave_name(u.m.bode_in));
                        assert_eq!(u.options(BodeIn), u.wave_names());
                        vec![
                            BodeOn, BodeMode, BodeRun, BodeStart, BodeStop, BodePoints, BodeAmp, BodeIn, BodeOut,
                            BodeGen,
                        ]
                    };
                    if points {
                        expected.push(BodeCur);
                    }
                    if generator != super::super::bode::FREE && generator != super::super::bode::BUILTIN {
                        expected.push(BodeLink);
                        if !usb {
                            expected.push(BodeAddr);
                        }
                    }
                    assert_eq!(
                        u.menu_items(MenuId::Bode),
                        expected,
                        "generator {generator}, USB {usb}, input {input}, points {points}"
                    );
                }
            }
        }
    }
}

#[test]
fn automatic_optional_and_clamped_values_keep_their_spelling() {
    let mut u = demo();
    u.m.spec_span = 0.0;
    u.m.spec_rbw = 0.0;
    u.m.spec_trace = usize::MAX;
    assert_eq!(u.value(Item::SpecSpan), "Full");
    assert_eq!(u.value(Item::SpecRbw), "Auto");
    assert_eq!(u.value(Item::SpecTrace), "Min hold");
    u.m.eye_rate = 0.0;
    u.m.eye_notch = 0.0;
    u.m.eye_b = None;
    u.m.eye_rail = None;
    assert_eq!(u.value(Item::EyeRate), "Auto");
    assert_eq!(u.value(Item::EyeNotch), "off");
    assert_eq!(u.value(Item::EyeB), "Off");
    assert_eq!(u.value(Item::EyeRail), "Off");
    u.m.eye_rate = 1000.0;
    u.m.eye_from_record = true;
    assert_eq!(u.value(Item::EyeRate), "1 kb/s from record");
    u.m.pw_fline = 0.0;
    u.m.pw_fsw = 0.0;
    u.m.pw_ref = None;
    u.m.pw_vin = None;
    u.m.pw_clk = None;
    assert_eq!(u.value(Item::PowFline), "auto");
    assert_eq!(u.value(Item::PowFsw), "auto");
    assert_eq!(u.value(Item::PowRef), "Top");
    assert_eq!(u.value(Item::PowVin), "Off");
    assert_eq!(u.value(Item::PowClk), "Off");
    u.m.afg_mod = 2;
    u.m.afg_fm_dev = 500.0;
    assert_eq!(u.value(Item::AfgModDepth), "dev 500 Hz");
    u.m.afg_mod = 3;
    u.m.afg_pm_dev = 37.0;
    assert_eq!(u.value(Item::AfgModDepth), "dev 37°");
}

#[test]
fn shared_rendering_picker_back_and_close_preserve_committed_identity() {
    for size in TEXT_SIZES {
        for (menu, item) in [
            (MenuId::Spectrum, Item::SpecOn),
            (MenuId::Eye, Item::EyeOn),
            (MenuId::Power, Item::PowOn),
            (MenuId::Bode, Item::BodeOn),
            (MenuId::Afg, Item::AfgOn),
        ] {
            let mut u = demo();
            u.m.text_size = size;
            u.m.view = AView::Scope;
            u.m.afg_on = false;
            u.open_menu(MenuId::Main);
            u.focus = 3;
            u.open_submenu(menu);
            u.focus = 1;
            let rows = u.dlg_layout().unwrap().rows;
            assert_eq!(rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            assert!(matches!(u.ctl(item, 1), Ctl::Pick));
            assert!(matches!(u.ctl(item, 1000), Ctl::Seg(_, Some(0))));
            u.open_picker(item);
            assert_eq!(u.focus, 0);
            u.focus = 1;
            assert_eq!(active_choice::index(&u, item), Some(0));
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (menu, 1));
            let back = u.dlg_layout().unwrap().back.unwrap();
            assert!(u.tap_menu(back.x + 1, back.y + 1));
            assert_eq!((u.menu, u.focus), (MenuId::Main, 3));
            u.open_submenu(menu);
            let close = u.dlg_layout().unwrap().close;
            assert!(u.tap_menu(close.x + 1, close.y + 1));
            assert!(!u.menu_open);
        }
    }
}

#[test]
fn capabilities_in_isolated_processes() {
    for model in ["DHO802", "DHO804", "DHO924S"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ui::menu::analysis_read::capability_fixture", "--nocapture"])
            .env("OOS_ANALYSIS_READ_MODEL", model)
            .output()
            .unwrap();
        assert!(output.status.success(), "{model}: {}", String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn capability_fixture() {
    let Ok(model) = std::env::var("OOS_ANALYSIS_READ_MODEL") else { return };
    assert!(oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some(&model))));
    generator_visibility_covers_every_wave_and_modulation();
    bode_visibility_covers_every_generator_connection_input_and_cursor();
    for size in TEXT_SIZES {
        let mut u = demo();
        u.m.text_size = size;
        for offline in [false, true] {
            u.offline = offline;
            for menu in [MenuId::Spectrum, MenuId::Eye, MenuId::Power, MenuId::Bode, MenuId::Afg] {
                u.open_menu(menu);
                assert_eq!(u.dlg_layout().unwrap().rows.iter().map(|r| r.it).collect::<Vec<_>>(), u.items(menu));
            }
            for (item, optional) in [
                (Item::SpecSrc, false),
                (Item::EyeSrc, false),
                (Item::PowV, false),
                (Item::PowI, false),
                (Item::BodeIn, false),
                (Item::BodeOut, false),
                (Item::EyeB, true),
                (Item::EyeRail, true),
                (Item::PowVin, true),
                (Item::PowClk, true),
            ] {
                assert_eq!(u.options(item), if optional { u.wave_names_opt() } else { u.wave_names() });
            }
            for item in [Item::AfgArbSrc, Item::BodeSync] {
                assert_eq!(u.options(item), ch_names());
            }
        }
    }
}
