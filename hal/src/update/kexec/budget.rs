// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Growth ceiling for the measured kexec watchdog gap; timing still needs unit acceptance (docs/update.md).

const MAX_PAYLOAD: usize = 85 << 20;

pub(super) fn check(kernel: usize, initrd: usize, tree: usize) -> Result<(), String> {
    let bytes = kernel.checked_add(initrd).and_then(|n| n.checked_add(tree));
    if bytes.is_none_or(|n| n > MAX_PAYLOAD) {
        return Err(concat!(
            "kexec payload exceeds 85 MiB watchdog budget; remeasure the gap before raising it. ",
            "--no-kexec skips the trial start: a hanging kernel restarts at every power-on until the card is pulled"
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_payload_fits_but_unbounded_growth_does_not() {
        assert!(check(27_159_040, 57_611_256, 94_618).is_ok());
        assert!(check(27_159_040, MAX_PAYLOAD - 27_159_040 - 94_618, 94_618).is_ok());
        assert!(check(27_159_040, MAX_PAYLOAD - 27_159_040 - 94_618 + 1, 94_618).is_err());
        assert!(check(usize::MAX, 1, 0).is_err());
        assert!(check(1, usize::MAX - 1, 1).is_err());
    }
}
