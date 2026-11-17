// SPDX-License-Identifier: GPL-2.0-or-later
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // An empty trust set exercises header/manifest parsing and stops before cryptography or installation.
    let _ = oos_hal::update::pkg::verify_header(data, &[]);
});
