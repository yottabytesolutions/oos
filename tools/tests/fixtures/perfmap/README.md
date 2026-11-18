# STATe fixture provenance

`state.txt` is an unmodified production `:SYSTem:STATe?` reply captured on the
host on 2026-11-18 by
`state_query_real_reply_has_separate_panel_and_view_renders` in
`ui/src/ui/tests/change_picture_contracts.rs`. It uses the real SCPI handler,
demo acquisition, one panel render and two HDMI view renders, including a
start-up early return. Timing values are observations, not expected constants.

The sandbox denies TCP listeners, so capture through `tools/host_harness.py`
remains pending on a host permitting loopback sockets. This fixture proves the
handler envelope and diagnostic grammar; it makes no transport or unit claim.
Regenerate from the host test's printed reply, preserving the whole line.
