// ETNA workload runner for the `time` crate.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: DurationAbsMatchesModel | DurationCheckedDivMatchesModel
//             | UtcOffsetOrdering | All
//
// Every invocation prints exactly one JSON line to stdout and exits 0
// (except argv parsing which exits 2).

use crabcheck::quickcheck as crabcheck_qc;
use crabcheck::quickcheck::Arbitrary as CcArbitrary;
use hegel::{generators as hgen, HealthCheck, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestError, TestRunner};
use qc_etna::{Arbitrary as QcArbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use rand09::Rng;
use rand09 as rand;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use time::etna::{
    property_duration_abs_matches_model, property_duration_checked_div_matches_model,
    property_utc_offset_ordering, PropertyResult,
};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &[
    "DurationAbsMatchesModel",
    "DurationCheckedDivMatchesModel",
    "UtcOffsetOrdering",
];

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if let Err(e) = r {
            return (Err(e), total);
        }
    }
    (Ok(()), total)
}

// ---------- etna (deterministic witness replay) ----------
//
// These frozen inputs mirror the `tests/etna_witnesses.rs` cases.

fn check_duration_abs_matches_model() -> Result<(), String> {
    // i64::MIN + nonzero ns — must saturate to Duration::MAX, not leak a
    // negative subsec_nanoseconds.
    to_err(property_duration_abs_matches_model(i64::MIN, 0))?;
    to_err(property_duration_abs_matches_model(-5, 0))?;
    to_err(property_duration_abs_matches_model(-5, -500_000_000))?;
    to_err(property_duration_abs_matches_model(0, -1))?;
    to_err(property_duration_abs_matches_model(7, 123_456_789))
}

fn check_duration_checked_div_matches_model() -> Result<(), String> {
    // The 80601003 regression case, verbatim.
    to_err(property_duration_checked_div_matches_model(1, 1, 7))?;
    to_err(property_duration_checked_div_matches_model(10, 0, 2))?;
    to_err(property_duration_checked_div_matches_model(-10, 0, 2))?;
    to_err(property_duration_checked_div_matches_model(0, 1, 7))?;
    to_err(property_duration_checked_div_matches_model(2, 500_000_000, 3))
}

