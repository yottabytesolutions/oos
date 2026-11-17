// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = oos_ui::store::parse_ag(data);
});
