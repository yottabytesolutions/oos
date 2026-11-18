// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

/// Read-only rows in display and navigation order. Visibility never changes option identity.
pub(super) struct Descriptor {
    item: Item,
    kind: Kind,
    label: &'static str,
    value: fn(&Ui) -> String,
    options: fn(&Ui) -> Vec<String>,
    visible: fn(&Ui) -> bool,
}

impl Descriptor {
    const fn new(
        item: Item,
        kind: Kind,
        label: &'static str,
        value: fn(&Ui) -> String,
        options: fn(&Ui) -> Vec<String>,
        visible: fn(&Ui) -> bool,
    ) -> Self {
        Self { item, kind, label, value, options, visible }
    }
}

fn toggles(_: &Ui) -> Vec<String> {
    ["Off", "On"].into_iter().map(String::from).collect()
}

fn onoff(on: bool) -> String {
    if on { "On" } else { "Off" }.into()
}

struct Area {
    descriptors: &'static [Descriptor],
    sources: fn(&Ui, MenuId) -> Vec<WaveId>,
    kind: fn(Item) -> Option<Kind>,
    label: fn(Item) -> Option<&'static str>,
    value: fn(&Ui, Item) -> Option<String>,
    // True when the area already handled change acceptance; otherwise Ui accepts the edit.
    adjust: fn(&mut Ui, Item, i32) -> bool,
    activate: fn(&mut Ui, Item) -> bool,
    options: fn(&Ui, Item) -> Option<Vec<String>>,
    menu_items: fn(&Ui, MenuId) -> Vec<Item>,
}

impl Area {
    const EMPTY: Self = Self {
        descriptors: &[],
        sources: |_, _| Vec::new(),
        kind: |_| None,
        label: |_| None,
        value: |_, _| None,
        adjust: |_, _, _| false,
        activate: |_, _| false,
        options: |_, _| None,
        menu_items: |_, _| Vec::new(),
    };
}

mod acquire;
mod backlight;
mod bandwidth;
mod bode;
mod bus;
mod calibration;
mod channel;
mod cursor;
mod decode;
mod display;
mod dpt;
mod dvm;
mod eye;
mod features;
mod generator;
mod graphics;
mod horizontal;
mod identify;
mod inverter;
mod logger;
mod logic;
mod magnetic;
mod mask;
mod math;
mod measure;
mod monitor;
mod navigation;
mod network;
mod offline;
mod picker;
mod power;
mod reference;
mod remote;
mod serial;
mod simulation;
mod spectrum;
mod ssh;
mod storage;
mod system;
mod time;
mod trigger;
mod view_more;
mod views;
mod windows;
mod zone;

macro_rules! areas {
    ($($area:ident { [$($item:pat_param)|*], [$($menu:pat_param)|*] })*) => {

        #[deny(unreachable_patterns)]
        fn for_item(it: Item) -> &'static Area {
            match it {
                Item::Open(menu) => for_menu(menu),
                $($( $item => &$area::AREA, )*)*
            }
        }

        #[deny(unreachable_patterns)]
        fn for_menu(menu: MenuId) -> &'static Area {
            match menu {
                $($( $menu => &$area::AREA, )*)*
            }
        }
    };
}

