# time — Injected Bugs

Total mutations: 3

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `duration_abs_saturation_0e99ae7_1` | `duration_abs_saturation` | `time/src/duration.rs` | `patch` | `0e99ae76a8dbbbf34a4950be6e6396cc88cc6486` |
| 2 | `duration_checked_div_8060100_1` | `duration_checked_div` | `time/src/duration.rs` | `patch` | `80601003b3fbc993dc23e65d5c7c476970ba9053` |
| 3 | `utc_offset_ordering_3a60ceb_1` | `utc_offset_ordering` | `time/src/utc_offset.rs` | `patch` | `3a60ceba3f8677da34f28d56753a808812ea2a94` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `duration_abs_saturation_0e99ae7_1` | `DurationAbsMatchesModel` | `witness_duration_abs_case_min_zero_nanos`, `witness_duration_abs_case_min_negative_nanos`, `witness_duration_abs_case_negative_one_second` |
| `duration_checked_div_8060100_1` | `DurationCheckedDivMatchesModel` | `witness_duration_checked_div_case_regression_one_ns`, `witness_duration_checked_div_case_regression_eight_seconds`, `witness_duration_checked_div_case_regression_negative` |
| `utc_offset_ordering_3a60ceb_1` | `UtcOffsetOrdering` | `witness_utc_offset_ordering_case_neg_pos`, `witness_utc_offset_ordering_case_negative_sixty_positive_sixty`, `witness_utc_offset_ordering_case_negative_zero`, `witness_utc_offset_ordering_case_pos_neg_seconds` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `DurationAbsMatchesModel` | ✓ | ✓ | ✓ | ✓ |
| `DurationCheckedDivMatchesModel` | ✓ | ✓ | ✓ | ✓ |
| `UtcOffsetOrdering` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. duration_abs_saturation

- **Variant**: `duration_abs_saturation_0e99ae7_1`
- **Location**: `time/src/duration.rs`
- **Property**: `DurationAbsMatchesModel`
- **Witness(es)**:
  - `witness_duration_abs_case_min_zero_nanos`
  - `witness_duration_abs_case_min_negative_nanos`
  - `witness_duration_abs_case_negative_one_second`
- **Source**: Fix saturating behavior of Duration::abs
  > `Duration::abs` delegated to `i64::saturating_abs` on the seconds field and ignored the nanoseconds, so `Duration::new(-5, -500_000_000).abs()` leaked a negative sub-second component; the fix handles the sign on both fields and saturates the `i64::MIN` case to `Duration::MAX`.
- **Fix commit**: `0e99ae76a8dbbbf34a4950be6e6396cc88cc6486` — Fix saturating behavior of Duration::abs
- **Invariant violated**: `Duration::abs` must return a non-negative duration whose magnitude equals `self`, saturating to `Duration::MAX` when `self.seconds == i64::MIN`; in particular `abs.subsec_nanoseconds() >= 0`.
- **How the mutation triggers**: The fix replaced `self.seconds.saturating_abs()` with a `match self.seconds.checked_abs()` that saturates to `Duration::MAX`. The variant patch reverts that change, so `abs` of a `Duration::new(i64::MIN, 0)` returns `(i64::MAX, 0)` but `abs` of `Duration::new(-5, -500_000_000)` leaks the negative nanoseconds field.

### 2. duration_checked_div

- **Variant**: `duration_checked_div_8060100_1`
- **Location**: `time/src/duration.rs`
- **Property**: `DurationCheckedDivMatchesModel`
- **Witness(es)**:
  - `witness_duration_checked_div_case_regression_one_ns`
  - `witness_duration_checked_div_case_regression_eight_seconds`
  - `witness_duration_checked_div_case_regression_negative`
- **Source**: Fix implementation of Duration::checked_div
  > `Duration::checked_div` used `carry * 1_000_000_000 / rhs`, which truncated the remainder of both the seconds and the nanoseconds field; the fix reconstructs the quotient from both remainders so the answer drifts by at most one nanosecond instead of seconds.
- **Fix commit**: `80601003b3fbc993dc23e65d5c7c476970ba9053` — Fix implementation of Duration::checked_div
- **Invariant violated**: `Duration::checked_div(rhs)` must return a quotient `q` whose reconstruction `rhs * q` differs from `self` by at most `|rhs|` nanoseconds.
- **How the mutation triggers**: The fix rewrote the integer division so the remainder of `self.seconds % rhs` and the remainder of `self.nanoseconds % rhs` both contribute to the nanosecond field. The variant patch reverts to the old `carry * 1_000_000_000 / rhs` formula which truncates the remainder and produces a quotient that drifts by seconds rather than nanoseconds.

### 3. utc_offset_ordering

- **Variant**: `utc_offset_ordering_3a60ceb_1`
- **Location**: `time/src/utc_offset.rs`
- **Property**: `UtcOffsetOrdering`
- **Witness(es)**:
  - `witness_utc_offset_ordering_case_neg_pos`
  - `witness_utc_offset_ordering_case_negative_sixty_positive_sixty`
  - `witness_utc_offset_ordering_case_negative_zero`
  - `witness_utc_offset_ordering_case_pos_neg_seconds`
- **Source**: Fix ordering of UtcOffset
  > `UtcOffset::cmp` reused the byte-packed `as_u32_for_equality` representation in which seconds are cast through `u8`, so `-1s` packed as `255` compared greater than `+1s` packed as `1`; the fix routes ordering through a signed packing that preserves the sign.
- **Fix commit**: `3a60ceba3f8677da34f28d56753a808812ea2a94` — Fix ordering of UtcOffset
- **Invariant violated**: For sub-minute offsets (`|a|, |b| <= 59` so `hours == minutes == 0`), `UtcOffset::cmp` must agree with comparison of the whole-second inputs. `is_positive` / `is_negative` must agree with the sign of the whole-second value.
- **How the mutation triggers**: The fix introduced `as_i32_for_comparison` (signed packed `(h,m,s)`) and rerouted `Ord::cmp`, `is_positive`, and `is_negative` through it. The variant patch reverts `Ord::cmp` to the byte-packed `as_u32_for_equality`, whose ordering flips across zero because seconds are cast through `u8` (`-1 -> 255` compares greater than `1 -> 1`). Within the `[-59, 59]` sub-minute domain the fixed packing degenerates to raw-seconds ordering, so the packed-`u32` fault is the only observable difference.
