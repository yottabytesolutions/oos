// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::bootcard;
use std::path::Path;

pub(crate) fn run() {
    let status = bootcard::detect();
    if let Err(error) = bootcard::write_state(Path::new(bootcard::STATE_PATH), status) {
        super::super::die(&format!("card-check: write state: {error}"));
    }
}