areas! {
    decode { [Item::DecSel | Item::DecTable | Item::DecExport | Item::DecOn | Item::DecSrc | Item::DecBaud | Item::DecCanRate | Item::DecCanH | Item::DecUsbSpeed | Item::DecOpt(..) | Item::DecSrc4 | Item::DecDiff | Item::DecThr | Item::DecFmt | Item::DecProto | Item::DecSrc2 | Item::DecCs | Item::DecMode | Item::DecBits | Item::DecParity | Item::DecCphyRate], [MenuId::Decode] }
    trigger { [Item::SearchTable | Item::SearchSave | Item::TrigBusWhat | Item::TrigBusVal | Item::TrigSrc | Item::TrigSlope | Item::TrigMode | Item::TrigLevel | Item::Trig50 | Item::TrigType | Item::SearchOn | Item::TrigLevel2 | Item::TrigSrc2 | Item::TrigSlope2 | Item::TrigPol | Item::TrigWhen | Item::TrigT1 | Item::TrigT2 | Item::VidStd | Item::VidSync | Item::VidLine | Item::Pat(..) | Item::PatLevel | Item::WinModeI | Item::NthIdle | Item::NthN | Item::Setup | Item::Hold | Item::TrigHoldoff | Item::TrigNoise | Item::TrigAutoNoise | Item::TrigCoup], [MenuId::Trigger] }
    storage { [Item::StoreBin | Item::StoreSlot | Item::StoreDest | Item::StoreSaveSetup | Item::StoreRecall | Item::StoreCsv | Item::StorePng | Item::StoreFfSave | Item::StoreFfLoad], [MenuId::Storage] }
    display { [Item::PaneSel | Item::XyMode | Item::Persist | Item::Intensity | Item::GridStyle | Item::ClearDisp | Item::Dots | Item::BottomBar | Item::Hdmi | Item::TextSize | Item::ZInput | Item::Histogram | Item::TrendSel(..) | Item::GridBright | Item::GridFrame | Item::ColorGrade], [MenuId::Display] }
    serial { [Item::SerRel | Item::SerMax | Item::SerMask | Item::UartPol | Item::UartStop | Item::UartErr | Item::SpiCs | Item::SpiTimeout | Item::I2cDir | Item::I2cAddrBits | Item::I2cBytes], [] }
    bus { [Item::BusOn | Item::BusLabel | Item::BusPos | Item::BusClk | Item::BusClkThr | Item::BusData | Item::BusDataThr | Item::BusRw], [MenuId::Bus] }
    horizontal { [Item::Zoom | Item::AutoRoll | Item::ZScale | Item::ZDelay | Item::HScale | Item::HDelay | Item::HZero | Item::AutoDeskew | Item::ClearDeskew | Item::HFine | Item::HExpandI], [MenuId::Horizontal] }
    measure { [Item::MeasStats | Item::MeasLabels | Item::StripDock | Item::MeasSrc | Item::MeasCopyTo | Item::MeasAdd(..) | Item::AddMeas | Item::MeasCfg(..) | Item::MeasPair(..) | Item::MeasSrcB | Item::MeasClear | Item::MeasIdle | Item::MeasOutside | Item::MeasN | Item::MeasEdges | Item::MeasRefs | Item::MeasGate | Item::MeasTsetup | Item::MeasThold | Item::MeasRemove(..) | Item::CatSourceC | Item::CatSourceD], [MenuId::Measure | MenuId::MeasAdd | MenuId::MeasHold | MenuId::MeasPair | MenuId::MeasSelected] }
    reference { [Item::RefSrc | Item::RefSave | Item::RefShow(..) | Item::RefClear | Item::RefColor | Item::RefLabel | Item::RefSel | Item::RefVpd | Item::RefOffset | Item::RefExport | Item::RefImport | Item::RefLoad], [MenuId::Ref] }
    math { [Item::MathOpI | Item::MathFam | Item::MathA | Item::MathB | Item::MathScale | Item::MathOffset | Item::FftWin | Item::MathFc | Item::MathAuto | Item::MathSel | Item::MathBw | Item::MathExprI | Item::MathExprEdit | Item::MathLinA | Item::MathLinB], [MenuId::Math] }
    navigation { [Item::SrcOn | Item::RunStop | Item::SingleAct | Item::AutosetAct | Item::DefaultAct | Item::SlopeAct | Item::ForceAct | Item::NavHist | Item::NavBack | Item::NavFwd | Item::QuickRun | Item::LockAct | Item::PowerAct], [MenuId::Main] }
    channel { [Item::ChOn(..) | Item::ChCoupling(..) | Item::ChBw(..) | Item::ChProbe(..) | Item::ChInvert(..) | Item::ChScale(..) | Item::ChOffset(..) | Item::ChLabel(..) | Item::ChFine | Item::ChUnit(..) | Item::ChBias(..) | Item::ChBiasZero(..) | Item::ChSkew(..) | Item::ChExpand], [MenuId::Channel(..) | MenuId::ChMore(..)] }
    acquire { [Item::AcqMode | Item::AcqAvg | Item::AcqDepth | Item::AcqSeg | Item::AcqCores | Item::AcqDrawPath | Item::AcqAlias | Item::AcqSegHw | Item::AcqAvgHw | Item::AcqSegIvl | Item::AcqSegTable], [MenuId::Acquire] }
    dvm { [Item::DvmOn | Item::DvmSrc | Item::DvmMode | Item::CntOn | Item::CntSrc | Item::CntMode | Item::CntDigits | Item::CntClear], [MenuId::Dvm] }
    logger { [Item::LogOn | Item::LogIvl | Item::LogInfo], [MenuId::Logger] }
    spectrum { [Item::SpecLimitSet | Item::SpecLimitClear | Item::SpecMargin | Item::SpecOn | Item::SpecSrc | Item::SpecCenter | Item::SpecSpan | Item::SpecRbw | Item::SpecWin | Item::SpecTrace | Item::SpecRef | Item::SpecWf | Item::SpecDist | Item::SpecAcprW | Item::SpecAcprS | Item::SpecPeak | Item::SpecHarm], [MenuId::Spectrum] }
    cursor { [Item::CurC | Item::CurD | Item::CurMode | Item::CurSrcB | Item::CurA | Item::CurB], [MenuId::Cursor] }
    mask { [Item::MaskSave | Item::MaskLoad | Item::MaskOn | Item::MaskSrc | Item::MaskX | Item::MaskY | Item::MaskAction | Item::MaskInvert | Item::MaskCreate | Item::MaskFromRef | Item::MaskReset | Item::MaskClear], [MenuId::Mask] }
    windows { [Item::WinLayout | Item::WinCh(..) | Item::WinRef(..) | Item::WinMath(..) | Item::WinLa], [MenuId::Windows] }
    features { [Item::FeatOn(..) | Item::FeatDec(..)], [MenuId::Features | MenuId::Decoders(..)] }
    monitor { [Item::HdmiView], [] }
    eye { [Item::EyeOn | Item::EyeAct | Item::EyePlace | Item::EyeSrc | Item::EyeRate | Item::EyeClear | Item::EyeBer | Item::EyeB | Item::EyeHp | Item::EyePnLo | Item::EyePnHi | Item::EyeNpj | Item::EyeRail | Item::EyeNotch], [MenuId::Eye] }
    power { [Item::PowOn | Item::PowV | Item::PowI | Item::PowMode | Item::PowFline | Item::PowFsw | Item::PowVin | Item::PowRef | Item::PowClk], [MenuId::Power] }
    identify { [Item::IdentifyAct | Item::ProtoWatch | Item::IdFind(..)], [MenuId::Identify] }
    generator { [Item::AfgSel | Item::AfgOn | Item::AfgArbSrc | Item::AfgArbGrab | Item::AfgArbFile | Item::AfgFreq | Item::AfgFreqEntry | Item::AfgAmpl | Item::AfgOffset | Item::AfgWave | Item::AfgDuty | Item::AfgSym | Item::AfgMod | Item::AfgModRate | Item::AfgModDepth | Item::AfgModShape], [MenuId::Afg] }
    logic { [Item::LaOn | Item::DecIn | Item::LaShow | Item::LaThr(..) | Item::LaUser(..) | Item::LaHeight | Item::LaLine(..) | Item::LaPat(..)], [MenuId::La | MenuId::LaLines | MenuId::LaPat] }
    bandwidth { [Item::BwOn | Item::BwSrc | Item::BwHint | Item::BwBins], [MenuId::BwTest] }
    bode { [Item::BodeOn | Item::BodeRun | Item::BodeStart | Item::BodeStop | Item::BodePoints | Item::BodeAmp | Item::BodeIn | Item::BodeOut | Item::BodeGen | Item::BodeAddr | Item::BodeLink | Item::BodeCur | Item::BodeMode | Item::BodeSwTime | Item::BodeSwLog | Item::BodeSync | Item::BodeSyncEdge | Item::BodeSyncLvl | Item::BodeRef], [MenuId::Bode] }
    calibration { [Item::CalCh | Item::CalZero | Item::CalAdc | Item::CalStart | Item::CalCancel | Item::CalStatus], [MenuId::SelfCal] }
    zone { [Item::ZoneSel | Item::ZoneOn | Item::ZoneSrc | Item::ZoneMode | Item::ZoneBox | Item::ZoneShape | Item::ZoneClear], [MenuId::Zone] }
    system { [Item::About | Item::SysInfo | Item::Shot | Item::AuxOut | Item::Stock | Item::Update | Item::UpdateRollback | Item::BootMode | Item::Overclock | Item::QuickAct], [MenuId::Utility] }
    view_more { [Item::ViewExit | Item::ViewSettings], [MenuId::ViewMore] }
    network { [Item::NetEth | Item::NetMac | Item::NetPorts | Item::NetWlan | Item::WlanInfo | Item::WlanScan | Item::WlanForget | Item::WlanNet | Item::WlanForgetNet], [MenuId::Network] }
    remote { [Item::NetWeb | Item::RemWeb | Item::RemScpi | Item::RemVxi | Item::RemTelnet | Item::RemSsh | Item::RemScpiInfo | Item::RemVxiInfo | Item::RemTelnetInfo | Item::RemMdns | Item::RemName | Item::RemAllow | Item::RemFirewall], [MenuId::Remote] }
    ssh { [Item::SshState | Item::SshPassword | Item::SshShowPass | Item::SshHostKey | Item::SshKeys | Item::SshKeyList | Item::SshImport | Item::SshRemove | Item::SshRemoveKey], [MenuId::Ssh] }
    graphics { [Item::Graphics], [] }
    views { [Item::Views], [] }
    backlight { [Item::Backlight], [] }
    time { [Item::NetRetry10 | Item::TimeNow | Item::NtpOn | Item::NtpServer | Item::NtpStatus | Item::NtpSync | Item::TimeZone | Item::TimeSet], [MenuId::Time] }
    picker { [Item::PickOpt(..)], [MenuId::Pick] }
    magnetic { [Item::MagOn | Item::MagV | Item::MagI | Item::MagN | Item::MagAe | Item::MagLe | Item::MagRcu | Item::MagPlot | Item::MagCur], [MenuId::Magnetic] }
    inverter { [Item::InvOn | Item::InvPage | Item::InvA | Item::InvB | Item::InvC | Item::InvAng | Item::InvPoles | Item::InvAn | Item::InvQty | Item::InvTq | Item::InvEv | Item::InvEi | Item::InvScale | Item::InvDeg | Item::InvOff | Item::InvElec | Item::InvQa | Item::InvQb | Item::InvQi | Item::InvCpr | Item::InvSwap | Item::InvHa | Item::InvHb | Item::InvHc], [MenuId::Inverter] }
    dpt { [Item::DptOn | Item::DptV | Item::DptI | Item::DptG | Item::DptFourth | Item::Dpt4 | Item::DptPct | Item::DptZoom], [MenuId::Dpt] }
    simulation { [Item::SimPreset | Item::SimSlot | Item::SimPlay | Item::SimLive | Item::SimShow | Item::SimRate | Item::SimPat | Item::SimHigh | Item::SimLow | Item::SimRise | Item::SimBw | Item::SimDeemph | Item::SimNoise | Item::SimSsc | Item::SimSscCenter | Item::SimSscDeviation | Item::SimSscHz | Item::SimDcd | Item::SimCorrRj | Item::SimCorrHz | Item::SimRj | Item::SimPj | Item::SimPjHz | Item::SimDefaults], [MenuId::Sim | MenuId::SimAdvanced] }
    offline { [Item::Offline], [] }
}

