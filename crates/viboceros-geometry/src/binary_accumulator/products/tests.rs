use super::*;
use crate::binary_accumulator::{add_product, finish_at};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};

fn reference<const N: usize>(positive: [u64; N], negative: [u64; N], origin: i32) -> f64 {
    let integer = positive
        .into_iter()
        .zip(negative)
        .enumerate()
        .fold(BigInt::zero(), |sum, (i, (p, n))| {
            sum + ((BigInt::from(p) - BigInt::from(n)) << (i * 64))
        });
    let rational = if origin < 0 {
        BigRational::new(integer, BigInt::one() << (-origin) as usize)
    } else {
        BigRational::from_integer(integer << origin as usize)
    };
    rational.to_f64().unwrap()
}

#[test]
fn compact_product_window_reserves_all_carries_and_ignores_zero_terms() {
    let mut left = [1., 2_f64.powi(87), 0., 0., 0., 0.];
    let right = [1.; 6];
    assert_eq!(compact_base(&left, &right, 0, 106), Some(1984));
    left[1] = 2_f64.powi(88);
    assert_eq!(compact_base(&left, &right, 0, 106), None);
    left[1] = 2_f64.powi(36);
    assert_eq!(compact_base(&left, &right, 1022, 159), Some(3008));
    left[1] = 2_f64.powi(37);
    assert_eq!(compact_base(&left, &right, 1022, 159), None);
    for exponent in [-1022, -512, -1, 0, 512, 1000] {
        let values = [2_f64.powi(exponent); 6];
        assert!(compact_base(&values, &values, 0, 106).is_some());
        assert!(compact_base(&values, &values, 2045, 159).is_some());
    }
    assert_eq!(
        compact_base(&[f64::MAX, f64::from_bits(1)], &[1.; 2], 0, 106),
        None
    );
    assert_eq!(compact_base(&[], &[], 0, 106), Some(0));
    assert_eq!(compact_base(&[-0.; 6], &[f64::MAX; 6], 0, 106), Some(0));
}

fn check_rationals<const WORDS: usize>(origins: &[i32]) {
    let mut state = 0x8172_9182_aabb_ccdd_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state
    };
    for &origin in origins {
        for case in 0..128 {
            let mut positive = std::array::from_fn::<_, WORDS, _>(|_| next());
            let mut negative = std::array::from_fn::<_, WORDS, _>(|_| next());
            if case % 3 == 0 {
                positive[WORDS / 2..].fill(0);
                negative[WORDS / 2..].fill(0);
            }
            if case % 5 == 0 {
                negative = positive;
                negative[0] ^= 1;
            }
            let expected = reference(positive, negative, origin);
            assert_eq!(
                finish_at(positive, negative, origin).to_bits(),
                expected.to_bits(),
                "words={WORDS}, origin={origin}, case={case}"
            );
        }
    }
}

#[test]
fn compact_and_full_accumulators_match_independent_big_rationals() {
    check_rationals::<4>(&[
        -3222, -2148, -1400, -1330, -1329, -1280, -1074, -512, 0, 768, 769, 1024,
    ]);
    check_rationals::<66>(&[-2148, -2149]);
    check_rationals::<99>(&[-3222]);
}

#[test]
fn dynamic_accumulator_origins_cover_zero_subnormals_normals_and_overflow() {
    for origin in [
        -2200, -1076, -1075, -1074, -1022, -1, 0, 53, 1023, 1024, 2200,
    ] {
        let expected = reference([1, 0, 0, 0], [0; 4], origin);
        assert_eq!(
            finish_at([1, 0, 0, 0], [0; 4], origin).to_bits(),
            expected.to_bits()
        );
        assert_eq!(
            finish_at([0; 4], [1, 0, 0, 0], origin).to_bits(),
            (-expected).to_bits()
        );
        assert_eq!(finish_at([1, 0, 0, 0], [1, 0, 0, 0], origin).to_bits(), 0);
    }
    // The retained significand can be just beyond the compact storage.
    assert_eq!(finish_at([0, 0, 0, 1 << 63], [0; 4], -1330).to_bits(), 0);
    assert_eq!(finish_at([1, 0, 0, 1 << 63], [0; 4], -1330).to_bits(), 1);
}

#[test]
fn compact_rounding_preserves_halfway_sticky_and_overflow_boundaries() {
    for exponent in [-1022, -1000, -1, 0, 500, 1023] {
        let high = (2148 + exponent) as usize;
        let base = (high - 180) / 64 * 64;
        let origin = base as i32 - 2148;
        for variant in 0..4 {
            let mut positive = [0; 4];
            let mut negative = [0; 4];
            add_product(&mut positive, 1, high - base);
            add_product(&mut positive, 1, high - 53 - base);
            match variant {
                1 => add_product(&mut positive, 1, high - 180 - base),
                2 => add_product(&mut positive, 1, high - 52 - base),
                3 => add_product(&mut negative, 1, high - 180 - base),
                _ => {}
            }
            let expected = reference(positive, negative, origin);
            assert_eq!(
                finish_at(positive, negative, origin).to_bits(),
                expected.to_bits()
            );
            assert_eq!(
                finish_at(negative, positive, origin).to_bits(),
                (-expected).to_bits()
            );
        }
    }
    let base = 3008;
    let origin = base as i32 - 2148;
    let mut positive = [0; 4];
    let mut negative = [0; 4];
    add_product(&mut positive, (1_u128 << 53) - 1, 2148 + 971 - base);
    add_product(&mut positive, 1, 2148 + 970 - base);
    assert_eq!(finish_at(positive, negative, origin), f64::INFINITY);
    add_product(&mut negative, 1, 2148 + 890 - base);
    assert_eq!(finish_at(positive, negative, origin), f64::MAX);
}
