//! Git-discovered Python and declared-shell syntax.
use super::Outcome;
use std::path::Path;

pub(crate) fn syntax(root: &Path) -> Outcome {
    super::host::command(root, "python3", &["-I", "tools/script_syntax.py"])
}
