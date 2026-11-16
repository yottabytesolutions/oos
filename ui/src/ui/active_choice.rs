// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Committed option indices in the order offered by `Ui::options`. Labels are presentation only.

use super::*;

mod analysis;
mod system;
#[cfg(test)]
mod tests;

pub(super) fn index(ui: &Ui, it: Item) -> Option<usize> {
    channel(ui, it)
        .or_else(|| trigger(ui, it))
        .or_else(|| acquisition(ui, it))
        .or_else(|| display(ui, it))
        .or_else(|| decoder(ui, it))
        .or_else(|| serial(ui, it))
        .or_else(|| analysis::index(ui, it))
        .or_else(|| system::index(ui, it))
}

fn position<T: PartialEq>(options: &[T], active: T) -> Option<usize> {
    options.iter().position(|o| *o == active)
}

fn channel(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::ChOn(c) => usize::from(m.ch[c].on),
        Item::ChInvert(c) => usize::from(m.ch[c].invert),
        Item::ChCoupling(c) => return position(board().couplings(), m.ch[c].coupling),
        Item::ChProbe(c) => return position(&[1.0, 10.0, 100.0], m.ch[c].probe),
        Item::ChBw(c) => m.ch[c].bw,
        Item::ChUnit(c) => return position(&AmpUnit::ALL, m.ch[c].unit),
        Item::ChFine => usize::from(m.fine_v),
        Item::HFine => usize::from(m.fine_t),
        Item::ChExpand => usize::from(m.v_expand_gnd),
        Item::HExpandI => {
            return position(&[HExpand::Center, HExpand::Left, HExpand::Right, HExpand::Trigger], m.h_expand)
        }
        Item::MathA | Item::MathB => {
            return position(&u.math_source_ids(m.math_sel), if it == Item::MathA { m.math_a } else { m.math_b })
        }
        Item::MathSel => m.math_sel,
        Item::MathFam => return position(&MathFamily::ALL, u.math_fam),
        Item::MathOpI => return position(&u.math_ops_in_family(), m.math),
        Item::FftWin => return position(&crate::math::WINDOWS, m.fft_win),
        Item::MeasSrc => return position(&u.wave_ids(), u.meas_src),
        Item::MeasSrcB => return position(&u.wave_ids(), u.meas_src_b),
        Item::CatSourceC => return position(&u.wave_ids(), u.cat_c),
        Item::CatSourceD => return position(&u.wave_ids(), u.cat_d),
        Item::CurSrcB => return position(&u.wave_ids(), u.cursor_b_source()),
        Item::RefSrc => u.ref_src,
        Item::RefSel => u.ref_sel,
        Item::RefColor => return u.refs[u.ref_sel].as_ref().map(|r| r.color.min(REF_COLORS.len() - 1)),
        Item::RefShow(i) => return u.refs[i].as_ref().map(|r| usize::from(r.on)),
        Item::MeasStats => usize::from(u.stats_on),
        Item::MeasLabels => usize::from(m.meas_labels),
        Item::MeasOutside => m.meas_out as usize,
        Item::MeasEdges => usize::from(m.meas_data),
        Item::MeasRefs => return position(&[(0.1, 0.9), (0.2, 0.8)], (m.meas_lpct, m.meas_hpct)),
        Item::MeasGate => usize::from(m.meas_full_record),
        Item::DvmSrc | Item::CntSrc if u.offline => u.offline_ref.min(NREF - 1),
        Item::DvmSrc => m.dvm_src,
        Item::CntSrc => m.cnt_src,
        Item::DvmMode => m.dvm_mode.min(2),
        Item::CntMode => m.cnt_mode.min(2),
        Item::DvmOn => usize::from(m.dvm_on),
        Item::CntOn => usize::from(m.cnt_on),
        _ => return None,
    })
}

