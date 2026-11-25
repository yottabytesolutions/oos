// SPDX-License-Identifier: GPL-2.0-or-later

//! Capture geometry and the settings actually used, for the host golden inventory.

use crate::ui::Ui;

pub fn metadata(ui: &Ui, path: &str) -> Result<(), String> {
    if shows_release_version(ui) {
        println!("release-version {} {}", env!("CARGO_PKG_VERSION"), path);
    }
    let plots = if ui.m.zoom {
        let (a, b) = Ui::split_zoom(ui.grid);
        vec![("main", a), ("zoom", b)]
    } else {
        ui.wins.iter().enumerate().map(|(i, r)| (if i == 0 { "main" } else { "window" }, *r)).collect()
    };
    let regions = plots
        .iter()
        .enumerate()
        .map(|(i, (name, r))| format!("{}{} {} {} {} {}\n", name, i, r.x, r.y, r.w, r.h))
        .collect::<String>();
    let frame = match &ui.src {
        crate::wave::Source::Hw { shared, .. } => {
            let g = oos_hal::sync::lock(shared);
            format!(
                "acq={:?} envelope={} roll={} scan={} partial={} hw_avg={}",
                g.frame.acq, g.frame.envelope, g.frame.roll, g.frame.scan, g.frame.partial, g.frame.hw_avg
            )
        }
        _ => "Demo records".into(),
    };
    let settings = format!(
        "settings dots={:?} channels={} tdiv={} persist={:?} grade={} zoom={} acquisition={:?} run={:?} frames={} records={} menu_open={} {}\n",
        ui.m.dots,
        ui.m.ch.iter().filter(|c| c.on).count(),
        ui.m.time_per_div(),
        ui.m.persist,
        ui.m.color_grade,
        ui.m.zoom,
        ui.m.acq,
        ui.m.run,
        ui.frames,
        ui.records,
        ui.menu_open,
        frame
    );
    std::fs::write(format!("{}.meta", path), regions + &settings).map_err(|e| format!("capture metadata: {}", e))
}

fn shows_release_version(ui: &Ui) -> bool {
    let release = format!("oos-ui {}", env!("CARGO_PKG_VERSION"));
    let Some(dialog) = &ui.dialog else { return false };
    if let Some(columns) = &ui.sysinfo {
        columns.iter().flatten().any(
            |row| matches!(row, crate::ui::sysinfo::Row::Kv(label, value, _) if label == "OOS" && value == &release),
        )
    } else {
        dialog.iter().any(|line| line == &release)
    }
}
