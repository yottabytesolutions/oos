How to return to the stock system, for one start or for good.

- In one-shot mode: power-cycle. System > Back to stock says the same.
- In persist mode: System > Boot mode > One-shot, then power-cycle. Or, with the card in the reader:
  `sudo python3 boot/switch.py stock /dev/rdiskM --execute`
- For good: put your original card back. After an install without opening the scope, see
  [After the install](Install-without-opening-the-scope#after-the-install).

OOS never writes the FPGA flash, `/rigol/data`, or the `uboot`, `trust` and `parameter` partitions.
