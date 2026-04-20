//! ETNA framework-neutral property functions for the `time` crate.
//!
//! Each `property_<name>` is a pure function taking concrete owned inputs and
//! returning `PropertyResult`. Framework adapters (proptest / quickcheck /
//! crabcheck / hegel) in `src/bin/etna.rs` and deterministic witness tests in
//! `tests/etna_witnesses.rs` all call these functions directly; the invariant
//! is never re-implemented inside an adapter.
//!
//! The properties exercise three unrelated surfaces:
//!
//!   * `Duration::abs` — must yield a non-negative duration whose magnitude
//!     equals `self`, saturating at `Duration::MAX` when `self.seconds ==
//!     i64::MIN`.
//!   * `Duration::checked_div(rhs)` — scalar division must reconstruct
//!     `self` up to a bounded remainder.
//!   * `UtcOffset` ordering — `PartialOrd`/`Ord` and `is_positive`/
//!     `is_negative` must agree with ordering on the underlying total-second
//!     representation.

#![allow(missing_docs)]

use alloc::format;
use alloc::string::String;

use crate::{Duration, UtcOffset};

pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

/// `Duration::abs` returns the non-negative duration whose magnitude equals
/// `self`. When `self.seconds == i64::MIN` the result saturates to
/// `Duration::MAX`. In particular the `subsec_nanoseconds` component of the
/// result must itself be non-negative — a pre-`0e99ae76` bug let the old
/// `saturating_abs()` leak a negative `nanoseconds` field through.
///
/// Detects `duration_abs_saturation_0e99ae7_1`.
pub fn property_duration_abs_matches_model(seconds: i64, nanoseconds: i32) -> PropertyResult {
    if !(-999_999_999..=999_999_999).contains(&nanoseconds) {
        return PropertyResult::Discard;
    }
    if (seconds > 0 && nanoseconds < 0) || (seconds < 0 && nanoseconds > 0) {
        return PropertyResult::Discard;
    }

    let d = Duration::new(seconds, nanoseconds);
    let abs = d.abs();

    if abs.whole_seconds() < 0 {
        return PropertyResult::Fail(format!(
            "abs({seconds}s {nanoseconds}ns).whole_seconds() = {} (< 0)",
            abs.whole_seconds()
        ));
    }
    if abs.subsec_nanoseconds() < 0 {
        return PropertyResult::Fail(format!(
            "abs({seconds}s {nanoseconds}ns).subsec_nanoseconds() = {} (< 0)",
            abs.subsec_nanoseconds()
        ));
    }

    if seconds == i64::MIN {
        if abs != Duration::MAX {
            return PropertyResult::Fail(format!(
                "abs(i64::MIN secs, {nanoseconds}ns) = {abs:?}, expected Duration::MAX"
            ));
        }
        return PropertyResult::Pass;
    }

    let expected = if seconds < 0 || (seconds == 0 && nanoseconds < 0) {
        Duration::new(-seconds, -nanoseconds)
    } else {
        d
    };
    if abs != expected {
        return PropertyResult::Fail(format!(
            "abs({seconds}s {nanoseconds}ns) = {abs:?}, expected {expected:?}"
        ));
    }
    PropertyResult::Pass
}

