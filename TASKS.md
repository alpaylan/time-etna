# time — ETNA Tasks

Total tasks: 12

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `duration_abs_saturation_0e99ae7_1` | proptest | `DurationAbsMatchesModel` | `witness_duration_abs_case_min_zero_nanos` |
| 002 | `duration_abs_saturation_0e99ae7_1` | quickcheck | `DurationAbsMatchesModel` | `witness_duration_abs_case_min_zero_nanos` |
| 003 | `duration_abs_saturation_0e99ae7_1` | crabcheck | `DurationAbsMatchesModel` | `witness_duration_abs_case_min_zero_nanos` |
| 004 | `duration_abs_saturation_0e99ae7_1` | hegel | `DurationAbsMatchesModel` | `witness_duration_abs_case_min_zero_nanos` |
| 005 | `duration_checked_div_8060100_1` | proptest | `DurationCheckedDivMatchesModel` | `witness_duration_checked_div_case_regression_one_ns` |
| 006 | `duration_checked_div_8060100_1` | quickcheck | `DurationCheckedDivMatchesModel` | `witness_duration_checked_div_case_regression_one_ns` |
| 007 | `duration_checked_div_8060100_1` | crabcheck | `DurationCheckedDivMatchesModel` | `witness_duration_checked_div_case_regression_one_ns` |
| 008 | `duration_checked_div_8060100_1` | hegel | `DurationCheckedDivMatchesModel` | `witness_duration_checked_div_case_regression_one_ns` |
| 009 | `utc_offset_ordering_3a60ceb_1` | proptest | `UtcOffsetOrdering` | `witness_utc_offset_ordering_case_neg_pos` |
| 010 | `utc_offset_ordering_3a60ceb_1` | quickcheck | `UtcOffsetOrdering` | `witness_utc_offset_ordering_case_neg_pos` |
| 011 | `utc_offset_ordering_3a60ceb_1` | crabcheck | `UtcOffsetOrdering` | `witness_utc_offset_ordering_case_neg_pos` |
| 012 | `utc_offset_ordering_3a60ceb_1` | hegel | `UtcOffsetOrdering` | `witness_utc_offset_ordering_case_neg_pos` |

## Witness Catalog

- `witness_duration_abs_case_min_zero_nanos` — base passes, variant fails
- `witness_duration_abs_case_min_negative_nanos` — base passes, variant fails
- `witness_duration_abs_case_negative_one_second` — base passes, variant fails
- `witness_duration_checked_div_case_regression_one_ns` — base passes, variant fails
- `witness_duration_checked_div_case_regression_eight_seconds` — base passes, variant fails
- `witness_duration_checked_div_case_regression_negative` — base passes, variant fails
- `witness_utc_offset_ordering_case_neg_pos` — base passes, variant fails
- `witness_utc_offset_ordering_case_negative_sixty_positive_sixty` — base passes, variant fails
- `witness_utc_offset_ordering_case_negative_zero` — base passes, variant fails
- `witness_utc_offset_ordering_case_pos_neg_seconds` — base passes, variant fails
