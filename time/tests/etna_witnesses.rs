// Deterministic ETNA witness tests for the `time` workload.
//
// Every `witness_<name>_case_<tag>` test calls the framework-neutral property
// function in `time::etna` with frozen inputs chosen so the property passes on
// the base tree and fails once the corresponding variant patch has reverted
// the upstream fix. Framework adapters in `src/bin/etna.rs` share the same
// property bodies, so these witnesses are the ground truth for detectability.

use time::etna::{
    property_duration_abs_matches_model, property_duration_checked_div_matches_model,
    property_utc_offset_ordering, PropertyResult,
};

fn expect_pass(result: PropertyResult, context: &str) {
    match result {
        PropertyResult::Pass => {}
        PropertyResult::Discard => panic!("{context}: property discarded the witness input"),
        PropertyResult::Fail(msg) => panic!("{context}: property failed: {msg}"),
    }
}

// ---------- duration_abs_saturation_0e99ae7_1 ----------
//
// `Duration::abs(Duration::new(i64::MIN, _))` must saturate to `Duration::MAX`;
// the pre-fix `saturating_abs()` path leaked a negative `subsec_nanoseconds`
// field, so `abs.subsec_nanoseconds() >= 0` fails under the buggy variant.

#[test]
fn witness_duration_abs_case_min_zero_nanos() {
    expect_pass(
        property_duration_abs_matches_model(i64::MIN, 0),
        "abs(i64::MIN s, 0 ns)",
    );
}

#[test]
fn witness_duration_abs_case_min_negative_nanos() {
    expect_pass(
        property_duration_abs_matches_model(i64::MIN, -1),
        "abs(i64::MIN s, -1 ns)",
    );
}

#[test]
fn witness_duration_abs_case_negative_one_second() {
    expect_pass(
        property_duration_abs_matches_model(-1, -1),
        "abs(-1 s, -1 ns)",
    );
}

// ---------- duration_checked_div_8060100_1 ----------
//
// `Duration::checked_div(rhs)` must return a quotient whose round-trip
// reconstruction `rhs * q` lies within `|rhs|` nanoseconds of `self`. The
// pre-fix implementation mis-carried the integer part of the division for
// `|rhs| > 1`, so the reconstruction drifts by seconds rather than nanoseconds.

#[test]
fn witness_duration_checked_div_case_regression_one_ns() {
    // The exact regression test appended to tests/duration.rs by 80601003:
    // `Duration::new(1, 1).checked_div(7)` must round to (0, 142_857_143), not
    // (0, 142_857_142) — the old truncation formula drifts by one whole rhs.
    expect_pass(
        property_duration_checked_div_matches_model(1, 1, 7),
        "checked_div(1 s 1 ns, /7)",
    );
}

#[test]
fn witness_duration_checked_div_case_regression_eight_seconds() {
    expect_pass(
        property_duration_checked_div_matches_model(8, 1, 7),
        "checked_div(8 s 1 ns, /7)",
    );
}

#[test]
fn witness_duration_checked_div_case_regression_negative() {
    expect_pass(
        property_duration_checked_div_matches_model(-1, -1, -7),
        "checked_div(-1 s -1 ns, /-7)",
    );
}

// ---------- utc_offset_ordering_3a60ceb_1 ----------
//
// `UtcOffset::cmp` must agree with the ordering of the underlying whole-second
// representation. The pre-fix implementation packed `(h, m, s)` bytes into a
// `u32`, which flips monotonicity whenever the offset crosses zero; the
// `is_positive`/`is_negative` predicates used field-by-field sign checks that
// disagreed with the total-second sign on the same mixed inputs.

#[test]
fn witness_utc_offset_ordering_case_neg_pos() {
    expect_pass(
        property_utc_offset_ordering(-1, 1),
        "cmp(from_whole_seconds(-1), from_whole_seconds(1))",
    );
}

#[test]
fn witness_utc_offset_ordering_case_negative_hour_positive_hour() {
    expect_pass(
        property_utc_offset_ordering(-3600, 3600),
        "cmp(from_whole_seconds(-3600), from_whole_seconds(3600))",
    );
}

#[test]
fn witness_utc_offset_ordering_case_negative_zero() {
    expect_pass(
        property_utc_offset_ordering(-1, 0),
        "cmp(from_whole_seconds(-1), from_whole_seconds(0))",
    );
}

#[test]
fn witness_utc_offset_ordering_case_pos_neg_seconds() {
    expect_pass(
        property_utc_offset_ordering(1, -1),
        "cmp(from_whole_seconds(1), from_whole_seconds(-1))",
    );
}
