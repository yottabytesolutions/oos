// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Pure OOS domain values. Adapters supply capabilities and protocol metadata.

#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods, clippy::disallowed_types, clippy::disallowed_macros)]

pub mod change;
pub mod config;
pub mod decode;
pub mod la;
pub mod math;
pub mod model;
pub mod protocols;
pub mod zone;
