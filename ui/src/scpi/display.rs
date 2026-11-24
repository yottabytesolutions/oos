// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn exec(
    scpi: &mut Scpi,
    ui: &mut Ui,
    cv: &Canvas,
    cmd: &str,
    nodes: &[&str],
    query: bool,
    arg: &str,
) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "DISPlay") && !(nodes.len() == 2 && kw(nodes[1], "DATA")) && !(nodes.len() >= 2 && kw(nodes[1], "VIEW")) {
        return scpi.display_cmd(ui, &nodes[1..], query, arg);
    }
    if kw(n0, "DISPlay") && nodes.len() == 2 && kw(nodes[1], "DATA") && query {
        return data(scpi, cv, arg);
    }
    // Display > Views (ui/views.rs): :DISPlay:VIEW <n> shows view n, :DISPlay:VIEW:COUNt <1..4> how many there are
    if kw(n0, "DISPlay") && nodes.len() >= 2 && kw(nodes[1], "VIEW") {
        return views(scpi, ui, nodes, query, arg);
    }
    // OOS extensions: analyser view, Identify, self-calibration, Bode
    if kw(n0, "VIEW") && nodes.len() == 1 {
        return view(scpi, ui, query, arg);
    }

    super::undefined(scpi, cmd)
}

#[derive(Clone, Copy)]
pub(super) enum Format {
    Bmp,
    Png,
}

impl Format {
    fn parse(arg: &str) -> Result<Self, &str> {
        let kind = arg.split_whitespace().next().unwrap_or("");
        let up = kind.to_ascii_uppercase();
        if up.starts_with("PNG") {
            Ok(Self::Png)
        } else if up.is_empty() || up.starts_with("BMP") {
            Ok(Self::Bmp)
        } else {
            Err(kind)
        }
    }

    pub(super) fn encode(self, cv: &Canvas) -> Vec<u8> {
        block(match self {
            Self::Bmp => bmp(cv),
            Self::Png => crate::store::png_of(cv),
        })
    }
}

fn data(scpi: &mut Scpi, cv: &Canvas, arg: &str) -> Option<Vec<u8>> {
    match Format::parse(arg) {
        Ok(format) => Some(format.encode(cv)),
        Err(kind) => {
            scpi.push_err(format!("-224,\"Illegal parameter value\" {kind}"));
            None
        }
    }
}

pub(super) fn data_header(header: &str) -> bool {
    header.trim_start_matches(':').split_once(':').is_some_and(|(root, leaf)| {
        (root.eq_ignore_ascii_case("DISP") || root.eq_ignore_ascii_case("DISPLAY"))
            && leaf.trim_end_matches('?').eq_ignore_ascii_case("DATA")
    })
}

pub(super) fn capture_format(command: &str) -> Option<Format> {
    let (header, arg) = command.split_once(char::is_whitespace).unwrap_or((command, ""));
    (header.ends_with('?') && data_header(header)).then(|| Format::parse(arg).ok()).flatten()
}

fn views(scpi: &mut Scpi, ui: &mut Ui, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let count = nodes.len() == 3 && kw(nodes[2], "COUNt");
    // Display > Monitor view (ui/monitor.rs): the view an HDMI monitor in Layout shows, 0 the panel's
    if nodes.len() == 3 && kw(nodes[2], "MONitor") {
        if query {
            return text_reply(&ui.m.hdmi_view.to_string());
        }
        match arg.trim().parse::<usize>().ok() {
            Some(n) if n <= ui.m.views && (n == 0 || ui.m.views >= 2) => ui.m.hdmi_view = n,
            _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
        }
        return None;
    }
    if nodes.len() > 3 || (nodes.len() == 3 && !count) {
        scpi.push_err("-113,\"Undefined header\"".into());
        return None;
    }
    if query {
        return text_reply(&(if count { ui.m.views } else { ui.m.view_cur + 1 }).to_string());
    }
    let n = arg.trim().parse::<usize>().ok();
    match n {
        Some(n) if count && (1..=crate::ui::VIEWS_MAX).contains(&n) => ui.views_set(n),
        Some(n) if !count && (1..=ui.m.views).contains(&n) => ui.view_go(n - 1),
        _ => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

fn view(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    let views = crate::ui::scpi_views();
    if query {
        return text_reply(&format!("{:?}", ui.m.view).to_ascii_uppercase());
    }
    match views.iter().find(|(k, _)| kw(arg, k)) {
        Some((_, v)) => ui.set_analysis_view(*v),
        None => scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg)),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::readback::Readback;

    #[test]
    fn display_queries_resume_in_order_with_one_fresh_picture_each() {
        let mut scpi = Scpi::disconnected();
        scpi.use_readback(Readback::default());
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(2, 2);
        let mut program = Program::new(":DISP:DATA? BMP;:DISPLAY:DATA? PNG;*ESR?");
        assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_none());
        let first = scpi.readback().unwrap().take().unwrap();
        assert!(scpi.readback().unwrap().take().is_none());
        assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_none());

        cv.clear(0x23_45_67);
        first.complete(&cv);
        let mut expected = Format::Bmp.encode(&cv);
        assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_none());
        let second = scpi.readback().unwrap().take().unwrap();
        assert!(scpi.readback().unwrap().take().is_none());

        cv.clear(0x12_34_56);
        second.complete(&cv);
        expected.push(b';');
        expected.extend(Format::Png.encode(&cv));
        expected.extend_from_slice(b";0");
        assert_eq!(scpi.poll_program(&mut ui, &cv, &mut program), Some(expected));
        assert!(scpi.readback().unwrap().take().is_none());
    }
}
