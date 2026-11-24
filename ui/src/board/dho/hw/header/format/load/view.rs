// SPDX-License-Identifier: GPL-2.0-or-later

use super::{cast, field, finite_str, words, Fields};
use crate::drawpipe::route::ZoomView;
use crate::model::{Model, NCH};
use crate::sparse::Interp;

pub(crate) struct PictureView {
    model: Model,
    size: (usize, usize),
    interp: Interp,
    zoom: Option<ZoomView>,
    workers: usize,
    prior_hits: u64,
}

impl PictureView {
    pub(crate) fn inputs(&self) -> (&Model, (usize, usize), Interp, Option<ZoomView>) {
        (&self.model, self.size, self.interp, self.zoom)
    }

    pub(crate) fn scheduling(&self) -> (usize, u64) {
        (self.workers, self.prior_hits)
    }
}

pub(super) fn view(fields: &Fields<'_>) -> Result<Option<PictureView>, String> {
    if !fields.contains_key("view") {
        return Ok(None);
    }
    let [w, h, interp, workers, prior_hits] = words(fields, "view")?;
    let workers = cast("workers", workers)?;
    if !(1..=16).contains(&workers) {
        return Err("invalid worker count".into());
    }
    let mut model = Model::default();
    for (&key, &val) in fields {
        if let Some(key) = key.strip_prefix("view.") {
            if !crate::drawpipe::route::fixture_view_key(key) {
                return Err(format!("unexpected view key {key}"));
            }
            crate::store::set(&mut model, key, val)?;
        }
    }
    for k in [
        "tdiv",
        "delay",
        "dots",
        "acq",
        "trig_src",
        "trig_type",
        "trig_level",
        "slope",
        "trig_coup",
        "zoom_tdiv",
        "zdelay",
    ] {
        field(fields, &format!("view.{k}"))?;
    }
    for c in 1..=NCH {
        for k in ["on", "vdiv", "offset", "probe", "invert", "coupling", "bw"] {
            field(fields, &format!("view.ch{c}.{k}"))?;
        }
    }
    let zoom = fields.contains_key("zoom_view").then(|| zoom_view(fields)).transpose()?;
    let interp = match interp {
        "Exact" => Interp::Exact,
        "Fast" => Interp::Fast,
        _ => return Err("invalid interpolation".into()),
    };
    Ok(Some(PictureView {
        model,
        size: size(w, h)?,
        interp,
        zoom,
        workers,
        prior_hits: cast("prior hits", prior_hits)?,
    }))
}

fn zoom_view(fields: &Fields<'_>) -> Result<ZoomView, String> {
    let [w, h, factor, center] = words(fields, "zoom_view")?;
    let factor = finite_str("zoom factor", factor)? as f32;
    if !factor.is_finite() || factor <= 0.0 {
        return Err("invalid zoom factor".into());
    }
    Ok(ZoomView::new(size(w, h)?, factor, finite_str("zoom center", center)? as f32))
}

fn size(w: &str, h: &str) -> Result<(usize, usize), String> {
    let (w, h) = (cast("width", w)?, cast("height", h)?);
    if !(1..=2048).contains(&w) || !(1..=2048).contains(&h) {
        return Err("invalid picture size".into());
    }
    Ok((w, h))
}
