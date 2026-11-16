// SPDX-License-Identifier: GPL-2.0-or-later

use oos_core::change::Identity;

/// A value published with the settings and generator under Shared's lock.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Publication {
    identity: Identity,
    route: bool,
}
impl Publication {
    pub(crate) fn new(identity: Identity, route: bool) -> Self {
        Self { identity, route }
    }
    pub(crate) fn identity(self) -> Identity {
        self.identity
    }
    pub(crate) fn route(self) -> bool {
        self.route
    }
}