fn descriptor(it: Item) -> Option<&'static Descriptor> {
    for_item(it).descriptors.iter().find(|row| row.item == it)
}

pub(super) fn kind(it: Item) -> Kind {
    if let Some(row) = descriptor(it) {
        return row.kind;
    }
    if matches!(it, Item::Open(_)) {
        return Kind::Action;
    }
    (for_item(it).kind)(it).unwrap_or(Kind::Choice)
}

pub(super) fn label(it: Item) -> &'static str {
    descriptor(it).map_or_else(|| (for_item(it).label)(it).unwrap_or(""), |row| row.label)
}

pub(super) fn value(ui: &Ui, it: Item) -> String {
    descriptor(it).map_or_else(|| (for_item(it).value)(ui, it).unwrap_or_default(), |row| (row.value)(ui))
}

pub(super) fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    (for_item(it).adjust)(ui, it, d)
}

pub(super) fn activate(ui: &mut Ui, it: Item) {
    if matches!(it, Item::Open(_)) {
        navigation::open(ui, it);
    } else {
        (for_item(it).activate)(ui, it);
    }
}

pub(super) fn options(ui: &Ui, it: Item) -> Vec<String> {
    descriptor(it).map_or_else(|| (for_item(it).options)(ui, it).unwrap_or_default(), |row| (row.options)(ui))
}

pub(super) fn items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    let area = for_menu(menu);
    if area.descriptors.is_empty() {
        (area.menu_items)(ui, menu)
    } else {
        area.descriptors.iter().filter(|row| (row.visible)(ui)).map(|row| row.item).collect()
    }
}

pub(super) use navigation::main_groups;

pub(super) use decode::decode_items;

pub(super) use trigger::trigger_items;

pub(super) fn off_sources(ui: &Ui, menu: MenuId) -> Vec<usize> {
    let want = (for_menu(menu).sources)(ui, menu);
    let mut v: Vec<usize> = want
        .into_iter()
        .filter_map(|id| match id {
            WaveId::Ch(c) if c < n_ch() && !ui.m.ch[c].on => Some(c),
            _ => None,
        })
        .collect();
    v.dedup();
    v
}

#[cfg(test)]
mod tests;
