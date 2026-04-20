# time — Injected Bugs

Total mutations: 3

Each mutation lives as a `patches/<variant>.patch` file that reverts the
corresponding upstream fix. The `etna/<variant>` branch is the single-commit
materialization of that patch applied on top of the shared `base_commit`.

## Bug Index

| # | Name | Variant | File | Injection | Fix Commit |
|---|------|---------|------|-----------|------------|
| 1 | duration abs saturation | `duration_abs_saturation_0e99ae7_1` | `patches/duration_abs_saturation_0e99ae7_1.patch` | `patch` | `0e99ae76a8dbbbf34a4950be6e6396cc88cc6486` |
| 2 | duration checked_div | `duration_checked_div_8060100_1` | `patches/duration_checked_div_8060100_1.patch` | `patch` | `80601003b3fbc993dc23e65d5c7c476970ba9053` |
| 3 | utc_offset ordering | `utc_offset_ordering_3a60ceb_1` | `patches/utc_offset_ordering_3a60ceb_1.patch` | `patch` | `3a60ceba3f8677da34f28d56753a808812ea2a94` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `duration_abs_saturation_0e99ae7_1` | `property_duration_abs_matches_model` | `witness_duration_abs_case_min_zero_nanos`, `witness_duration_abs_case_min_negative_nanos`, `witness_duration_abs_case_negative_one_second` |
| `duration_checked_div_8060100_1` | `property_duration_checked_div_matches_model` | `witness_duration_checked_div_case_regression_one_ns`, `witness_duration_checked_div_case_regression_eight_seconds`, `witness_duration_checked_div_case_regression_negative` |
| `utc_offset_ordering_3a60ceb_1` | `property_utc_offset_ordering` | `witness_utc_offset_ordering_case_neg_pos`, `witness_utc_offset_ordering_case_negative_sixty_positive_sixty`, `witness_utc_offset_ordering_case_negative_zero`, `witness_utc_offset_ordering_case_pos_neg_seconds` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `property_duration_abs_matches_model` | ✓ | ✓ | ✓ | ✓ |
| `property_duration_checked_div_matches_model` | ✓ | ✓ | ✓ | ✓ |
| `property_utc_offset_ordering` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. duration_abs_saturation_0e99ae7_1
- **Variant**: `duration_abs_saturation_0e99ae7_1`
- **Location**: `patches/duration_abs_saturation_0e99ae7_1.patch` (rewrites `time/src/duration.rs` `Duration::abs`)
- **Property**: `property_duration_abs_matches_model`
- **Witness(es)**: `witness_duration_abs_case_min_zero_nanos`, `witness_duration_abs_case_min_negative_nanos`, `witness_duration_abs_case_negative_one_second`
- **Fix commit**: `0e99ae76a8dbbbf34a4950be6e6396cc88cc6486` — `Fix saturating behavior of Duration::abs`
- **Invariant violated**: `Duration::abs` must return a non-negative duration whose magnitude equals `self`, saturating to `Duration::MAX` when `self.seconds == i64::MIN`; in particular `abs.subsec_nanoseconds() >= 0`.
- **How the mutation triggers**: The fix replaced `self.seconds.saturating_abs()` with a `match self.seconds.checked_abs()` that saturates to `Duration::MAX`. The variant patch reverts that change, so `abs` of a `Duration::new(i64::MIN, 0)` returns `(i64::MAX, 0)` but `abs` of `Duration::new(-5, -500_000_000)` leaks the negative nanoseconds field.

### 2. duration_checked_div_8060100_1
- **Variant**: `duration_checked_div_8060100_1`
- **Location**: `patches/duration_checked_div_8060100_1.patch` (rewrites `time/src/duration.rs` `Duration::checked_div`)
- **Property**: `property_duration_checked_div_matches_model`
- **Witness(es)**: `witness_duration_checked_div_case_regression_one_ns`, `witness_duration_checked_div_case_regression_eight_seconds`, `witness_duration_checked_div_case_regression_negative`
- **Fix commit**: `80601003b3fbc993dc23e65d5c7c476970ba9053` — `Fix implementation of Duration::checked_div`
- **Invariant violated**: `Duration::checked_div(rhs)` must return a quotient `q` whose reconstruction `rhs * q` differs from `self` by at most `|rhs|` nanoseconds.
- **How the mutation triggers**: The fix rewrote the integer division so the remainder of `self.seconds % rhs` and the remainder of `self.nanoseconds % rhs` both contribute to the nanosecond field. The variant patch reverts to the old `carry * 1_000_000_000 / rhs` formula which truncates the remainder and produces a quotient that drifts by seconds rather than nanoseconds.

### 3. utc_offset_ordering_3a60ceb_1
- **Variant**: `utc_offset_ordering_3a60ceb_1`
- **Location**: `patches/utc_offset_ordering_3a60ceb_1.patch` (rewrites `time/src/utc_offset.rs` `impl Ord`, `is_positive`, `is_negative`)
- **Property**: `property_utc_offset_ordering`
- **Witness(es)**: `witness_utc_offset_ordering_case_neg_pos`, `witness_utc_offset_ordering_case_negative_sixty_positive_sixty`, `witness_utc_offset_ordering_case_negative_zero`, `witness_utc_offset_ordering_case_pos_neg_seconds`
- **Fix commit**: `3a60ceba3f8677da34f28d56753a808812ea2a94` — `Fix ordering of UtcOffset`
- **Invariant violated**: For sub-minute offsets (`|a|, |b| <= 59` so `hours == minutes == 0`), `UtcOffset::cmp` must agree with comparison of the whole-second inputs. `is_positive` / `is_negative` must agree with the sign of the whole-second value.
- **How the mutation triggers**: The fix introduced `as_i32_for_comparison` (signed packed `(h,m,s)`) and rerouted `Ord::cmp`, `is_positive`, and `is_negative` through it. The variant patch reverts `Ord::cmp` to the byte-packed `as_u32_for_equality`, whose ordering flips across zero because seconds are cast through `u8` (`-1 -> 255` compares greater than `1 -> 1`). Within the `[-59, 59]` sub-minute domain the fixed packing degenerates to raw-seconds ordering, so the packed-`u32` fault is the only observable difference.
