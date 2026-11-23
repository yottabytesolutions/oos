// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "SYSTem") && nodes.len() == 2 {
        if let Some(reply) = settings(scpi, ui, nodes, query, arg) {
            return reply;
        }
    }

    // fork: the TPU UART receiver's words for the current settings (docs/fork/hwuart.md)
    if kw(n0, "SYSTem")
        && nodes.len() == 3
        && kw(nodes[1], "DEBug")
        && (kw(nodes[2], "UART") || kw(nodes[2], "SERial"))
        && query
    {
        return text_reply(&board().uart_debug(&ui.m, ui.switches()));
    }
    // fork: the zone unit's raw status and counter next to the software filter (docs/fork/hwzone.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "ZONE") && query {
        return text_reply(&ui.zone_debug_text());
    }
    // fork: the comparator mapping of the mask test and zone trigger, and the per-column view (docs/fork/hwmask.md)
    if kw(n0, "SYSTem") && nodes.len() == 4 && kw(nodes[1], "DEBug") && kw(nodes[2], "MASK") && kw(nodes[3], "MAP") {
        return mask_map(scpi, ui, query, arg);
    }
    if kw(n0, "SYSTem")
        && nodes.len() == 4
        && kw(nodes[1], "DEBug")
        && kw(nodes[2], "MASK")
        && kw(nodes[3], "COLumns")
        && query
    {
        let mut it = arg.split(',').map(|x| x.trim().parse::<usize>());
        let (a, b) = match (it.next(), it.next()) {
            (Some(Ok(a)), Some(Ok(b))) => (a.min(b), a.max(b)),
            (Some(Ok(a)), None) => (a, a),
            _ => (0, 9),
        };
        return text_reply(&ui.mask_columns_text(a, b));
    }
    // fork: the mask unit's raw counters and slot next to the software test (docs/fork/hwmask.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "MASK") && query {
        return text_reply(&ui.mask_debug_text());
    }
    // fork: the DVM block's raw registers next to the software DVM (docs/fork/dvm.md)
    if kw(n0, "SYSTem") && nodes.len() == 3 && kw(nodes[1], "DEBug") && kw(nodes[2], "DVM") && query {
        return text_reply(&ui.dvm_debug_text());
    }
    // fork, unit debug: force the DVM block's lane with one channel (each lane = one pair of ADC cores)
    if kw(n0, "SYSTem") && nodes.len() == 4 && kw(nodes[1], "DEBug") && kw(nodes[2], "DVM") && kw(nodes[3], "LANE") {
        return dvm_lane(scpi, ui, query, arg);
    }
    if kw(n0, "SYSTem") && nodes.len() == 2 && kw(nodes[1], "ERRor") && query {
        return text_reply(&if scpi.err.is_empty() { "0,\"No error\"".to_string() } else { scpi.err.remove(0) });
    }

    super::undefined(scpi, cmd)
}

fn settings(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if kw(nodes[1], "SET") && !query {
        return set(scpi, ui, arg);
    }
    if kw(nodes[1], "GET") && query {
        let text = crate::store::setup_text(&ui.m);
        let v = text
            .lines()
            .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == arg.trim()).map(|x| x.1.trim().to_string()));
        return Some(text_reply(&v.unwrap_or_default()));
    }
    if kw(nodes[1], "STATe") && query {
        let mut text = crate::cmd::stat_text(ui).trim().to_string();
        if let Some(raw) = &scpi.transport.endpoint.io {
            text.push_str(&raw.ports().enqueue().text());
        }
        use std::fmt::Write;
        let on_off = |on| if on { "on" } else { "off" };
        let switches = ui.switches();
        let _ = write!(
            text,
            " acq.dump={} acq.frame_header={} acq.stop_slot={} gpu.cpu={} gpu.pinned_cpu={}",
            on_off(switches.board.batch_dump.on()),
            on_off(switches.board.header_probe.on()),
            switches.source.stop_slot().as_str(),
            switches.perf.gpu_cpu.as_str(),
            ui.gpu_pinned_cpu().map_or_else(|| "none".into(), |cpu| cpu.to_string()),
        );
        text.push_str(&board().capture_state(&ui.src));
        text.push_str(
            &crate::drawpipe::counters()
                .pipeline_text(ui.switches().source.work_budget(), crate::pacing::step_missed()),
        );
        return Some(text_reply(&text));
    }
    // System > Graphics: CPU or GPU (whether the GPU really draws: the `gpu` part of :SYSTem:STATe?)
    if kw(nodes[1], "GRAPhics") {
        return graphics(scpi, ui, query, arg);
    }
    // rear AUX OUT, stock names TOUT and PFAil; OFF is OOS's (constant low)
    if kw(nodes[1], "AOUTput") {
        return aux_output(scpi, ui, query, arg);
    }
    if kw(nodes[1], "LOG") && !query {
        match parse_bool(arg) {
            Some(on) => ui.log_set(on),
            None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return Some(None);
    }

    None
}

