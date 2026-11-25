// SPDX-License-Identifier: GPL-2.0-or-later
//! Isolated Python tests with the host updater for the release artifact suite.
use super::{host, Outcome};
use crate::run::Context;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) fn run(ctx: &Context, root: &Path) -> Outcome {
    let mut lines = host::build_fpgad_binary(ctx, root, "oos-update")?;
    let updater = host::target(ctx, root)?.join("debug/oos-update");
    lines.extend(host::command_with_environment(
        ctx,
        root,
        "python3",
        &["-I", "tools/python_tests.py"],
        &[("OOS_RELEASE_TEST_UPDATER", updater.as_os_str())],
    )?);
    Ok(lines)
}
