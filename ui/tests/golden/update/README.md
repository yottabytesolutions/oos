# Update panel references

These PNG-encoded `.png.ref` fixtures belong to the focused Rust unit test
`panel_trial_guidance_matches_golden_pictures`, which compares exact encoded
output in debug and release. They cover pending, running and kexec trials.
The `.ref` suffix separates these direct overlay captures from the scripted
M1 PNG inventory. The test writes actual PNGs under the selected target's
`update-golden/` directory and never updates references.

The panel shows the trial state and automatic confirmation timing.
Pending and kexec trials explicitly request the required power cycle.

`slow-card.png.ref` covers the slow-card notice's Open System shortcut alongside its power
and deferral controls. `slow_card_warning_matches_the_panel_reference` checks the overlay.
