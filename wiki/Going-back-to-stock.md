Return to the stock system from this fork or upstream OOS.

## Start stock at the next power-on

1. In OOS, select **System > Boot mode > One-shot** or run `oos-update mode oneshot`.
   If OOS is already in one-shot mode after a normal start, stock is selected for the next power-on.
2. Power-cycle the scope. The unit session exercised this return from this fork only.

**Persist** makes the running OOS version the default. Select it under **System > Boot mode** or run
`oos-update mode persist`. After each confirmed start OOS selects itself for the next power-on.
An unconfirmed ordinary start leaves stock selected.

## Return to stock from the card

With the card in a reader, select stock with `sudo python3 boot/switch.py stock /dev/rdiskM --execute`.
To return to stock for good, put your original card back. After an install without opening the scope, see
[After the install](Install-without-opening-the-scope#after-the-install).

This fork never writes the FPGA flash, `/rigol/data`, or the `uboot`, `trust` and `parameter` partitions.

To move from one OOS version to the other, see [[Switching between OOS versions]].