fn mask_map(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    match board().mask_map(if query { None } else { Some(arg.trim()) }) {
        Ok(s) if query => return text_reply(&s),
        Ok(_) => change::edited(ui),
        Err(e) => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", e)),
    }
    None
}

fn set(scpi: &mut Scpi, ui: &mut Ui, arg: &str) -> Option<Option<Vec<u8>>> {
    let Some((key, value)) = arg.split_once('=') else {
        scpi.push_err("-224,\"Illegal parameter value\" no '='".into());
        return Some(None);
    };
    let (key, value) = (key.trim(), value.trim());
    if crate::store::setup::is_retired_setup_key(key) {
        eprintln!("oos-ui: ignoring retired setup key {key}");
        return Some(None);
    }
    let applied = if key == "trig_mode" {
        crate::store::parse_sweep(value).map(|mode| {
            ui.set_sweep(mode);
            true
        })
    } else {
        crate::store::setup::set_known(&mut ui.m, key, value)
    };
    let applied = match applied {
        // Keep the original setup application and edit bookkeeping for optional field suffixes.
        Ok(false) if ["label", "unit", "bias", "skew", "vfine", "tfine"].iter().any(|s| key.ends_with(s)) => {
            crate::store::set(&mut ui.m, key, value).map(|()| true)
        }
        result => result,
    };
    match applied {
        Ok(true) => change::edited(ui),
        Ok(false) => scpi.push_err(format!("-224,\"Illegal parameter value\" unknown key {key}")),
        Err(e) => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", e)),
    }
    Some(None)
}

fn graphics(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(if ui.sys.gpu { "GPU" } else { "CPU" }));
    }
    match arg.trim().to_ascii_uppercase().as_str() {
        "GPU" => ui.graphics_set(true),
        "CPU" => ui.graphics_set(false),
        _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    Some(None)
}

