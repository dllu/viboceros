//! Filtered comparison of a directional cosine with a binary64 threshold.
use super::*;
use crate::exact_scalar::rational;
use num_traits::{Signed, Zero};
use std::cmp::Ordering;

const ROUND_GUARD: Real = 128. * Real::EPSILON;

pub(super) fn compare(
    a: [Real; 3],
    b: [Real; 3],
    cosine: Real,
    budget: &mut usize,
) -> Result<Option<Ordering>, GeometryError> {
    let (av, bv) = (Vector3::try_from(a)?, Vector3::try_from(b)?);
    if av.length()? == 0. || bv.length()? == 0. {
        return Ok(None);
    }
    let dot = av
        .normalized_nonzero()?
        .as_vector()
        .dot(bv.normalized_nonzero()?.as_vector())?;
    Ok(Some(compare_with_dot(a, b, cosine, dot, budget)?))
}

pub(super) fn compare_with_dot(
    a: [Real; 3],
    b: [Real; 3],
    cosine: Real,
    dot: Real,
    budget: &mut usize,
) -> Result<Ordering, GeometryError> {
    if (dot - cosine).abs() > ROUND_GUARD {
        return Ok(dot.total_cmp(&cosine));
    }
    // The rounded normalization cannot decide values inside its error guard.
    // Compare dot / sqrt(aa * bb) exactly for these supplied coefficients,
    // taking signs before squaring so obtuse angles retain their meaning.
    charge(budget, 512)?;
    let (a, b) = (a.map(rational), b.map(rational));
    let dot = (0..3)
        .map(|i| &a[i] * &b[i])
        .sum::<num_rational::BigRational>();
    let sign = dot.cmp(&num_rational::BigRational::zero());
    let threshold = cosine.partial_cmp(&0.).unwrap();
    if sign != threshold {
        return Ok(sign.cmp(&threshold));
    }
    if sign.is_eq() {
        return Ok(Ordering::Equal);
    }
    let aa = a.iter().map(|x| x * x).sum::<num_rational::BigRational>();
    let bb = b.iter().map(|x| x * x).sum::<num_rational::BigRational>();
    let c = rational(cosine);
    let order = (&dot * &dot).cmp(&(&c * &c * aa * bb));
    Ok(if dot.is_negative() {
        order.reverse()
    } else {
        order
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_comparison_resolves_the_normalization_roundoff_band() {
        for scale in [1e-120, 1., 1e120] {
            let a = [3. * scale, 4. * scale, 0.];
            let b = [scale, 0., 0.];
            // These strict inequalities also hold for the rounded scaled
            // components. No normalized binary64 dot can distinguish them all.
            for (cosine, expected) in [
                (0.6_f64.next_down(), Ordering::Greater),
                (0.6_f64.next_up(), Ordering::Less),
                ((-0.6_f64).next_down(), Ordering::Greater),
                ((-0.6_f64).next_up(), Ordering::Less),
            ] {
                let b = if cosine < 0. { b.map(|v| -v) } else { b };
                assert_eq!(compare(a, b, cosine, &mut 4096).unwrap(), Some(expected));
            }
        }
        for cosine in [-1., 0., 1.] {
            let b = if cosine == 0. {
                [0., 1., 0.]
            } else {
                [cosine, 0., 0.]
            };
            assert_eq!(
                compare([1., 0., 0.], b, cosine, &mut 4096).unwrap(),
                Some(Ordering::Equal)
            );
        }
        assert_eq!(compare([0.; 3], [1., 0., 0.], 0., &mut 4096).unwrap(), None);
        assert!(compare([3., 4., 0.], [1., 0., 0.], 0.6, &mut 0).is_err());
    }
}
