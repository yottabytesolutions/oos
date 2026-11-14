# Display stutter (2026-11-14)

Symptom on the unit (build 20261114-083030, dho804 eba0da5; `acq.pipeline = batch`, `perf.place = on`,
`acq.prearm = on`, `graphics = gpu`): the trace does not update continuously, short freezes, while fps (29.8), step
(15 ms) and the waveform rate look fine. Setups: A) CH2 ripple, AC, BW 20M, 20 mV/div, Normal, 5-50 us/div;
B) CH2 DC 200 mV/div offset -5 V, trigger rising 5.10 V, Normal, bursts every ~1.5 ms (found at 200 ns/div).

## Plan

1. Instrument pacing (`ui/src/pacing.rs`): per picture the new frames, gaps between pictures with new frames,
   frame-loop pass intervals and step times, per batch the wait and the interval; percentiles in `:SYSTem:STATe?`
   (` pacing ...`), passes over 60 ms logged to /tmp/oos-ui.log.
2. Measure on the unit: setup B, Normal and Auto, and the 1 kHz probe comp on CH1, to separate trigger-driven stalls
   from pipeline stalls.
3. Root cause with numbers, fix, measure again.

## Status

- 1: in progress.
