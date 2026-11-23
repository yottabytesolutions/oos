# SD card speed mode safeguard

Status: detection, history and warning implemented. Recovery remains open.

Startup checks the negotiated mode against the card's recorded best mode and the device-tree
ceiling. It records each start and reports the result on the System page and in `:SYSTem:STATe?`.
The panel warns when an ordinary start comes up slower than expected. See the
[device boot card mode](../boot.md#device-boot-card-mode).

The remaining work is one bounded host-controller rebind behind a setting that defaults to off.
Check that the rebind can release and restore `/oos` safely while the UI has files open. Check on
hardware whether it restores SDR104 and whether a failed attempt leaves the card usable. A card
that fails the voltage switch may require a power cycle.
