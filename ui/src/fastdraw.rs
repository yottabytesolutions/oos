// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Trigger refinement, sample access and raster helpers shared by the batch and decoded draw paths.

pub(crate) mod code_rows;
mod corrected;
pub mod difference;
mod mapping;
mod parallel;
mod samples;
mod trigger;
mod whole;

pub(crate) use corrected::CorrectedCodes;
#[cfg(test)]
pub use difference::add_spans;
pub use difference::fold_t;
pub(crate) use difference::{add_spans_t, deposit_dots_t, diff_span, zoom};
pub(crate) use mapping::{clip_rows, code_spans_into, columns, mapped_rows, spans_into};
#[cfg(test)]
pub(crate) use mapping::{first_sample, spans_mapped};
pub use mapping::{span_rows, Map, Spans};
pub use parallel::{helpers, par_each, par_map};
pub use samples::{min_max, Codes, Fold, Samples};
pub use trigger::edge_near;
pub(crate) use trigger::{edge_smooth, EdgeSearch};
#[cfg(test)]
pub use whole::deposit_dots_band;
pub(crate) use whole::with_scratch;
pub use whole::{deposit, deposit_codes, deposit_dots};

#[cfg(test)]
mod validation;
