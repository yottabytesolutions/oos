// SPDX-License-Identifier: GPL-2.0-or-later
//! The same isolated Python runner as CI, without a UI build prerequisite.
use super::{host, Outcome};
use crate::run::Context;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) fn run(ctx: &Context, root: &Path) -> Outcome {
    host::command(ctx, root, "python3", &["-I", "tools/python_tests.py"])
}
