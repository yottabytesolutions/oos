// SPDX-License-Identifier: GPL-2.0-or-later

#[derive(Clone, Copy, Debug)]
pub(crate) enum Poll {
    Memory,
    Relayout,
    MeasHold,
    WorkspaceHold,
    Update,
    Wlan,
    Ntp,
    Ssh,
    Usb,
    ChHold,
    Counter,
    Dvm,
    Mask,
    Zone,
    LaProbe,
    Log,
    External,
}
impl Poll {
    pub(crate) const ALL: [Self; 17] = [
        Self::Memory,
        Self::Relayout,
        Self::MeasHold,
        Self::WorkspaceHold,
        Self::Update,
        Self::Wlan,
        Self::Ntp,
        Self::Ssh,
        Self::Usb,
        Self::ChHold,
        Self::Counter,
        Self::Dvm,
        Self::Mask,
        Self::Zone,
        Self::LaProbe,
        Self::Log,
        Self::External,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Relayout => "relayout",
            Self::MeasHold => "meas_hold",
            Self::WorkspaceHold => "workspace_hold",
            Self::Update => "update",
            Self::Wlan => "wlan",
            Self::Ntp => "ntp",
            Self::Ssh => "ssh",
            Self::Usb => "usb",
            Self::ChHold => "ch_hold",
            Self::Counter => "counter",
            Self::Dvm => "dvm",
            Self::Mask => "mask",
            Self::Zone => "zone",
            Self::LaProbe => "la_probe",
            Self::Log => "log",
            Self::External => "external",
        }
    }
}