fn trigger(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::TrigType => return position(&TRIG_TYPES, m.trig_type),
        Item::TrigSrc if m.trig_la_ok() => {
            return m.trig_ref.is_none().then(|| m.trig_line().map_or(m.trig_src, |l| n_ch() + l as usize))
        }
        Item::TrigSrc => return position(&u.trigger_sources(), m.trigger_wave()),
        Item::TrigSrc2 => return position(&u.trigger_sources(), m.trigger_wave2()),
        Item::TrigSlope => m.slope as usize,
        Item::TrigSlope2 => m.trig_slope2 as usize,
        Item::TrigMode => m.trig_mode as usize,
        Item::TrigCoup => return position(&TRIG_COUPS, m.trig_coup),
        Item::TrigNoise => usize::from(m.trig_nrej),
        Item::TrigAutoNoise => usize::from(m.auto_nrej),
        Item::TrigPol => usize::from(m.trig_pol != Pol::Pos),
        Item::TrigWhen if m.trig_type == TrigType::Runt => m.trig_when as usize,
        Item::TrigWhen => return (m.trig_when as usize).checked_sub(1),
        Item::VidStd => m.video_std as usize,
        Item::VidSync => m.video_sync as usize,
        Item::Pat(c) => return position(&[Bit::H, Bit::L, Bit::X, Bit::R, Bit::F], m.pat[c]),
        Item::WinModeI => m.win_mode as usize,
        Item::SearchOn => usize::from(u.search_on),
        _ => return None,
    })
}

fn acquisition(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::AcqMode => m.acq as usize,
        Item::AcqDepth => m.mdepth,
        Item::AcqAvg => return position(&AVERAGES, m.avg_count),
        Item::AcqSeg => return position(&u.seg_options(), if m.segments == 0 { 0 } else { u.seg_count() }),
        Item::AcqSegIvl => return position(&SEG_IVLS, m.seg_ivl),
        Item::AcqSegHw => usize::from(m.seg_hw),
        Item::AcqAvgHw => usize::from(m.avg_hw),
        Item::AcqCores => usize::from(m.cores),
        Item::AcqDrawPath => usize::from(m.draw_batch),
        Item::Zoom => usize::from(m.zoom),
        Item::AutoRoll => usize::from(m.auto_roll),
        Item::LogOn => usize::from(u.log.file.is_some()),
        Item::Persist => m.persist,
        Item::ColorGrade => usize::from(m.color_grade),
        Item::MaskOn => usize::from(m.mask_on),
        Item::MaskSrc => m.mask_src,
        Item::MaskAction => usize::from(m.mask_stop),
        Item::MaskInvert => usize::from(m.mask_inv),
        Item::ZoneSel => u.zone_sel,
        Item::ZoneOn => usize::from(m.zones[u.zone_sel].on),
        Item::ZoneSrc => m.zones[u.zone_sel].src,
        Item::ZoneMode => usize::from(!m.zones[u.zone_sel].inside),
        Item::ZInput => m.z_src.map_or(0, |c| c + 1),
        Item::Histogram => m.hist as usize,
        _ => return None,
    })
}

fn display(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::GridStyle => m.grid as usize,
        Item::GridFrame => usize::from(m.grid_frame),
        Item::Dots => return position(&DOTS_MODES, m.dots),
        Item::BottomBar => usize::from(m.bar_compact),
        Item::Hdmi => return position(&HDMI_MODES, m.hdmi),
        Item::TextSize => return position(&TEXT_SIZES, m.text_size),
        Item::TouchLock => usize::from(m.touch_lock),
        Item::CurMode => m.cursor as usize,
        Item::XyMode => usize::from(u.xy),
        Item::StripDock => return position(&StripDock::ALL, m.strip_dock),
        Item::PaneSel => m.pane.min(tables::PANES.len() - 1),
        Item::TrendSel(k) => {
            if m.trend_sel[k] < u.trend_options().len() {
                m.trend_sel[k]
            } else {
                0
            }
        }
        Item::WinLayout => return position(&Density::ALL, m.density),
        Item::WinCh(c) => m.ch_win[c].min(m.win_layout.count() - 1),
        Item::WinRef(i) => m.ref_win[i].min(m.win_layout.count() - 1),
        Item::WinMath(k) => m.math_win[k].min(m.win_layout.count() - 1),
        Item::WinLa => m.la_win.min(m.win_layout.count() - 1),
        Item::Views => m.views.max(1) - 1,
        Item::HdmiView => m.hdmi_view,
        _ => return None,
    })
}

