//! Exact sign of a finite binary64 3x3 determinant, without rounding its value.
use crate::binary_accumulator::{add_product, decompose};
use std::cmp::Ordering;

pub(super) fn determinant_sign(rows: [[f64; 3]; 3]) -> Ordering {
    // Triple products have quantum 2^-3222 and magnitude below 2^3072.
    // Six terms need at most 6297 bits; 99 limbs provide 6336.
    let mut positive = [0; 99];
    let mut negative = [0; 99];
    for (columns, odd) in [
        ([0, 1, 2], false),
        ([1, 2, 0], false),
        ([2, 0, 1], false),
        ([0, 2, 1], true),
        ([1, 0, 2], true),
        ([2, 1, 0], true),
    ] {
        let [a, b, c] = std::array::from_fn(|i| rows[i][columns[i]]);
        let (a_bits, a_shift) = decompose(a);
        let (b_bits, b_shift) = decompose(b);
        let (c_bits, c_shift) = decompose(c);
        let target = if odd ^ a.is_sign_negative() ^ b.is_sign_negative() ^ c.is_sign_negative() {
            &mut negative
        } else {
            &mut positive
        };
        let product = u128::from(a_bits) * u128::from(b_bits);
        let shift = a_shift + b_shift + c_shift;
        // Split the 106-bit product before the third multiplication so each
        // partial product fits u128. Accumulate both parts without rounding.
        add_product(
            target,
            u128::from(product as u64) * u128::from(c_bits),
            shift,
        );
        add_product(target, (product >> 64) * u128::from(c_bits), shift + 64);
    }
    positive.iter().rev().cmp(negative.iter().rev())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausts_ternary_matrices_including_singular_cases_and_subnormal_rows() {
        for mut code in 0..3_u32.pow(9) {
            let integers: [[i32; 3]; 3] = std::array::from_fn(|_| {
                std::array::from_fn(|_| {
                    let value = (code % 3) as i32 - 1;
                    code /= 3;
                    value
                })
            });
            let [a, b, c] = integers;
            let expected = (a[0] * (b[1] * c[2] - b[2] * c[1])
                - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                .cmp(&0);
            for scales in [[1.; 3], [f64::from_bits(1), f64::MAX, 1.]] {
                let rows = std::array::from_fn(|i| integers[i].map(|v| f64::from(v) * scales[i]));
                assert_eq!(determinant_sign(rows), expected, "{integers:?}, {scales:?}");
                let transposed = std::array::from_fn(|i| std::array::from_fn(|j| rows[j][i]));
                assert_eq!(determinant_sign(transposed), expected);
                let mut negated = rows;
                negated[0] = negated[0].map(|v| -v);
                assert_eq!(determinant_sign(negated), expected.reverse());
            }
        }
    }

    #[test]
    fn distinguishes_exact_cancellation_from_a_unit_determinant() {
        assert_eq!(determinant_sign([[f64::MAX; 3]; 3]), Ordering::Equal);
        let n = 2_f64.powi(52);
        assert_eq!(
            determinant_sign([[n, n - 1., 0.], [n + 1., n, 0.], [0., 0., 1.]]),
            Ordering::Greater
        );
    }

    #[test]
    fn matches_integer_determinants_under_extreme_binary_row_scaling() {
        let mut state = 42_u64;
        for _ in 0..1000 {
            let rows: [[i128; 3]; 3] = std::array::from_fn(|_| {
                std::array::from_fn(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    i128::from(state & 0xfffff) - 0x80000
                })
            });
            let [a, b, c] = rows;
            let determinant = a[0] * (b[1] * c[2] - b[2] * c[1])
                - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
            for exponents in [
                [0, 0, 0],
                [-1000, 0, 1000],
                [1000, 1000, 1000],
                [-1000, -1000, -1000],
            ] {
                let scaled =
                    std::array::from_fn(|i| rows[i].map(|v| v as f64 * 2_f64.powi(exponents[i])));
                assert_eq!(determinant_sign(scaled), determinant.cmp(&0));
                let mut swapped = scaled;
                swapped.swap(0, 1);
                assert_eq!(determinant_sign(swapped), determinant.cmp(&0).reverse());
            }
        }
    }
}
