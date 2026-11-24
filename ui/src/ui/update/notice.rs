// SPDX-License-Identifier: GPL-2.0-or-later

use super::{wrap_lines, Ui};

const NOTICE: &str = "/tmp/oos-update.notice";
const DATA_NOTICE: &str = "/tmp/oos-data.notice";

pub(super) fn poll(ui: &mut Ui) {
    if ui.dialog.is_some() || ui.start_up_active() {
        return;
    }
    if !ui.upd.data_notice_shown {
        if let Some(n) = ui.host.text(DATA_NOTICE).filter(|n| !n.trim().is_empty()) {
            ui.upd.data_notice_shown = true;
            show(ui, "Data area", &n);
            return;
        }
    }
    if let Some(n) = ui.host.text(NOTICE) {
        let n = n.trim().to_string();
        if !n.is_empty() && n != ui.upd.notice_shown {
            show(ui, "Update", &n);
            ui.upd.notice_shown = n;
        }
    }
}

fn show(ui: &mut Ui, title: &str, notice: &str) {
    let mut d = vec![title.to_string()];
    d.extend(wrap_lines(notice, 44));
    d.push("Tap to close".into());
    ui.dialog = Some(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_shows_the_data_notice_and_the_update_notice_separately() {
        const MESSAGE: &str = "The data area was repaired";
        let host = crate::ui::host::Host::fixture_with_text(|path| match path {
            DATA_NOTICE => Some(MESSAGE.into()),
            NOTICE => Some("Update completed".into()),
            _ => None,
        });
        let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, host);
        poll(&mut ui);
        let data = ui.dialog.take().unwrap();
        assert_eq!(data[0], "Data area");
        assert_eq!(data[1..data.len() - 1].join(" "), MESSAGE);
        poll(&mut ui);
        assert_eq!(ui.dialog.take().unwrap()[1], "Update completed");
        poll(&mut ui);
        assert!(ui.dialog.is_none());
    }
}
