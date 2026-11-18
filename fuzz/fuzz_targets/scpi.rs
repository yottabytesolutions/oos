// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

use oos_ui::{
    scpi::{Program, Scpi},
    Canvas, Host, Source, Ui,
};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = Program::new(&text);
    // Keep file access, external generators and background sample generation out of this parser target.
    let external = text.split([';', '\r', '\n']).any(|command| {
        command.split_whitespace().next().unwrap_or("").split(':').any(|node| {
            matches!(
                node.trim_end_matches('?').to_ascii_uppercase().as_str(),
                "LOAD" | "SAVE" | "IMP" | "IMPORT" | "ARB" | "LOG" | "BODE" | "SIM"
            )
        })
    });
    if !external {
        let mut scpi = Scpi::disconnected();
        let mut ui = Ui::new(Source::Demo { t: 0.0 }, Host::headless());
        let canvas = Canvas::new(1, 1);
        let _ = scpi.exec_message_checked(&mut ui, &canvas, &text);
    }
});