fn check_utc_offset_ordering() -> Result<(), String> {
    to_err(property_utc_offset_ordering(-1, 1))?;
    to_err(property_utc_offset_ordering(-30, 30))?;
    to_err(property_utc_offset_ordering(-59, 59))?;
    to_err(property_utc_offset_ordering(-1, 0))?;
    to_err(property_utc_offset_ordering(0, 1))
}

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        "DurationAbsMatchesModel" => check_duration_abs_matches_model(),
        "DurationCheckedDivMatchesModel" => check_duration_checked_div_matches_model(),
        "UtcOffsetOrdering" => check_utc_offset_ordering(),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            );
        }
    };
    (
        result,
        Metrics {
            inputs: 1,
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ---------- shared Arbitrary-biased generators (qc + cc) ----------
//
// The properties each need primitive inputs — a `(seconds, nanos)` pair, a
// `(seconds, nanos, rhs)` triple, or a `(a, b)` pair of offset totals. We use
// biased newtypes so boundary values (i64::MIN/MAX, 0, +-1_000_000_000) hit
// frequently across trial counts.

#[derive(Clone, Copy)]
struct DurSeconds(i64);

#[derive(Clone, Copy)]
struct DurNanos(i32);

#[derive(Clone, Copy)]
struct DivRhs(i32);

#[derive(Clone, Copy)]
struct OffsetSecs(i32);

impl fmt::Debug for DurSeconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for DurSeconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for DurNanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for DurNanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for DivRhs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for DivRhs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for OffsetSecs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for OffsetSecs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// A narrow "range-RNG" abstraction over both `qc_etna::Gen` and `rand::Rng` so
// the same generator body can be reused by QC and crabcheck without the type
// conflicts that arise from `Gen` not implementing `rand::Rng`.
trait RangeRng {
    fn rr_u8(&mut self, lo: u8, hi: u8) -> u8;
    fn rr_i32(&mut self, lo: i32, hi: i32) -> i32;
    fn rr_i64(&mut self, lo: i64, hi: i64) -> i64;
}

impl RangeRng for Gen {
    fn rr_u8(&mut self, lo: u8, hi: u8) -> u8 { self.random_range(lo..hi) }
    fn rr_i32(&mut self, lo: i32, hi: i32) -> i32 { self.random_range(lo..=hi) }
    fn rr_i64(&mut self, lo: i64, hi: i64) -> i64 { self.random_range(lo..=hi) }
}

struct RngWrap<'a, R: Rng>(&'a mut R);

impl<'a, R: Rng> RangeRng for RngWrap<'a, R> {
    fn rr_u8(&mut self, lo: u8, hi: u8) -> u8 { self.0.random_range(lo..hi) }
    fn rr_i32(&mut self, lo: i32, hi: i32) -> i32 { self.0.random_range(lo..=hi) }
    fn rr_i64(&mut self, lo: i64, hi: i64) -> i64 { self.0.random_range(lo..=hi) }
}

fn gen_dur_seconds<T: RangeRng>(rng: &mut T) -> i64 {
    // Weight i64::MIN, 0, +-1 heavily so the abs-saturation bug fires often.
    match rng.rr_u8(0, 8) {
        0 => i64::MIN,
        1 => 0,
        2 => -1,
        3 => 1,
        4 => rng.rr_i64(-1000, 1000),
        _ => rng.rr_i64(i64::MIN, i64::MAX),
    }
}

fn gen_dur_nanos<T: RangeRng>(rng: &mut T) -> i32 {
    match rng.rr_u8(0, 8) {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 999_999_999,
        4 => -999_999_999,
        5 => rng.rr_i32(-1000, 1000),
        _ => rng.rr_i32(-999_999_999, 999_999_999),
    }
}

fn gen_div_rhs<T: RangeRng>(rng: &mut T) -> i32 {
    // Weight the 80601003 trigger (small positive divisor with remainder).
    match rng.rr_u8(0, 6) {
        0 => 7,
        1 => 2,
        2 => -2,
        3 => i32::MIN,
        4 => rng.rr_i32(-10, 10),
        _ => rng.rr_i32(i32::MIN, i32::MAX),
    }
}

fn gen_offset_secs<T: RangeRng>(rng: &mut T) -> i32 {
    match rng.rr_u8(0, 6) {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 59,
        4 => -59,
        _ => rng.rr_i32(-59, 59),
    }
}

impl QcArbitrary for DurSeconds {
    fn arbitrary(g: &mut Gen) -> Self {
        DurSeconds(gen_dur_seconds(g))
    }
}

impl QcArbitrary for DurNanos {
    fn arbitrary(g: &mut Gen) -> Self {
        DurNanos(gen_dur_nanos(g))
    }
}

impl QcArbitrary for DivRhs {
    fn arbitrary(g: &mut Gen) -> Self {
        DivRhs(gen_div_rhs(g))
    }
}

impl QcArbitrary for OffsetSecs {
    fn arbitrary(g: &mut Gen) -> Self {
        OffsetSecs(gen_offset_secs(g))
    }
}

impl<R: Rng> CcArbitrary<R> for DurSeconds {
    fn generate(rng: &mut R, _n: usize) -> Self {
        DurSeconds(gen_dur_seconds(&mut RngWrap(rng)))
    }
}

impl<R: Rng> CcArbitrary<R> for DurNanos {
    fn generate(rng: &mut R, _n: usize) -> Self {
        DurNanos(gen_dur_nanos(&mut RngWrap(rng)))
    }
}

impl<R: Rng> CcArbitrary<R> for DivRhs {
    fn generate(rng: &mut R, _n: usize) -> Self {
        DivRhs(gen_div_rhs(&mut RngWrap(rng)))
    }
}

impl<R: Rng> CcArbitrary<R> for OffsetSecs {
    fn generate(rng: &mut R, _n: usize) -> Self {
        OffsetSecs(gen_offset_secs(&mut RngWrap(rng)))
    }
}

// ---------- proptest ----------

fn dur_seconds_strategy() -> BoxedStrategy<i64> {
    prop_oneof![
        1 => Just(i64::MIN),
        1 => Just(0i64),
        1 => Just(-1i64),
        1 => Just(1i64),
        2 => -1000i64..=1000i64,
        4 => any::<i64>(),
    ]
    .boxed()
}

fn dur_nanos_strategy() -> BoxedStrategy<i32> {
    prop_oneof![
        1 => Just(0i32),
        1 => Just(1i32),
        1 => Just(-1i32),
        1 => Just(999_999_999i32),
        1 => Just(-999_999_999i32),
        5 => -999_999_999i32..=999_999_999i32,
    ]
    .boxed()
}

fn div_rhs_strategy() -> BoxedStrategy<i32> {
    prop_oneof![
        2 => Just(7i32),
        1 => Just(2i32),
        1 => Just(-2i32),
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
        4 => any::<i32>(),
    ]
    .boxed()
}

fn offset_secs_strategy() -> BoxedStrategy<i32> {
    prop_oneof![
        1 => Just(0i32),
        1 => Just(1i32),
        1 => Just(-1i32),
        1 => Just(59i32),
        1 => Just(-59i32),
        4 => -59i32..=59i32,
    ]
    .boxed()
}

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let cfg = ProptestConfig {
        cases: 40_000_000,
        max_shrink_iters: 32,
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(cfg);
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "DurationAbsMatchesModel" => runner
            .run(&(dur_seconds_strategy(), dur_nanos_strategy()), move |(s, n)| {
                c.fetch_add(1, Ordering::Relaxed);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                    property_duration_abs_matches_model(s, n)
                ));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({} {})", s, n)))
                    }
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "DurationCheckedDivMatchesModel" => runner
            .run(
                &(
                    dur_seconds_strategy(),
                    dur_nanos_strategy(),
                    div_rhs_strategy(),
                ),
                move |(s, n, r)| {
                    c.fetch_add(1, Ordering::Relaxed);
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                        property_duration_checked_div_matches_model(s, n, r)
                    ));
                    match outcome {
                        Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                        Ok(PropertyResult::Fail(_)) | Err(_) => {
                            Err(TestCaseError::fail(format!("({} {} {})", s, n, r)))
                        }
                    }
                },
            )
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "UtcOffsetOrdering" => runner
            .run(&(offset_secs_strategy(), offset_secs_strategy()), move |(a, b)| {
                c.fetch_add(1, Ordering::Relaxed);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                    property_utc_offset_ordering(a, b)
                ));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({} {})", a, b)))
                    }
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ---------- quickcheck (forked crate with `etna` feature) ----------

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_duration_abs_matches_model(DurSeconds(s): DurSeconds, DurNanos(n): DurNanos) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_duration_abs_matches_model(s, n) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_duration_checked_div_matches_model(
    DurSeconds(s): DurSeconds,
    DurNanos(n): DurNanos,
    DivRhs(r): DivRhs,
) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_duration_checked_div_matches_model(s, n, r) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_utc_offset_ordering(OffsetSecs(a): OffsetSecs, OffsetSecs(b): OffsetSecs) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_utc_offset_ordering(a, b) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let mut qc = QuickCheck::new()
        .tests(40_000_000)
        .max_tests(80_000_000)
        .max_time(StdDuration::from_secs(86_400));
    let result = match property {
        "DurationAbsMatchesModel" => {
            qc.quicktest(qc_duration_abs_matches_model as fn(DurSeconds, DurNanos) -> TestResult)
        }
        "DurationCheckedDivMatchesModel" => qc.quicktest(
            qc_duration_checked_div_matches_model
                as fn(DurSeconds, DurNanos, DivRhs) -> TestResult,
        ),
        "UtcOffsetOrdering" => {
            qc.quicktest(qc_utc_offset_ordering as fn(OffsetSecs, OffsetSecs) -> TestResult)
        }
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("quickcheck aborted: {err:?}")),
        ResultStatus::TimedOut => Err("quickcheck timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "quickcheck gave up after {} tests",
            result.n_tests_passed
        )),
    };
    (status, Metrics { inputs, elapsed_us })
}

