// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) use {navigation::open_menu, picker::select_direct_option};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    Text,
    Choice,
    Value,
    Action,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Row(Kind),
    Context,
    Reference,
}

/// Read-only metadata owned by an area. Visibility never changes option identity.
#[derive(Clone, Copy)]
pub(super) struct Descriptor {
    item: Item,
    role: Role,
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
        Self { item, role: Role::Row(kind), label, value, options, visible }
    }

    const fn action(item: Item, label: &'static str) -> Self {
        Self::new(item, Kind::Action, label, |_| String::new(), |_| Vec::new(), |_| true)
    }

    const fn value(item: Item, label: &'static str, value: fn(&Ui) -> String) -> Self {
        Self::new(item, Kind::Value, label, value, |_| Vec::new(), |_| true)
    }

    const fn choice(
        item: Item,
        label: &'static str,
        value: fn(&Ui) -> String,
        options: fn(&Ui) -> Vec<String>,
    ) -> Self {
        Self::new(item, Kind::Choice, label, value, options, |_| true)
    }

    const fn text(item: Item, label: &'static str, value: fn(&Ui) -> String) -> Self {
        Self::new(item, Kind::Text, label, value, |_| Vec::new(), |_| true)
    }

    const fn title(item: Item, label: &'static str) -> Self {
        Self::picker_context(item, label, |_| Vec::new())
    }

    const fn picker_context(item: Item, label: &'static str, options: fn(&Ui) -> Vec<String>) -> Self {
        Self { item, role: Role::Context, label, value: |_| String::new(), options, visible: |_| true }
    }

    // References keep cross-area metadata with its owner while contributing to this menu's order.
    const fn reference(item: Item) -> Self {
        Self { role: Role::Reference, ..Self::title(item, "") }
    }

    fn shown(&self, ui: &Ui) -> bool {
        self.role != Role::Context && (self.visible)(ui)
    }

    const fn when(mut self, visible: fn(&Ui) -> bool) -> Self {
        self.visible = visible;
        self
    }
}

fn toggles(_: &Ui) -> Vec<String> {
    ["Off", "On"].into_iter().map(String::from).collect()
}

fn onoff(on: bool) -> String {
    if on { "On" } else { "Off" }.into()
}

fn choice_label<T: PartialEq>(value: T, choices: &[(T, &str)]) -> String {
    choices.iter().find(|(v, _)| *v == value).map_or("", |(_, label)| *label).into()
}

fn choice_options<T>(choices: &[(T, &str)]) -> Vec<String> {
    choices.iter().map(|(_, label)| (*label).into()).collect()
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
    rows: fn(&Ui, MenuId) -> &'static [Descriptor],
    ordered_rows: fn(&Ui, MenuId) -> Vec<&'static Descriptor>,
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
        rows: |_, _| &[],
        ordered_rows: |_, _| Vec::new(),
    };
    fn items(&self, ui: &Ui, menu: MenuId) -> Vec<Item> {
        let ordered = (self.ordered_rows)(ui, menu);
        if !ordered.is_empty() {
            return ordered.iter().filter(|row| row.shown(ui)).map(|row| row.item).collect();
        }
        let rows = (self.rows)(ui, menu);
        let rows = if rows.is_empty() { self.descriptors } else { rows };
        if rows.is_empty() {
            (self.menu_items)(ui, menu)
        } else {
            rows.iter().filter(|row| row.shown(ui)).map(|row| row.item).collect()
        }
    }
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
    acquire { [Item::AcqMode | Item::AcqAvg | Item::AcqDepth | Item::AcqAutoDepth | Item::AcqSeg | Item::AcqCores | Item::AcqAlias | Item::AcqSegHw | Item::AcqAvgHw | Item::AcqSegIvl | Item::AcqSegTable], [MenuId::Acquire] }
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
    system { [Item::About | Item::SysInfo | Item::Shot | Item::AuxOut | Item::Stock | Item::Update | Item::UpdateRollback | Item::BootMode | Item::BootCardMode | Item::NegotiatedCardMode | Item::Overclock | Item::QuickAct], [MenuId::Utility] }
    view_more { [Item::ViewExit | Item::ViewSettings], [MenuId::ViewMore] }
    network { [Item::NetEth | Item::NetMac | Item::NetPorts | Item::NetWlan | Item::WlanInfo | Item::WlanScan | Item::WlanForget | Item::WlanNet | Item::WlanForgetNet], [MenuId::Network] }
    remote { [Item::NetWeb | Item::RemWeb | Item::RemScpi | Item::RemVxi | Item::RemHislip | Item::RemSsh | Item::RemScpiInfo | Item::RemVxiInfo | Item::RemHislipInfo | Item::RemMdns | Item::RemName | Item::RemAllow | Item::RemFirewall], [MenuId::Remote] }
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
    for_item(it).descriptors.iter().find(|row| row.item == it && row.role != Role::Reference)
}

pub(super) fn kind(it: Item) -> Kind {
    if let Some(Descriptor { role: Role::Row(kind), .. }) = descriptor(it) {
        return *kind;
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
    for_menu(menu).items(ui, menu)
}

pub(super) use navigation::main_groups;

#[cfg(test)]
pub(super) use decode::decode_items;

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

/// x centre of what opened the menu, and whether that is in the top bar (the dialog hangs below it) or a card
/// (the dialog sits above it).
pub(super) fn anchor(ui: &Ui) -> (i32, bool) {
    let (chs, tr, trg) = ui.cards();
    match ui.menu_root() {
        MenuId::Channel(c) | MenuId::ChMore(c) => (chs[c].x + chs[c].w / 2, false),
        MenuId::Horizontal => (tr.x + tr.w / 2, false),
        MenuId::Trigger => (trg.x + trg.w / 2, false),
        MenuId::Main => (ui.menu_btn().x, false),
        MenuId::Network | MenuId::Remote | MenuId::Ssh => (ui.net_box().x + ui.net_box().w / 2, true),
        MenuId::ViewMore => (ui.scr.w, true),
        MenuId::La | MenuId::LaLines | MenuId::LaPat => {
            ui.aux_cards().0.map_or((ui.scr.w / 2, false), |r| (r.x + r.w / 2, false))
        }
        MenuId::Afg => ui.aux_cards().1.map_or((ui.scr.w / 2, false), |r| (r.x + r.w / 2, false)),
        // menus of front-panel keys (also opened from the start menu): centred above the cards
        m => ui
            .top_buttons()
            .iter()
            .find(|t| t.0 == TopBtn::Menu(m))
            .map_or((ui.scr.w / 2, false), |t| (t.2.x + t.2.w / 2, true)),
    }
}

pub(super) fn is_text_row(it: Item) -> bool {
    matches!(kind(it), Kind::Text)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod small_tests;

pub(super) use serial::serial_items;

#[cfg(test)]
mod analysis_read;
#[cfg(test)]
mod read_tests;
