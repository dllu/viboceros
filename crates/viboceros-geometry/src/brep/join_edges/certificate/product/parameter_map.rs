//! Endpoint-preserving positive projective parameter correspondences.
use super::*;
use num_traits::Signed;

/// F(t) = c*t/(1-t+c*t). Positive c makes F an increasing bijection of [0,1].
/// All parameters stay rational: a mapped knot need not be a binary64 value.
pub(super) struct Map {
    factor: Rational,
}

impl Map {
    pub fn identity() -> Self {
        Self {
            factor: rational(1.),
        }
    }

    fn denominator(&self, t: &Rational) -> Rational {
        rational(1.) - t + &self.factor * t
    }

    pub fn apply(&self, t: &Rational) -> Rational {
        &self.factor * t / self.denominator(t)
    }

    pub fn inverse(&self, t: &Rational) -> Rational {
        t / (&self.factor * (rational(1.) - t) + t)
    }

    /// The source was extracted over [F(left),F(right)]. On the local interval,
    /// F has factor k = denominator(right)/denominator(left). Substitution in
    /// Bernstein degree p multiplies homogeneous control i by k^i; the common
    /// positive denominator cancels on projection. Coordinates and weights
    /// must both be scaled, not just the Euclidean weight field.
    pub fn compose_span(&self, controls: &mut [H], left: &Rational, right: &Rational) {
        if self.factor == rational(1.) {
            return;
        }
        let k = self.denominator(right) / self.denominator(left);
        let mut power = rational(1.);
        for h in controls {
            for x in h {
                *x *= &power;
            }
            power *= &k;
        }
    }
}

pub(super) fn candidates(
    a: &extract::Spline<'_>,
    b: &extract::Spline<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Vec<Map>, GeometryError> {
    let mut maps: Vec<Map> = Vec::with_capacity(2);
    for end in [false, true] {
        let (ac, awidth) = a.end_span(end, charge)?;
        let (bc, bwidth) = b.end_span(end, charge)?;
        charge(24)?;
        let da = derivative(&ac, &awidth, end);
        let db = derivative(&bc, &bwidth, end);
        // F'(0)=c and F'(1)=1/c. A dominant component avoids choosing a
        // nearly zero tangent coordinate. Even a poor proposal is safe: the
        // resulting map still has to bound every complete span.
        let (numerator, denominator) = if end { (&db, &da) } else { (&da, &db) };
        let axis = (0..3).max_by_key(|&i| denominator[i].abs()).unwrap();
        let factor = if denominator[axis] != rational(0.) && numerator[axis] != rational(0.) {
            &numerator[axis] / &denominator[axis]
        } else if ac.len() == bc.len() {
            // A stationary end has no first derivative. Equal-degree control
            // nets can still suggest the factor through adjacent weights.
            // This is only a proposal, including when their control loci differ.
            let (endpoint, adjacent) = if end {
                (ac.len() - 1, ac.len() - 2)
            } else {
                (0, 1)
            };
            let ratio = (&ac[adjacent][3] * &bc[endpoint][3] * &bwidth)
                / (&bc[adjacent][3] * &ac[endpoint][3] * &awidth);
            if end { rational(1.) / ratio } else { ratio }
        } else {
            continue;
        };
        if factor > rational(0.)
            && factor != rational(1.)
            && !maps.iter().any(|m| m.factor == factor)
        {
            maps.push(Map { factor });
        }
    }
    Ok(maps)
}

fn derivative(controls: &[H], width: &Rational, end: bool) -> [Rational; 3] {
    let n = controls.len() - 1;
    let (a, b, endpoint) = if end {
        (&controls[n - 1], &controls[n], &controls[n])
    } else {
        (&controls[0], &controls[1], &controls[0])
    };
    let scale = Rational::from_integer(n.into()) / (width * &endpoint[3] * &endpoint[3]);
    std::array::from_fn(|i| (&b[i] * &a[3] - &a[i] * &b[3]) * &scale)
}
