//! Fault-localization integration tests for time.

use std::fmt;

use crabcheck::quickcheck::{Arbitrary, Mutate};
use rand09::Rng;
use time::etna::{
    property_duration_abs_matches_model, property_duration_checked_div_matches_model,
    property_utc_offset_ordering, PropertyResult,
};

#[derive(Clone)]
struct DurAbs { seconds: i64, nanoseconds: i32 }
impl fmt::Debug for DurAbs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "secs={} ns={}", self.seconds, self.nanoseconds)
    }
}

#[derive(Clone)]
struct DurDiv { seconds: i64, nanoseconds: i32, rhs: i32 }
impl fmt::Debug for DurDiv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "secs={} ns={} rhs={}", self.seconds, self.nanoseconds, self.rhs)
    }
}

#[derive(Clone)]
struct UtcOff { a: i32, b: i32 }
impl fmt::Debug for UtcOff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a={} b={}", self.a, self.b)
    }
}

impl<R: Rng> Arbitrary<R> for DurAbs {
    fn generate(rng: &mut R, _n: usize) -> Self {
        DurAbs { seconds: rng.random(), nanoseconds: rng.random_range(-999_999_999..=999_999_999) }
    }
}
impl<R: Rng> Arbitrary<R> for DurDiv {
    fn generate(rng: &mut R, _n: usize) -> Self {
        DurDiv {
            seconds: rng.random(),
            nanoseconds: rng.random_range(-999_999_999..=999_999_999),
            rhs: rng.random_range(-100i32..=100),
        }
    }
}
impl<R: Rng> Arbitrary<R> for UtcOff {
    fn generate(rng: &mut R, _n: usize) -> Self {
        UtcOff { a: rng.random_range(-59i32..=59), b: rng.random_range(-59i32..=59) }
    }
}

impl<R: Rng> Mutate<R> for DurAbs {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        if rng.random_bool(0.5) {
            let bit = rng.random_range(0u32..64);
            out.seconds ^= 1i64 << bit;
        } else {
            out.nanoseconds = rng.random_range(-999_999_999..=999_999_999);
        }
        out
    }
}
impl<R: Rng> Mutate<R> for DurDiv {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        match rng.random_range(0u8..3) {
            0 => { let b = rng.random_range(0u32..64); out.seconds ^= 1i64 << b; },
            1 => out.nanoseconds = rng.random_range(-999_999_999..=999_999_999),
            _ => out.rhs = rng.random_range(-100i32..=100),
        }
        out
    }
}
impl<R: Rng> Mutate<R> for UtcOff {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        if rng.random_bool(0.5) { out.a = rng.random_range(-59i32..=59); }
        else { out.b = rng.random_range(-59i32..=59); }
        out
    }
}

fn to_opt(r: PropertyResult) -> Option<bool> {
    match r {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn property_duration_abs_matches_model_test(i: DurAbs) -> Option<bool> {
    to_opt(property_duration_abs_matches_model(i.seconds, i.nanoseconds))
}
fn property_duration_checked_div_matches_model_test(i: DurDiv) -> Option<bool> {
    to_opt(property_duration_checked_div_matches_model(i.seconds, i.nanoseconds, i.rhs))
}
fn property_utc_offset_ordering_test(i: UtcOff) -> Option<bool> {
    to_opt(property_utc_offset_ordering(i.a, i.b))
}

fn emit_locate_json(r: &crabcheck::profiling::LocateResult) {
    use crabcheck::quickcheck::ResultStatus;
    let status = match &r.run.status {
        ResultStatus::Failed { .. } => "Failed",
        ResultStatus::Finished => "Finished",
        ResultStatus::GaveUp => "GaveUp",
        ResultStatus::TimedOut => "TimedOut",
        ResultStatus::Aborted { .. } => "Aborted",
    };
    let top = if let Some(s) = r.top() {
        serde_json::json!({
            "rank": s.rank, "file": s.region.file, "function": s.region.function,
            "start_line": s.region.start_line, "end_line": s.region.end_line,
            "ochiai": s.region.suspiciousness.ochiai, "delta": s.region.delta,
            "panic_overlap": s.panic_overlap,
            "confidence": format!("{}", s.confidence),
            "confidence_rule": s.confidence_rule,
        })
    } else { serde_json::Value::Null };
    let top_5: Vec<_> = r.suspects.iter().take(5).map(|s| serde_json::json!({
        "rank": s.rank, "file": s.region.file, "function": s.region.function,
        "start_line": s.region.start_line, "end_line": s.region.end_line,
        "confidence": format!("{}", s.confidence),
        "confidence_rule": s.confidence_rule,
        "panic_overlap": s.panic_overlap,
    })).collect();
    let diags: Vec<_> = r.diagnostics.iter().map(|d| d.tag()).collect();
    let out = serde_json::json!({
        "status": status, "passed": r.run.passed, "discarded": r.run.discarded,
        "n_panics": r.n_panics, "n_suspects": r.suspects.len(),
        "top": top, "top_5": top_5, "diagnostics": diags,
    });
    println!("@@LOCATE@@ {}", out);
}

#[test]
fn locate_duration_abs_matches_model() {
    let report = crabcheck::quickcheck_with_locate!(property_duration_abs_matches_model_test, "time");
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_duration_checked_div_matches_model() {
    let report = crabcheck::quickcheck_with_locate!(property_duration_checked_div_matches_model_test, "time");
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_utc_offset_ordering() {
    let report = crabcheck::quickcheck_with_locate!(property_utc_offset_ordering_test, "time");
    eprintln!("{report}");
    emit_locate_json(&report);
}
