Short answers, each with the page that has the details.

## Does installing OOS void the warranty?

Installing with a second card means opening the scope, and that voids the warranty. The USB install does not open the
scope, but it changes the card, and nothing here promises that your warranty survives that. See the [[Disclaimer]].

## Which models does OOS run on?

A DHO924S (the developer's unit) and, by a user report, a DHO804. Models of the same hardware variant differ only in
their licensed bandwidth and count as verified. The other DHO800/DHO900 models are prepared but have never run OOS.
See [[Supported models and features]].

## Can I go back to the stock system?

Yes. OOS runs next to the stock system on the same card. In one-shot mode, the next power-on starts stock. See
[[Going back to stock]].

## Does OOS change my calibration?

OOS never writes the FPGA flash, `/rigol/data`, or the `uboot`, `trust` and `parameter` partitions. Self-calibration
writes its results to `/oos/cal`; the factory files are never changed. A mistake while writing the card can still
destroy the calibration stored on it, so back up the whole card first
([backup](Install-on-a-second-card#back-up-the-original-card)).

## Will my option licences keep working?

Option licences of the stock system have been seen to stop working after running OOS, and had to be entered again.
Keep your licence keys at hand.

## Are measurements with OOS accurate?

OOS is not a calibrated or certified measuring instrument. Readings can be wrong without any visible sign. Do not use
it for safety-relevant, medical, legal, commercial or compliance measurements.

## Is it safe to put the scope on my network?

Only on a trusted bench network, never directly on the internet. SSH is the only remote shell. Apart from SSH, OOS has no authentication:
SCPI, VXI-11, the web remote and MCP are open to everyone who can reach the scope. Updates require a signature
from this project's release key.

## Why did the stock system start instead of OOS?

By default OOS is one-shot: every start of OOS restores the stock boot. Set System > Boot mode to Persist to keep OOS.
See [[First start and choosing how OOS starts]] and [[Troubleshooting]].

## How do I update OOS?

Open `http://<scope>:8080/update` and choose the `.oosupd` file of a release. See
[Updates](Daily-use#updates).

## Why does stock say “No firmware available”?

The start menu's **Upgrade** icon opens the online updater. Use **Storage > Upgrade** for a USB GEL.
See [the stock install route](Troubleshooting#no-firmware-available).

## Why does stock ignore my USB stick or keep stopping?

Use a [FAT32 stick](Troubleshooting#the-stock-system-does-not-see-the-stick). If the modified stock app keeps
stopping, unplug the stick and [insert it after the app starts](Troubleshooting#the-stock-scope-app-keeps-stopping).

## Why does OOS say “SD card is in slow mode”?

The card negotiated a slower mode than expected. A warm restart after a GEL or bridge install can leave it in
High Speed (HS). See [SD card is in slow mode](Troubleshooting#sd-card-is-in-slow-mode).

## What do the data area messages mean?

[The data area was repaired](Troubleshooting#the-data-area-was-repaired) means `/oos` mounted after a checked
repair. Check your recent changes. [The data area is unavailable](Troubleshooting#the-data-area-is-unavailable)
means `/oos` could not be prepared or mounted. Save `/tmp/oos-data.log` and contact support before changing the card.

## What if the power went off during an update?

Check the `last event` in `oos-update status`. Two consecutive missed SDR104 starts make the next install select High Speed.
See [what status says after a power cut and how to retry SDR104](Troubleshooting#an-update-did-not-finish-after-a-power-cut).

## How do I tell what is running and what will start next?

In `oos-update status`, `running` shows the current build and `boot magic` shows the next power-on selection.
See [Read the update status](Troubleshooting#read-the-update-status) for the recovery image, rollback, tracking
and trial fields.

## Can I try OOS without a scope?

Yes, with simulated signals. See [[Try it without a scope]].

## Where do I ask questions or report a problem?

[SUPPORT.md](https://github.com/yottabytesolutions/oos/blob/main/SUPPORT.md) says where: the repository's
[discussions](https://github.com/yottabytesolutions/oos/discussions) for questions and
[issues](https://github.com/yottabytesolutions/oos/issues) for bugs. Security problems go through private
vulnerability reporting ([SECURITY.md](https://github.com/yottabytesolutions/oos/blob/main/SECURITY.md)).
