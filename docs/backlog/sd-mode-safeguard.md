# SD card speed mode safeguard

Status: backlog. Owner request 2026-11-22.

## Problem

The card that holds `/oos` normally runs in UHS SDR104. Some boots come up in SD high-speed mode instead. Seen
again on 2026-11-22 after an update install that starts the new build by kexec (`timing spec: 2 (sd
high-speed)` in the mmc `ios` debug file, against `6 (sd uhs SDR104)` on the other boots of that day). Nothing
tells the user. Storage is about three times slower in that mode, and performance numbers taken on such a boot
are not representative.

## Wanted behaviour

1. Detect at start-up, and after every resume of the card, that the card is not in the expected mode.
2. Try once to bring it back to SDR104 without a reboot.
3. When that fails, show a dialog that names the condition and offers a reboot. Never reboot unasked.
4. Report the mode in `:SYSTem:STATe?` and on the System page, so remote tools can refuse to measure.

## Known facts

- The mode is readable from the kernel's mmc `ios` debug file.
- A card that failed the voltage switch (CMD11) recovers only by removing power. A warm reset then gives a black
  screen. A reboot offer must therefore say that a power cycle may be needed, and the recovery attempt must not
  make things worse than the high-speed mode it started from.
- The update flow waits for the loader re-arm before a reboot is safe. The dialog has to respect that wait.

## Open questions, to answer before design

- The cause of the fallback was never found in earlier attempts. Owner decision 2026-11-22: do not hunt the
  root cause for this item. Detect, try to repair, otherwise ask.
- Whether a re-initialisation is possible while `/oos` is mounted: unbind and bind of the host controller, or a
  forced card rescan, needs `/oos` unmounted or remounted, with open files in the UI.
- Whether the expected mode is a property of the card and board (some cards only do SDR50) and has to be
  learned once and stored, instead of assuming SDR104.

## Shape

One small unit that owns the card mode: read, expected value, one bounded recovery attempt, result as a typed
state. The UI and the remote interface only read that state. No polling loop: act on boot and on the kernel's
card events.

## Tests

- State machine on recorded `ios` texts: expected, degraded, recovered, recovery failed.
- Dialog appears once per boot, not on every check.
- On the unit: force a high-speed boot (the kexec install path does it sometimes; find a way to do it on
  purpose), verify detection, the recovery attempt and the dialog.
