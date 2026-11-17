// SPDX-License-Identifier: GPL-2.0-or-later
//! oos-ui with core decoders only and each optional decoder alone, including test targets.
use super::Outcome;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) fn matrix(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let shard = crate::run::partition("OOS_DECODER_PARTITION", std::env::var("OOS_DECODER_PARTITION").ok().as_deref())
        .map_err(|e| vec![e])?;
    partitioned(ctx, root, shard)
}

fn partitioned(ctx: &crate::run::Context, root: &Path, shard: Option<(u32, u32)>) -> Outcome {
    let list = crate::run::capture(ctx, root, "sh", &["ui/tools/decoder-features.sh", "list"]).map_err(|e| vec![e])?;
    if !list.ok {
        return Err(vec![format!("ui: list decoders: {}", crate::run::tail(&list.stderr, 30))]);
    }
    let features = std::iter::once(None).chain(list.stdout.split_whitespace().map(|d| Some(format!("dec-{d}"))));
    let mut ok = Vec::new();
    for (i, feature) in features.enumerate() {
        if shard.is_none_or(|(k, n)| i % n as usize == (k - 1) as usize) {
            ok.extend(build(ctx, root, feature.as_deref())?);
        }
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
