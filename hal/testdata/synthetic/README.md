# Synthetic calibration fixtures

These files contain invented test data. They were generated from formulas, never
copied from a unit. Do not install them on hardware or use them for measurements.

Regenerate the seven stored containers and the adjacent `adc_expected.txt` and
`afe_expected.txt` with:

```sh
python3 tools/generate-test-cal.py
```

`python3 tools/generate-test-cal.py --output /path/to/scratch` writes all eight
containers, including the large vertical container, to a scratch directory without
updating the stored fixtures or model expectations. The `plan-matrix` xtask step
uses this mode under the selected target directory to cover the AFE cases.

The generator uses only Python's standard library and the repository's independent
ADC and AFE reference models. It overwrites their calibration-directory input
before importing them. The large vertical container is generated in a temporary
directory for the reference model and in memory by the Rust test helper. A CRC
regression test pins its byte identity to the original Python output. It reads no
private calibration, clock, home directory or network input. Regeneration is byte-identical and is unnecessary for `cargo test`.

Every container has version zero, the fixed stamp 2000-01-01 00:00:00 and valid
header and payload CRCs. The `.hex` suffix follows the calibration format; the
contents are binary.

| File | Invented values |
| --- | --- |
| `cal_adc.hex` | Eleven modes, small signed core offsets and gains, delay codes 2 to 5, references 17 and 19, repeating two-bit fine gains |
| `cal_ddr.hex` | Six padded taps: 0x21, 0x345, 0x678, 0x789, 0x123, 0 |
| `cal_lsb.hex` | Four channels, 1 to 4 mV per offset DAC code |
| `cal_afe_zero.hex` | Trim codes cycling from 20 to 26 |
| `cal_afe_bandwidth.hex` | Trim codes cycling from 40 to 50 |
| `cal_vertical.hex` | Four groups and channels, the documented 801 scale keys, 81 gain configurations per block, gains derived from ten-row scale bands and zeros near 32000 |
| `cal_la.hex` | Pod zero codes 8500 and 8520 |
| `cal_afg.hex` | Forty single-segment records with unit slopes and small biases; flatness is unity |

Mandatory tests embed the small containers and generate vertical bytes in memory,
or read this directory explicitly, bypassing the production loader's `/oos/cal` precedence. Container round trips, model write lists,
ADC convergence, AFE scale reconstruction, complete self-calibration passes and
boot ordering all run without private files.

Private validation is a separate opt-in suite. The four original unit assertions
for DDR taps, boot ADC parameters, AFG codes and default LA calibration are
retained as ignored tests with the same OOS_CAL_DIR requirement. Each reads only
the named directory, without production loader fallbacks. Run all five checks with:

```sh
OOS_CAL_DIR=/path/to/private/cal cargo test --offline --locked \
  --manifest-path hal/Cargo.toml --lib \
  -- --ignored unit_ private_files_parse_and_rebuild_byte_for_byte
```

Select just the all-file round trip with:

```sh
OOS_CAL_DIR=/path/to/private/cal cargo test --offline --locked \
  --manifest-path hal/Cargo.toml --lib \
  -- --ignored --exact dho::calfile::tests::private_files_parse_and_rebuild_byte_for_byte
```

It requires all eight named files and fails if any is absent or invalid. Ordinary
runs report it as ignored. Never use an ignored test's absence as evidence that
private calibration passed.
