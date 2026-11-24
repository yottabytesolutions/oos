# Setup keys

## Auto memory

`auto_depth = Extended` selects the existing Auto record length. `auto_depth = Screen` keeps the Extended sample
rate and selects a screen plus the existing one-eighth margin when that fits in a shorter record. Trigger delay
can require additional points to keep the trigger and screen in the record. Setups without `auto_depth` use
`Extended`. The new register plans are host-tested. Capture with them has yet to be verified on hardware.

The setting is saved even when `mdepth` selects a fixed depth. Acquire shows **Auto memory** for a hardware
source when Memory is **Auto**. Roll and scan keep their streaming geometry. Changing **Auto memory** while
stopped leaves the held record available and applies the policy on the next RUN.
