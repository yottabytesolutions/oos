// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::board::{fpga_xadc, line};

pub(super) fn run() {
    println!("{}", line(&oos_hal::dho::board::thermal_zones(std::path::Path::new("/sys/class/thermal")), fpga_xadc()));
}
