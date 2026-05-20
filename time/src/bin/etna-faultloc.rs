use std::fmt;

use crabcheck::profiling::quickcheck;
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
        DurAbs {
            seconds: rng.random(),
            nanoseconds: rng.random_range(-999_999_999..=999_999_999),
        }
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
        UtcOff {
            a: rng.random_range(-59i32..=59),
            b: rng.random_range(-59i32..=59),
        }
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

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 3 { return; }
    let result = match (args[1].as_str(), args[2].as_str()) {
        ("crabcheck", "DurationAbsMatchesModel") => {
            quickcheck(|i: DurAbs| to_opt(property_duration_abs_matches_model(i.seconds, i.nanoseconds)))
        },
        ("crabcheck", "DurationCheckedDivMatchesModel") => {
            quickcheck(|i: DurDiv| to_opt(property_duration_checked_div_matches_model(i.seconds, i.nanoseconds, i.rhs)))
        },
        ("crabcheck", "UtcOffsetOrdering") => {
            quickcheck(|i: UtcOff| to_opt(property_utc_offset_ordering(i.a, i.b)))
        },
        (a, b) => panic!("Unknown: {a} {b}"),
    };
    println!("Result: {:?}", result);
}
