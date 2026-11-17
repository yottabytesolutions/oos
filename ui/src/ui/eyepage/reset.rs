// SPDX-License-Identifier: GPL-2.0-or-later

//! Reset temporal data while retaining the page's warmed buffers and labels.

use super::*;

pub(in crate::ui) fn clear(p: &mut Page) {
    let Page {
        pool,
        head,
        means,
        edges,
        records,
        rms,
        ui,
        step,
        step2,
        t,
        short,
        status,
        title,
        img,
        img_w,
        levels,
        vstep,
        charts,
        trend,
        hist,
        spec,
        tub,
        measured,
        model,
        eye_measured,
        eye_model,
        stats,
        readings,
    } = p;
    pool.clear();
    (*head, *edges, *records, *short) = (0, 0, 0, 0);
    *means = EdgeMeans::default();
    (*rms, *ui) = (Run::default(), Run::default());
    (*step, *step2) = ((0.0, 0), (0.0, 0));
    *t = f64::NEG_INFINITY;
    status[0].clear();
    status[0].push_str("Waiting for edges");
    status[1].clear();
    title.clear();
    img.clear();
    (*img_w, *levels, *vstep) = (0, None, 0.0);
    clear_charts(charts);
    trend.clear();
    hist.clear();
    spec.clear();
    *tub = Bathtub::default();
    measured.clear();
    model.clear();
    eye_measured.clear();
    eye_model.clear();
    stats.clear();
    readings.clear();
}

fn clear_charts(charts: &mut [Chart; 4]) {
    for (chart, title) in charts.iter_mut().zip(["TIE trend", "TIE histogram", "TIE spectrum", "Bathtub (model)"]) {
        chart.title.clear();
        chart.title.push_str(title);
        chart.empty.clear();
        chart.empty.push_str("Waiting for edges");
        chart.x.0.clear();
        chart.x.1.clear();
        chart.y.0.clear();
        chart.y.1.clear();
    }
}