fn aux_output(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Option<Vec<u8>>> {
    if query {
        return Some(text_reply(match ui.m.aux_out {
            AuxOut::TrigOut => "TOUT",
            AuxOut::Off => "OFF",
            AuxOut::PassFail => "PFA",
        }));
    }
    let want = match arg.trim().to_ascii_uppercase().as_str() {
        "TOUT" => Some(AuxOut::TrigOut),
        "OFF" => Some(AuxOut::Off),
        "PFA" | "PFAIL" => Some(AuxOut::PassFail),
        _ => None,
    };
    match want.filter(|a| a.offered(board().mask_hw(ui.switches()) || board().zone_hw(ui.switches()))) {
        Some(a) => {
            ui.m.aux_out = a;
            change::edited(ui);
        }
        None => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    Some(None)
}

fn dvm_lane(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&ui.dvm_lane_text());
    }
    match arg.trim().to_ascii_uppercase().as_str() {
        "AUTO" => ui.dvm_lane_set(None),
        a => match a.parse::<u32>() {
            Ok(l) if l < 4 => ui.dvm_lane_set(Some(l)),
            _ => scpi.err.push(format!("-224,\"Illegal parameter value\" {}", arg)),
        },
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_setup_key_is_ignored_and_queries_empty() {
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        let before = crate::store::setup_text(&ui.m);
        assert!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:SET draw_path = batch").is_empty());
        assert_eq!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"), b"0,\"No error\"");
        assert!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:GET? draw_path").is_empty());
        assert_eq!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"), b"0,\"No error\"");
        assert_eq!(crate::store::setup_text(&ui.m), before);
    }

    #[test]
    fn unknown_setup_keys_are_rejected() {
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        for key in ["draw_paths", "ch1.unknown", "d2.unknown"] {
            scpi.exec_line(&mut ui, &canvas, &format!(":SYSTem:SET {key} = batch"));
            assert_eq!(
                scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"),
                format!("-224,\"Illegal parameter value\" unknown key {key}").as_bytes()
            );
        }
        assert!(ui.m.decs[1].is_none());
    }

    #[test]
    fn optional_setup_suffixes_keep_legacy_scpi_behavior() {
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        assert!(ui.m.decs[1].is_none());
        for prefix in ["", "ch1.", "d2."] {
            for suffix in ["label", "unit", "bias", "skew", "vfine", "tfine"] {
                scpi.exec_line(&mut ui, &canvas, &format!(":SYSTem:SET {prefix}unknown_{suffix} = ignored"));
                assert_eq!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"), b"0,\"No error\"");
            }
        }
        assert!(ui.m.decs[1].is_some());
        scpi.exec_line(&mut ui, &canvas, ":SYSTem:SET ch0.unknown_label = ignored");
        assert_eq!(
            scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"),
            b"-224,\"Illegal parameter value\" unknown channel"
        );
    }

    #[test]
    fn known_setup_key_applies_even_if_omitted_at_default() {
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        scpi.exec_line(&mut ui, &canvas, ":SYSTem:SET ch1.vfine = 1.5");
        assert_eq!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"), b"0,\"No error\"");
        assert_eq!(scpi.exec_line(&mut ui, &canvas, ":SYSTem:GET? ch1.vfine"), b"1.5");
    }

    #[test]
    fn setup_file_aliases_are_rejected_without_applying_over_scpi() {
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
        let before = crate::store::setup_text(&ui.m);
        for (key, value) in [
            ("tscale", "0"),
            ("zscale", "0"),
            ("cur_b_ch", "2"),
            ("hdmi_wave", "true"),
            ("ch1.vscale", "0"),
            ("ch1.bw20", "true"),
        ] {
            scpi.exec_line(&mut ui, &canvas, &format!(":SYSTem:SET {key} = {value}"));
            assert_eq!(
                scpi.exec_line(&mut ui, &canvas, ":SYSTem:ERRor?"),
                format!("-224,\"Illegal parameter value\" unknown key {key}").as_bytes()
            );
            assert_eq!(crate::store::setup_text(&ui.m), before);
        }
    }

    #[test]
    fn state_reports_startup_diagnostics_switches() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let mut scpi = Scpi::open(0u16, false);
        let canvas = Canvas::new(1, 1);
        for (value, slot, reported) in [("off", "", "zero"), ("on", "acq.stop_slot = alternate", "alternate")] {
            let conf = format!("acq.dump = {value}\nacq.frame_header = {value}\n{slot}");
            let switches = crate::sysconf::Switches::parse(&|_| None, &conf, "", Default::default());
            let host = crate::ui::host::Host::headless().with_switches(switches);
            let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, host);
            let reply = scpi.exec_line(&mut ui, &canvas, ":SYSTem:STATe?");
            let state = String::from_utf8(reply).unwrap();
            assert!(state.contains(&format!("acq.dump={value} acq.frame_header={value}")), "{state}");
            assert!(state.split_whitespace().any(|word| word == format!("acq.stop_slot={reported}")), "{state}");
        }
    }

    #[test]
    fn state_reports_capture_only_after_the_current_hardware_frame() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let shared = crate::board::HwShared::default();
        let mut scpi = Scpi::disconnected();
        let canvas = Canvas::new(1, 1);
        let switches = crate::sysconf::Switches::parse(
            &|_| None,
            "acq.capture = peak:4\nacq.capture.inferred = on",
            "",
            Default::default(),
        );
        let host = crate::ui::host::Host::headless().with_switches(switches);
        let mut ui = Ui::new(crate::wave::Source::Hw { shared: shared.clone(), seen: 0 }, host);
        let state =
            |scpi: &mut Scpi, ui: &mut Ui| String::from_utf8(scpi.exec_line(ui, &canvas, ":SYSTem:STATe?")).unwrap();
        assert!(state(&mut scpi, &mut ui).contains("acq.capture=pending acq.capture.stored_points=0"));
        {
            let mut g = crate::perfguard::deep::acq_lock(&shared);
            let mut want = crate::board::Want { tdiv: 1e-6, depth: 14_064, window: None, ..Default::default() };
            let p = want.reduce_capture(true, 4, true).unwrap();
            let id = g.acquisition.identity();
            g.want = Some(want);
            g.frame = std::sync::Arc::new(crate::board::Frame {
                seq: 1,
                cfg: id.gen(),
                stamp: id.epoch(),
                acq: (p.rate as f64, p.memsel as usize),
                ..Default::default()
            });
        }
        let active = state(&mut scpi, &mut ui);
        assert!(active.contains("acq.capture.requested=peak:4 acq.capture.inferred=on"));
        assert!(active.contains("acq.capture=peak:4 acq.capture.stored_points=3512"));
        assert!(active.contains(" bytes=0/0 "));
    }
}