fn decoder(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::DecOn => usize::from(m.dec_on),
        Item::DecSel => m.dec_sel,
        Item::DecProto => return position(&u.protos_on(), m.dec_proto),
        Item::DecSrc | Item::DecSrc2 | Item::DecCs | Item::DecSrc4 if m.dec_on_la() => {
            let role = match it {
                Item::DecSrc2 => 1,
                Item::DecCs => 2,
                Item::DecSrc4 => 3,
                _ => 0,
            };
            let extra = usize::from(role >= 2);
            return m.dec_la[role].map(|n| n as usize + extra).or_else(|| (extra == 1).then_some(0));
        }
        Item::DecSrc => return position(&u.decoder_sources(), m.dec_src),
        Item::DecSrc2 => return position(&u.decoder_sources(), m.dec_src2),
        Item::DecCs => return optional_decoder(u, m.dec_cs, m.dec_proto != Proto::CPHY),
        Item::DecSrc4 => return optional_decoder(u, m.dec_src4, true),
        Item::DecDiff => return optional_decoder(u, m.dec_diff, true),
        Item::DecBaud => m.dec_baud,
        Item::DecCphyRate => return position(&crate::decode::CPHY_RATES, m.dec_cphy_rate),
        Item::DecCanRate => m.dec_canrate,
        Item::DecCanH => usize::from(m.dec_canh),
        Item::DecUsbSpeed => m.dec_uspeed,
        Item::DecFmt => usize::from(m.dec_ascii),
        Item::DecOpt(k) => m.dec_opts()[k],
        Item::DecMode => m.dec_mode as usize,
        Item::DecParity => m.dec_parity as usize,
        Item::TrigBusWhat => return position(&BUS_WHATS, m.trig_bus),
        _ => return None,
    })
}

fn optional_decoder(u: &Ui, active: Option<usize>, none: bool) -> Option<usize> {
    match active {
        Some(id) => position(&u.decoder_sources(), id).map(|i| i + usize::from(none)),
        None => none.then_some(0),
    }
}

fn serial(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    let s = &m.ser;
    Some(match it {
        Item::SerRel => {
            return position(
                &RELS,
                if m.dec_proto == Proto::I2C && m.trig_bus == BusWhat::Id { s.addr_rel } else { s.rel },
            )
        }
        Item::UartPol => usize::from(s.uart_inv),
        Item::UartStop => return position(&STOP_BITS, s.uart_stop),
        Item::UartErr => return position(&UART_ERRS, s.uart_err),
        Item::SpiCs => usize::from(s.spi_cs_high),
        Item::I2cDir => return position(&I2C_DIRS, s.dir),
        Item::I2cAddrBits => usize::from(s.ten_bit),
        Item::I2cBytes => s.bytes as usize,
        _ => return None,
    })
}

/// Selection fill and contrasting ink, shared with compact menu source selectors.
pub(super) fn paint(cv: &mut Canvas, r: Rect, active: bool) -> Rgb {
    if active {
        cv.round(r, 5, ACCENT, 255);
        DARK_TEXT
    } else {
        btn(cv, r, BTN, BTN_BORDER);
        TEXT
    }
}

/// A source selector keeps its channel identity visible beside the contrasting caption.
pub(super) fn source(cv: &mut Canvas, r: Rect, color: Rgb) -> Rgb {
    let ink = paint(cv, r, true);
    cv.fill(Rect::new(r.x + 2, r.y + 6, 3, r.h - 12), color);
    ink
}

/// The value surface shared by numeric steppers and collapsed option pickers.
pub(super) fn value(u: &Ui, cv: &mut Canvas, it: Item, mid: Rect, pick: bool) {
    let ink = paint(cv, mid, index(u, it).is_some());
    let text = u.value_shown(it);
    cv.text_in(u.value_font(mid, pick, &text), mid, 1, &text, ink);
    if pick {
        chevron_down(cv, mid.x + mid.w - 20, mid.y + mid.h / 2 - 3, ink);
    }
}
