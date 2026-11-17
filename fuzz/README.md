# Parser fuzzing

Each target uses libFuzzer's panic, sanitizer and timeout detection. SCPI exercises
`Program::new` and `Scpi::exec_line_checked` with a fresh headless demo UI and
a disconnected interpreter. Commands for file access, external generators and
background sample generation are excluded from interpreter execution. HTTP parses
headers and early refusal decisions without opening a socket. Update parsing uses an empty trust set and never installs anything.
Setup checks that writing and reading the imported model produces stable setup text.

The small, synthetic seeds reproduce existing host fixtures: SCPI message tests,
store setup tests, `store::tests::wfm_f32`, `store::header_import_tests::{isf_i16,ag_f32}`,
HTTP request tests, mDNS query and compression-loop tests, and the update package
round-trip header (zeroed hashes and signature, since no key is trusted).
The three WFM versions retain their distinct packed layouts.

Only the named seeds are checked in. Preserve useful discoveries deliberately;
keep generated corpus expansions and crash artifacts out of commits.
