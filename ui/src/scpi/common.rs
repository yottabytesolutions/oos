// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(ui: &mut Ui, nodes: &[&str], query: bool) -> Option<Option<Vec<u8>>> {
    if !query && nodes.len() == 1 {
        if let Some(reply) = mixed_run(ui, nodes) {
            return Some(reply);
        }
    }
    if !query
        && ((nodes.len() == 1 && kw(nodes[0], "TFORce"))
            || (nodes.len() == 2 && kw(nodes[0], "TRIGger") && kw(nodes[1], "FORCe")))
    {
        ui.force_trigger();
        return Some(None);
    }

    // fork debug: raw measure-unit slots (the slot map of the auto set-up's levels)
    if nodes.len() == 2 && kw(nodes[0], "DEBug") && kw(nodes[1], "SLOTs") && query {
        return Some(text_reply(&ui.debug_slots()));
    }
    if nodes.len() == 1 && kw(nodes[0], "AUToscale") && query {
        return Some(text_reply(if ui.auto_running() { "1" } else { "0" }));
    }
    if nodes.len() == 1 && kw(nodes[0], "AUToscale") {
        ui.key(crate::ui::key::AUTO);
        return Some(None);
    }
    if !query && nodes.len() == 1 && kw(nodes[0], "CLEar") {
        ui.key(crate::ui::key::CLEAR);
        return Some(None);
    }
    // Rigol: a forced trigger releases Normal or Single when the level never crosses. On a reference or SIM
    // session that is the replay flag. On the live hardware the front-panel key also asks the acquisition
    // thread to pulse the FPGA force bit.
    if !query && nodes.len() == 1 && kw(nodes[0], "TFORce") {
        return force_key(ui);
    }
    if !query && nodes.len() == 1 && kw(nodes[0], "STOP") && ui.sim_generating() {
        let _ = ui.sim_play_set(false);
        change::edited(ui);
        return Some(None);
    }
    // A reference capture uses the same acquisition state as the screen.
    // RUN and STOP are assignments, so repeating either must not toggle it.
    // Call the shared record handler directly: an open UI dialog must not
    // consume a remote acquisition command as a dialog-dismissal key.
    if !query
        && nodes.len() == 1
        && ui.record_mode()
        && ["RUN", "STOP", "SINGle"].iter().any(|command| kw(nodes[0], command))
    {
        return record_run(ui, nodes);
    }
    if !query && nodes.len() == 1 && kw(nodes[0], "SINGle") {
        ui.scpi_arm_operation();
    }
    if !query && nodes.len() == 1 && actions::run(ui, nodes[0], query) {
        return Some(None);
    }

    if kw(nodes[0], "IDENtify") && query {
        return Some(text_reply(&ui.identify_text()));
    }
    None
}

pub(super) fn ieee(scpi: &mut Scpi, ui: &mut Ui, query: bool, head: &str) -> Option<Option<Vec<u8>>> {
    let up = head.to_ascii_uppercase();
    match up.as_str() {
        "*IDN" => {
            let p = oos_hal::model::platform();
            return Some(text_reply(&format!(
                "OOS,{},{},{}",
                p.caps.name,
                p.serial.as_deref().unwrap_or("0"),
                ui.firmware_version()
            )));
        }
        "*OPC" if !query => {
            scpi.poll_events(ui);
            if scpi.opc_events.len() < 64 {
                scpi.opc_events.push(ui.scpi_operations());
            } else {
                scpi.push_err("-350,\"Too many pending OPC events\"".into());
            }
            scpi.poll_events(ui);
            return Some(None);
        }
        "*ESR" if query => {
            scpi.poll_events(ui);
            let status = std::mem::take(&mut scpi.event_status);
            return Some(text_reply(&status.to_string()));
        }
        "*CLS" if !query => {
            scpi.err.clear();
            scpi.event_status = 0;
            scpi.opc_events.clear();
            return Some(None);
        }
        "*RST" if !query => {
            scpi.event_status = 0;
            scpi.opc_events.clear();
            ui.key(crate::ui::key::DEFAULT);
            return Some(None);
        }
        _ => {}
    }

    None
}

fn record_run(ui: &mut Ui, nodes: &[&str]) -> Option<Option<Vec<u8>>> {
    let key = crate::ui::key::RUNSTOP;
    if kw(nodes[0], "RUN") {
        ui.resume_sweep(TrigMode::Normal);
        if ui.m.run != RunState::Run {
            ui.record_key(key);
        }
    } else if kw(nodes[0], "STOP") {
        if ui.m.run == RunState::Run {
            ui.record_key(key);
        } else {
            let _ = ui.sim_play_set(false);
        }
    } else {
        ui.record_key(crate::ui::key::SINGLE);
    }
    change::edited(ui);
    Some(None)
}

fn mixed_run(ui: &mut Ui, nodes: &[&str]) -> Option<Option<Vec<u8>>> {
    let command = if kw(nodes[0], "STOP") {
        Some(0)
    } else if kw(nodes[0], "RUN") {
        Some(1)
    } else if kw(nodes[0], "SINGle") {
        Some(2)
    } else {
        None
    };
    if command.is_some_and(|command| ui.mixed_bus_command(command)) {
        return Some(None);
    }
    None
}

fn force_key(ui: &mut Ui) -> Option<Option<Vec<u8>>> {
    if ui.record_mode() {
        ui.record_key(crate::ui::key::FORCE);
    } else {
        ui.key(crate::ui::key::FORCE);
    }
    change::edited(ui);
    Some(None)
}

pub(super) fn text_reply(text: &str) -> Option<Vec<u8>> {
    Some(text.as_bytes().to_vec())
}

pub(super) fn undefined(scpi: &mut Scpi, cmd: &str) -> Option<Vec<u8>> {
    scpi.push_err(format!("-113,\"Undefined header\" {}", cmd));
    None
}

pub(super) fn exec_cmd(scpi: &mut Scpi, ui: &mut Ui, cv: &Canvas, cmd: &str) -> Option<Vec<u8>> {
    if cmd.is_empty() {
        return None;
    }
    let (head, arg) = match cmd.find(char::is_whitespace) {
        Some(i) => (&cmd[..i], cmd[i..].trim()),
        None => (cmd, ""),
    };
    let query = head.ends_with('?');
    let head = head.trim_end_matches('?');

    if let Some(reply) = common::ieee(scpi, ui, query, head) {
        return reply;
    }
    let nodes: Vec<&str> = head.trim_start_matches(':').split(':').collect();
    if let Some(reply) = common::exec(ui, &nodes, query) {
        return reply;
    }
    dispatch::exec(scpi, ui, cv, cmd, &nodes, query, arg)
}
