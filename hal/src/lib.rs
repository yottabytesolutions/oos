// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! OOS hardware layer.
//!
//! The crate root holds what does not depend on one board: the model and its limits (`model`), the vendor file keys
//! (`vendorkeys`), signed updates (`update`), WLAN (`wlan`), OS calls (`system`) and the start-up status file (`bootstatus`). Everything of
//! the DHO800/DHO900 board (RK3399 + Zynq FPGA over PCIe/XDMA) is in `dho`; a port to another board family adds a
//! module next to it (docs/porting.md).
//!
//! The register sequences are described in `docs/protocols/`. No code of the vendor libraries is used. Functions that
//! build register sequences are pure (they return `(offset, value)` lists and update software shadows of the
//! write-only registers), so they are unit-tested on the host; `dho::bus` applies them to the hardware.

pub mod bootstatus;
pub mod model;
pub mod sync;
pub mod system;
pub mod target;
pub mod update;
pub mod vendorkeys;
pub mod wlan;

pub mod dho;

mod crc;
pub use crc::crc32;
