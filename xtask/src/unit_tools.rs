// SPDX-License-Identifier: GPL-2.0-or-later

mod flags;
pub(super) mod report;
pub(super) mod scpi;

pub(super) use flags::Flags;

pub(super) fn with_restore<S, T>(
    scope: &mut S,
    restore: impl FnMut(&mut S) -> Result<(), String>,
    work: impl FnOnce(&mut S) -> Result<T, String>,
) -> Result<T, String> {
    let mut restoration = Ok(());
    let result = {
        let guard = Restore { scope, restore, outcome: &mut restoration };
        work(guard.scope)
    };
    match (result, restoration) {
        (Err(error), Err(restore)) => Err(format!("{error}; {restore}")),
        (Ok(_), Err(restore)) => Err(restore),
        (result, Ok(())) => result,
    }
}

struct Restore<'a, S, F: FnMut(&mut S) -> Result<(), String>> {
    scope: &'a mut S,
    restore: F,
    outcome: &'a mut Result<(), String>,
}

impl<S, F: FnMut(&mut S) -> Result<(), String>> Drop for Restore<'_, S, F> {
    fn drop(&mut self) {
        *self.outcome = (self.restore)(self.scope);
    }
}
