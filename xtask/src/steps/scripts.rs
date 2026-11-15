//! Git-discovered Python and declared-shell syntax.
use super::Outcome;
use std::path::Path;

pub(crate) fn syntax(ctx: &crate::run::Context, root: &Path) -> Outcome {
    super::host::command(ctx, root, "python3", &["-I", "tools/script_syntax.py"])
}
