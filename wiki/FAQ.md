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

## Can I try OOS without a scope?

Yes, with simulated signals. See [[Try it without a scope]].

## Where do I ask questions or report a problem?

[SUPPORT.md](https://github.com/yottabytesolutions/oos/blob/main/SUPPORT.md) says where: the repository's
[discussions](https://github.com/yottabytesolutions/oos/discussions) for questions and
[issues](https://github.com/yottabytesolutions/oos/issues) for bugs. Security problems go through private
vulnerability reporting ([SECURITY.md](https://github.com/yottabytesolutions/oos/blob/main/SECURITY.md)).
