//! Explicit image-tier checks, run by the coordinator before unit rounds and releases.
use super::Outcome;
use std::path::Path;

pub(crate) fn required(ctx: &crate::run::Context, root: &Path) -> Outcome {
    super::host::command(ctx, root, "python3", &["-I", "tools/image_required.py"])
}

pub(crate) fn suite(ctx: &crate::run::Context, root: &Path) -> Outcome {
    super::host::command(ctx, root, "python3", &["-I", "tools/image_suite.py"])
}
