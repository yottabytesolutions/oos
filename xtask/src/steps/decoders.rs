// SPDX-License-Identifier: GPL-2.0-or-later
//! oos-ui with core decoders only and each optional decoder alone, including test targets.
use super::Outcome;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) fn matrix(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let list = crate::run::capture(ctx, root, "sh", &["ui/tools/decoder-features.sh", "list"]).map_err(|e| vec![e])?;
    if !list.ok {
        return Err(vec![format!("ui: list decoders: {}", crate::run::tail(&list.stderr, 30))]);
    }
    let mut ok = build(ctx, root, None)?;
    for decoder in list.stdout.split_whitespace() {
        ok.extend(build(ctx, root, Some(&format!("dec-{decoder}")))?);
    }
    Ok(ok)
}

fn build(ctx: &crate::run::Context, root: &Path, feature: Option<&str>) -> Outcome {
    let mut args = vec!["check", "--locked", "--quiet", "--all-targets", "--no-default-features"];
    if let Some(feature) = feature {
        args.extend(["--features", feature]);
    }
    let name = feature.map_or_else(|| "the core decoders only".into(), |f| format!("{f} alone"));
    let out = crate::run::capture(ctx, &root.join("ui"), "cargo", &args).map_err(|e| vec![e])?;
    if out.ok {
        Ok(vec![format!("ui: builds with {name}")])
    } else {
        Err(vec![format!("ui: {name}: {}", crate::run::tail(&out.stderr, 30))])
    }
}
