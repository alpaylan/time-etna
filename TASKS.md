# time — ETNA Tasks

Total tasks: 12

Each ETNA task is a (variant, framework) pair. Running one task means: build
from the `etna/<variant>` branch, then run
`cargo run --release --bin etna -- <framework> <PropertyKey>` and parse the
single JSON line printed to stdout. A successful detection is a `status:
"failed"` record with a concrete counterexample.

## Task Index

| Task | Variant | Framework | Property | Witness | Command |
|------|---------|-----------|----------|---------|---------|
| 001  | `duration_abs_saturation_0e99ae7_1` | proptest  | `property_duration_abs_matches_model` | `witness_duration_abs_case_min_zero_nanos` | `cargo run --release --bin etna -- proptest DurationAbsMatchesModel` |
| 002  | `duration_abs_saturation_0e99ae7_1` | quickcheck | `property_duration_abs_matches_model` | `witness_duration_abs_case_min_negative_nanos` | `cargo run --release --bin etna -- quickcheck DurationAbsMatchesModel` |
| 003  | `duration_abs_saturation_0e99ae7_1` | crabcheck | `property_duration_abs_matches_model` | `witness_duration_abs_case_negative_one_second` | `cargo run --release --bin etna -- crabcheck DurationAbsMatchesModel` |
| 004  | `duration_abs_saturation_0e99ae7_1` | hegel     | `property_duration_abs_matches_model` | `witness_duration_abs_case_min_zero_nanos` | `cargo run --release --bin etna -- hegel DurationAbsMatchesModel` |
| 005  | `duration_checked_div_8060100_1` | proptest  | `property_duration_checked_div_matches_model` | `witness_duration_checked_div_case_regression_one_ns` | `cargo run --release --bin etna -- proptest DurationCheckedDivMatchesModel` |
| 006  | `duration_checked_div_8060100_1` | quickcheck | `property_duration_checked_div_matches_model` | `witness_duration_checked_div_case_regression_eight_seconds` | `cargo run --release --bin etna -- quickcheck DurationCheckedDivMatchesModel` |
| 007  | `duration_checked_div_8060100_1` | crabcheck | `property_duration_checked_div_matches_model` | `witness_duration_checked_div_case_regression_negative` | `cargo run --release --bin etna -- crabcheck DurationCheckedDivMatchesModel` |
| 008  | `duration_checked_div_8060100_1` | hegel     | `property_duration_checked_div_matches_model` | `witness_duration_checked_div_case_regression_one_ns` | `cargo run --release --bin etna -- hegel DurationCheckedDivMatchesModel` |
| 009  | `utc_offset_ordering_3a60ceb_1` | proptest  | `property_utc_offset_ordering` | `witness_utc_offset_ordering_case_neg_pos` | `cargo run --release --bin etna -- proptest UtcOffsetOrdering` |
| 010  | `utc_offset_ordering_3a60ceb_1` | quickcheck | `property_utc_offset_ordering` | `witness_utc_offset_ordering_case_negative_hour_positive_hour` | `cargo run --release --bin etna -- quickcheck UtcOffsetOrdering` |
| 011  | `utc_offset_ordering_3a60ceb_1` | crabcheck | `property_utc_offset_ordering` | `witness_utc_offset_ordering_case_negative_zero` | `cargo run --release --bin etna -- crabcheck UtcOffsetOrdering` |
| 012  | `utc_offset_ordering_3a60ceb_1` | hegel     | `property_utc_offset_ordering` | `witness_utc_offset_ordering_case_utc` | `cargo run --release --bin etna -- hegel UtcOffsetOrdering` |

## Witness catalog

Each witness is a deterministic concrete test in `time/tests/etna_witnesses.rs`.
Base build: passes. Variant-active build: fails.

- `witness_duration_abs_case_min_zero_nanos` — `(i64::MIN, 0)` → `Duration::MAX`
- `witness_duration_abs_case_min_negative_nanos` — `(i64::MIN, -1)` → `Duration::MAX` (buggy variant leaks a negative subsec_nanoseconds through `saturating_abs`)
- `witness_duration_abs_case_negative_one_second` — `(-1, 0)` → `Duration::new(1, 0)`
- `witness_duration_checked_div_case_regression_one_ns` — `checked_div(Duration::new(1, 1), 7)` must reconstruct to `(0, 142_857_143)` (the 80601003 regression input; the buggy formula returns `(0, 142_857_142)` which drifts by the bound)
- `witness_duration_checked_div_case_regression_eight_seconds` — `checked_div(Duration::new(8, 1), 7)` must reconstruct within `|7|` ns
- `witness_duration_checked_div_case_regression_negative` — `checked_div(Duration::new(-1, -1), -7)` must reconstruct within `|-7|` ns (negative dividend and divisor)
- `witness_utc_offset_ordering_case_neg_pos` — `cmp(from_whole_seconds(-1), from_whole_seconds(1))` must be `Less`
- `witness_utc_offset_ordering_case_negative_hour_positive_hour` — `cmp(from_whole_seconds(-3600), from_whole_seconds(3600))` must be `Less`
- `witness_utc_offset_ordering_case_negative_zero` — `cmp(from_whole_seconds(-1), from_whole_seconds(0))` must be `Less`; `is_negative` must hold
- `witness_utc_offset_ordering_case_utc` — `cmp(UTC, UTC)` must be `Equal`; `is_positive`/`is_negative` must be `false`

## Framework dispatch

`src/bin/etna.rs` dispatches `<tool> <property>` directly into each framework
crate — no shell-out to `cargo test`. The supported `<tool>` values are `etna`,
`proptest`, `quickcheck`, `crabcheck`, `hegel`. The supported `<property>`
values are `DurationAbsMatchesModel`, `DurationCheckedDivMatchesModel`,
`UtcOffsetOrdering`, and `All`. Every invocation prints exactly one JSON line
to stdout and exits 0 (except argv parsing which exits 2).