// ---------- crabcheck ----------

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_duration_abs_matches_model((DurSeconds(s), DurNanos(n)): (DurSeconds, DurNanos)) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_duration_abs_matches_model(s, n) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_duration_checked_div_matches_model(
    (DurSeconds(s), DurNanos(n), DivRhs(r)): (DurSeconds, DurNanos, DivRhs),
) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_duration_checked_div_matches_model(s, n, r) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_utc_offset_ordering((OffsetSecs(a), OffsetSecs(b)): (OffsetSecs, OffsetSecs)) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_utc_offset_ordering(a, b) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cc_config = crabcheck_qc::Config { tests: 40_000_000 };
    let result = match property {
        "DurationAbsMatchesModel" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_duration_abs_matches_model)
        }
        "DurationCheckedDivMatchesModel" => crabcheck_qc::quickcheck_with_config(
            cc_config,
            cc_duration_checked_div_matches_model,
        ),
        "UtcOffsetOrdering" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_utc_offset_ordering)
        }
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("crabcheck timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "crabcheck gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => {
            Err(format!("crabcheck aborted: {error}"))
        }
    };
    (status, Metrics { inputs, elapsed_us })
}

// ---------- hegel ----------

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(40_000_000)
        .suppress_health_check(HealthCheck::all())
}

