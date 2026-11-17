// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let head = oos_ui::http::parse_head(&text);
    for peer_ok in [false, true] {
        let _ = oos_ui::http::refuse_early(&head, &text, peer_ok);
    }
});
