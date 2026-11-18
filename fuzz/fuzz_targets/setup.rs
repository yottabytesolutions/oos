// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

use oos_ui::store::{parse_setup_lenient, setup_text};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let (model, _) = parse_setup_lenient(text);
        let written = setup_text(&model);
        let (read, errors) = parse_setup_lenient(&written);
        assert!(errors.is_empty(), "the setup writer emits valid setup text");
        assert_eq!(setup_text(&read), written);
    }
});