fn hg_draw_dur_seconds(tc: &TestCase) -> i64 {
    let tag = tc.draw(hgen::integers::<u8>().min_value(0).max_value(7));
    match tag {
        0 => i64::MIN,
        1 => 0,
        2 => -1,
        3 => 1,
        4 => tc.draw(hgen::integers::<i64>().min_value(-1000).max_value(1000)),
        _ => tc.draw(hgen::integers::<i64>()),
    }
}

fn hg_draw_dur_nanos(tc: &TestCase) -> i32 {
    let tag = tc.draw(hgen::integers::<u8>().min_value(0).max_value(7));
    match tag {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 999_999_999,
        4 => -999_999_999,
        5 => tc.draw(hgen::integers::<i32>().min_value(-1000).max_value(1000)),
        _ => tc.draw(
            hgen::integers::<i32>()
                .min_value(-999_999_999)
                .max_value(999_999_999),
        ),
    }
}

fn hg_draw_div_rhs(tc: &TestCase) -> i32 {
    let tag = tc.draw(hgen::integers::<u8>().min_value(0).max_value(5));
    match tag {
        0 => 7,
        1 => 2,
        2 => -2,
        3 => i32::MIN,
        4 => tc.draw(hgen::integers::<i32>().min_value(-10).max_value(10)),
        _ => tc.draw(hgen::integers::<i32>()),
    }
}

fn hg_draw_offset_secs(tc: &TestCase) -> i32 {
    let tag = tc.draw(hgen::integers::<u8>().min_value(0).max_value(5));
    match tag {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 59,
        4 => -59,
        _ => tc.draw(hgen::integers::<i32>().min_value(-59).max_value(59)),
    }
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match property {
        "DurationAbsMatchesModel" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let s = hg_draw_dur_seconds(&tc);
                let n = hg_draw_dur_nanos(&tc);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                    property_duration_abs_matches_model(s, n)
                ));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("({} {})", s, n),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "DurationCheckedDivMatchesModel" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let s = hg_draw_dur_seconds(&tc);
                let n = hg_draw_dur_nanos(&tc);
                let r = hg_draw_div_rhs(&tc);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                    property_duration_checked_div_matches_model(s, n, r)
                ));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("({} {} {})", s, n, r),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "UtcOffsetOrdering" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let a = hg_draw_offset_secs(&tc);
                let b = hg_draw_offset_secs(&tc);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                    property_utc_offset_ordering(a, b)
                ));
                match outcome {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("({} {})", a, b),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{}", property),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

// ---------- dispatch ----------

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (
            Err(format!("Unknown tool: {tool}")),
            Metrics::default(),
        ),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!(
            "Properties: DurationAbsMatchesModel | DurationCheckedDivMatchesModel | UtcOffsetOrdering | All"
        );
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic with non-string payload".to_string()
            };
            emit_json(tool, property, "aborted", Metrics::default(), None, Some(&msg));
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(e) => emit_json(tool, property, "failed", metrics, Some(&e), None),
    }
}
