// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Identity readers for analyser, generator, logic and simulated-source options.
use super::{position, *};

pub(super) fn index(u: &Ui, it: Item) -> Option<usize> {
    views(u, it).or_else(|| sources(u, it)).or_else(|| settings(u, it)).or_else(|| logic(u, it))
}

fn views(u: &Ui, it: Item) -> Option<usize> {
    let view = match it {
        Item::SpecOn => AView::Spectrum,
        Item::EyeOn => AView::Eye,
        Item::PowOn => AView::Power,
        Item::BodeOn => AView::Bode,
        Item::BwOn => AView::Step,
        Item::MagOn => AView::Magnetic,
        Item::InvOn => AView::Inverter,
        Item::DptOn => AView::Dpt,
        _ => return None,
    };
    Some(usize::from(!u.xy && u.m.view == view))
}

fn sources(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    let active = match it {
        Item::SpecSrc => m.spec_src,
        Item::EyeSrc => m.eye_src,
        Item::PowV => m.pw_v,
        Item::PowI => m.pw_i,
        Item::MagV => m.mag_v,
        Item::MagI => m.mag_i,
        Item::InvA => m.inv_a,
        Item::InvB => m.inv_b,
        Item::InvC => m.inv_c,
        Item::InvAn => m.inv_an,
        Item::DptV => m.dpt_v,
        Item::DptI => m.dpt_i,
        Item::DptG => m.dpt_g,
        Item::Dpt4 => m.dpt_4ch,
        Item::BodeIn if m.bode_gen == bode::FREE => return Some(m.bode_sw_in),
        Item::BodeIn => m.bode_in,
        Item::BodeOut => m.bode_out,
        Item::BusClk | Item::BusData => {
            return position(&u.bus_ids(), if it == Item::BusClk { m.bus.clk } else { m.bus.data });
        }
        _ => {
            let active = match it {
                Item::EyeB => m.eye_b,
                Item::EyeRail => m.eye_rail,
                Item::PowVin => m.pw_vin,
                Item::PowClk => m.pw_clk,
                Item::InvTq => m.inv_tq,
                Item::InvEv => m.inv_ev,
                Item::InvEi => m.inv_ei,
                _ => return None,
            };
            return active.map_or(Some(0), |id| position(&u.wave_ids(), id).map(|i| i + 1));
        }
    };
    position(&u.wave_ids(), active)
}

fn settings(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    Some(match it {
        Item::ProtoWatch => usize::from(u.azr.watch),
        Item::EyePlace => m.eye_place as usize,
        Item::SpecHarm => m.spec_harm.min(PANE_HARMS.len() - 1),
        Item::SpecWin => return position(&crate::math::WINDOWS, m.spec_win),
        Item::SpecTrace => m.spec_trace.min(3),
        Item::SpecWf => usize::from(m.spec_wf),
        Item::SpecDist => usize::from(m.spec_dist),
        Item::EyeHp => usize::from(m.eye_hp),
        Item::EyeNpj => usize::from(m.eye_npj),
        Item::PowMode => m.pw_mode as usize,
        Item::MagPlot => usize::from(m.mag_plot == MagPlot::BH && m.mag_ae > 0.0 && m.mag_le > 0.0),
        Item::InvPage => m.inv_page as usize,
        Item::InvAng => {
            let k = m.inv_ang as usize;
            if m.inv_page == InvPage::Mech {
                k.max(1) - 1
            } else {
                k
            }
        }
        Item::InvQty => usize::from(!m.inv_volt),
        Item::InvElec => usize::from(m.inv_elec),
        Item::InvSwap => usize::from(m.inv_swap),
        Item::DptFourth => m.dpt_4th as usize,
        Item::DptZoom => m.dpt_zoom as usize,
        Item::BodeMode => return position(&bode::BODE_MODES, m.bode_mode),
        Item::BodeGen => m.bode_gen,
        Item::BodeLink => usize::from(m.bode_usb),
        Item::BodeSwLog => usize::from(m.bode_sw_log),
        Item::BodeSync => m.bode_sync,
        Item::BodeSyncEdge => usize::from(m.bode_sync_slope == Slope::Fall),
        Item::BwSrc => m.bw_src,
        Item::BwHint => return position(&bwtest::HINTS, m.bw_hint),
        Item::BwBins => return position(&bwtest::BINS, m.bw_bins),
        Item::AfgSel => m.gen_sel,
        Item::AfgOn => usize::from(m.afg_on),
        Item::AfgWave => m.afg_wave,
        Item::AfgMod => m.afg_mod,
        Item::AfgModShape => m.afg_mod_shape,
        Item::AfgArbSrc => m.afg_arb_src,
        Item::BusOn => usize::from(m.bus.on),
        Item::BusRw => usize::from(m.bus.addr_rw),
        _ => return None,
    })
}

fn logic(u: &Ui, it: Item) -> Option<usize> {
    let m = &u.m;
    let l = &m.la;
    Some(match it {
        Item::LaOn => usize::from(l.on),
        Item::LaShow => la::GROUPS.iter().take(3).position(|g| g.1 == l.show).unwrap_or(3),
        Item::LaThr(p) => l.preset[p.min(1)].min(la::USER),
        Item::LaHeight => l.height,
        Item::LaLine(n) => usize::from(l.show >> n.min(15) & 1 == 1),
        Item::LaPat(n) => return position(&[Bit::H, Bit::L, Bit::X], m.pat_la[n.min(15)]),
        Item::DecIn => usize::from(m.dec_la_in),
        Item::CalCh => u.cal.ch,
        Item::CalZero => usize::from(u.cal.zero),
        Item::CalAdc => usize::from(u.cal.adc),
        Item::SimPreset => m.sim_preset,
        Item::SimPat => return position(&sim::PRBS, m.sim_prbs),
        Item::SimSsc => usize::from(m.sim_ssc),
        Item::SimSscCenter => usize::from(m.sim_ssc_center),
        Item::SimSlot => m.sim_slot,
        Item::SimShow => return u.refs[m.sim_slot].as_ref().map(|r| usize::from(r.on)),
        Item::Offline => u.look as usize,
        _ => return None,
    })
}