/// `Duration::checked_div(rhs)` must return a quotient that, when multiplied
/// back out, matches `self` to within a strict remainder bound. Concretely:
///
///   `rhs * q` differs from `self` by at most `|rhs| - 1` nanoseconds, where
///   `q = self.checked_div(rhs).unwrap()`.
///
/// Pre-`80601003` the implementation produced a nanoseconds field that could
/// round off by up to `rhs` seconds because it accumulated the integer carry
/// incorrectly, so reconstructing the dividend failed wildly.
///
/// Detects `duration_checked_div_8060100_1`.
pub fn property_duration_checked_div_matches_model(
    seconds: i64,
    nanoseconds: i32,
    rhs: i32,
) -> PropertyResult {
    if rhs == 0 {
        return PropertyResult::Discard;
    }
    if !(-999_999_999..=999_999_999).contains(&nanoseconds) {
        return PropertyResult::Discard;
    }
    if (seconds > 0 && nanoseconds < 0) || (seconds < 0 && nanoseconds > 0) {
        return PropertyResult::Discard;
    }
    // Avoid degenerate `i64::MIN.checked_div(-1)` case where `Option::None` is
    // the documented answer and the exact reconstruction isn't meaningful.
    if seconds == i64::MIN && rhs == -1 {
        return PropertyResult::Discard;
    }

    let d = Duration::new(seconds, nanoseconds);
    let Some(q) = d.checked_div(rhs) else {
        // A `None` result is the implementation's signalled failure — there is
        // no ground truth to compare against, so discard the input rather than
        // enforce a specific failure reason.
        return PropertyResult::Discard;
    };

    // Reconstruct `rhs * q` as an i128 of nanoseconds and compare to `self`
    // as the same. The quotient must satisfy |self - rhs*q| < |rhs| ns — the
    // remainder of integer-dividing `self.whole_nanoseconds()` by `rhs`.
    let self_ns: i128 = d.whole_nanoseconds();
    let q_ns: i128 = q.whole_nanoseconds();
    let rhs128 = rhs as i128;
    let recon = match q_ns.checked_mul(rhs128) {
        Some(v) => v,
        None => return PropertyResult::Discard,
    };
    let diff = self_ns - recon;
    let bound = rhs128.unsigned_abs();
    if diff.unsigned_abs() >= bound {
        return PropertyResult::Fail(format!(
            "checked_div({seconds}s {nanoseconds}ns, {rhs}) = {q:?}: |self - rhs*q| = {} ns, bound = {} ns",
            diff.unsigned_abs(),
            bound
        ));
    }
    PropertyResult::Pass
}

/// `UtcOffset::cmp` must order offsets by their total-second representation:
///
///   `UtcOffset::from_whole_seconds(a).cmp(&from_whole_seconds(b))` == `a.cmp(&b)`
///
/// Similarly, `is_positive` / `is_negative` / `is_utc` must agree with the
/// sign / zero-ness of the total-second value.
///
/// Pre-`3a60ceba` the implementation packed `(hours, minutes, seconds)` bytes
/// into a `u32` for ordering, which breaks monotonicity whenever the offset
/// crosses zero (because `seconds.cast_unsigned()` flips ordering on the sign
/// bit). `is_positive` / `is_negative` went through a field-by-field check
/// that returned true whenever *any* component was positive/negative, so
/// mixed-sign offsets (impossible to construct via `from_whole_seconds`, but
/// cheap to exercise here) still exposed the bug via the exclusively-ordering
/// branch.
///
/// Detects `utc_offset_ordering_3a60ceb_1`.
pub fn property_utc_offset_ordering(a: i32, b: i32) -> PropertyResult {
    // UtcOffset is constrained to `-89_999..=89_999` whole seconds (25h bound
    // minus one second). Restrict the inputs to this domain.
    const MAX: i32 = 89_999;
    if !(-MAX..=MAX).contains(&a) || !(-MAX..=MAX).contains(&b) {
        return PropertyResult::Discard;
    }

    let oa = match UtcOffset::from_whole_seconds(a) {
        Ok(o) => o,
        Err(_) => return PropertyResult::Discard,
    };
    let ob = match UtcOffset::from_whole_seconds(b) {
        Ok(o) => o,
        Err(_) => return PropertyResult::Discard,
    };

    if oa.cmp(&ob) != a.cmp(&b) {
        return PropertyResult::Fail(format!(
            "from_whole_seconds({a}).cmp(&from_whole_seconds({b})) = {:?}, expected {:?}",
            oa.cmp(&ob),
            a.cmp(&b)
        ));
    }

    if oa.is_positive() != (a > 0) {
        return PropertyResult::Fail(format!(
            "from_whole_seconds({a}).is_positive() = {}, expected {}",
            oa.is_positive(),
            a > 0
        ));
    }
    if oa.is_negative() != (a < 0) {
        return PropertyResult::Fail(format!(
            "from_whole_seconds({a}).is_negative() = {}, expected {}",
            oa.is_negative(),
            a < 0
        ));
    }
    if oa.is_utc() != (a == 0) {
        return PropertyResult::Fail(format!(
            "from_whole_seconds({a}).is_utc() = {}, expected {}",
            oa.is_utc(),
            a == 0
        ));
    }

    PropertyResult::Pass
}
