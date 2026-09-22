//! Exact ordering without rounding away a common, dominant distance component.
use super::*;
use crate::binary_accumulator::{add_product, decompose};
use std::cmp::Ordering;

/// Exact ordering of two independent endpoint pairs, without a rounded origin
/// shift or allocation. 18 products, at most twice MAX², fit in 66 limbs at
/// quantum 2^-2148, including every same-sign carry.
pub(crate) fn compare_chords(first: [Point3; 2], second: [Point3; 2]) -> Ordering {
    let mut sum = DistanceComparison::default();
    for axis in 0..3 {
        let [a, b] = first.map(|p| p.to_array()[axis]);
        let [c, d] = second.map(|p| p.to_array()[axis]);
        if (a == c && b == d) || (a == d && b == c) {
            continue;
        }
        // (a-b)²-(c-d)² = a²+b²-2ab-c²-d²+2cd.
        for (x, y, subtract, twice) in [
            (a, a, false, false),
            (b, b, false, false),
            (a, b, true, true),
            (c, c, true, false),
            (d, d, true, false),
            (c, d, false, true),
        ] {
            sum.add(x, y, subtract, twice);
        }
    }
    sum.ordering()
}

struct DistanceComparison {
    positive: [u64; 66],
    negative: [u64; 66],
}

impl Default for DistanceComparison {
    fn default() -> Self {
        Self {
            positive: [0; 66],
            negative: [0; 66],
        }
    }
}

impl DistanceComparison {
    fn add(&mut self, a: Real, b: Real, subtract: bool, twice: bool) {
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let target = if a.is_sign_negative() ^ b.is_sign_negative() ^ subtract {
            &mut self.negative
        } else {
            &mut self.positive
        };
        add_product(
            target,
            u128::from(a_bits) * u128::from(b_bits),
            a_shift + b_shift + usize::from(twice),
        );
    }

    fn ordering(&self) -> Ordering {
        self.positive.iter().rev().cmp(self.negative.iter().rev())
    }
}

impl Point3 {
    /// Orders the distances from this point to `first` and `second` exactly for
    /// their stored binary64 coordinates, even when distances overflow or tie
    /// after rounding. No square root or rounded coordinate subtraction is used.
    pub fn compare_distances(self, first: Self, second: Self) -> Ordering {
        if first == second {
            return Ordering::Equal;
        }
        // 12 products, at most twice MAX² each: 66 limbs at quantum 2^-2148
        // cover every finite binary64 input and all carries (highest bit <4201).
        let mut sum = DistanceComparison::default();
        for ((t, a), b) in self
            .to_array()
            .into_iter()
            .zip(first.to_array())
            .zip(second.to_array())
        {
            if a == b {
                continue;
            }
            // |a-t|² - |b-t|² = a² - b² - 2ta + 2tb; t² cancels exactly.
            sum.add(a, a, false, false);
            sum.add(b, b, true, false);
            sum.add(t, a, true, true);
            sum.add(t, b, false, true);
        }
        sum.ordering()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_chord_order_matches_full_range_rational_differences() {
        use num_rational::BigRational as R;
        let mut state = 0x57917198324_u64;
        for _ in 0..512 {
            let mut next = || loop {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let x = Real::from_bits(state);
                if x.is_finite() {
                    break x;
                }
            };
            let points: [Point3; 4] = std::array::from_fn(|_| p(std::array::from_fn(|_| next())));
            let squared = |a: Point3, b: Point3| {
                a.to_array()
                    .into_iter()
                    .zip(b.to_array())
                    .map(|(a, b)| {
                        let difference = R::from_float(a).unwrap() - R::from_float(b).unwrap();
                        &difference * &difference
                    })
                    .sum::<R>()
            };
            assert_eq!(
                compare_chords([points[0], points[1]], [points[2], points[3]]),
                squared(points[0], points[1]).cmp(&squared(points[2], points[3]))
            );
        }
    }

    #[test]
    fn product_distance_order_matches_independent_finite_binary64_rationals() {
        use num_rational::BigRational as R;
        use num_traits::Zero;
        let mut state = 314159_u64;
        for case in 0..512 {
            let mut next = || loop {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let value = Real::from_bits(state);
                if value.is_finite() {
                    break value;
                }
            };
            let target = p(std::array::from_fn(|_| next()));
            let a = p(std::array::from_fn(|_| next()));
            let mut b = std::array::from_fn(|_| next());
            if case % 2 == 0 {
                b[0] = a.x();
            }
            let b = p(b);
            let squared = |point: Point3| {
                target.to_array().into_iter().zip(point.to_array()).fold(
                    R::zero(),
                    |sum, (t, a)| {
                        let d = R::from_float(t).unwrap() - R::from_float(a).unwrap();
                        sum + &d * &d
                    },
                )
            };
            assert_eq!(target.compare_distances(a, b), squared(a).cmp(&squared(b)));
        }
    }
    fn p(a: [Real; 3]) -> Point3 {
        Point3::try_from(a).unwrap()
    }

    #[test]
    fn exact_distance_order_retains_subnormal_differences_and_overflowing_distances() {
        let target = p([0., 0., Real::MAX]);
        let a = p([0.; 3]);
        let b = p([Real::from_bits(1), 0., 0.]);
        assert_eq!(
            target.distance_to(a).unwrap(),
            target.distance_to(b).unwrap()
        );
        assert_eq!(target.compare_distances(a, b), Ordering::Less);
        assert_eq!(target.compare_distances(b, a), Ordering::Greater);
        assert_eq!(
            target.compare_distances(p([1., 0., 0.]), p([0., -1., 0.])),
            Ordering::Equal
        );
        let target = p([Real::MAX, 0., 0.]);
        assert_eq!(
            target.compare_distances(p([-Real::MAX, 0., 0.]), a),
            Ordering::Greater
        );
        assert_eq!(a.compare_distances(p([-0., 0., 0.]), a), Ordering::Equal);
    }

    #[test]
    fn exact_distance_order_matches_integer_squared_norms_across_binary_scales() {
        let mut state = 143_u64;
        for _ in 0..1000 {
            let mut triple = [[0_i128; 3]; 3];
            for point in &mut triple {
                for value in point {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    *value = i128::from((state >> 32) as u8) - 128;
                }
            }
            let squared = |a: [i128; 3]| {
                a.into_iter()
                    .zip(triple[0])
                    .map(|(a, t)| (a - t) * (a - t))
                    .sum::<i128>()
            };
            let expected = squared(triple[1]).cmp(&squared(triple[2]));
            for exponent in [-1000, -500, 0, 500, 1000] {
                let scale = 2_f64.powi(exponent);
                let [target, a, b] = triple.map(|v| p(v.map(|v| v as Real * scale)));
                assert_eq!(target.compare_distances(a, b), expected);
            }
        }
    }
}
