//! Conservative projection-image admission, with exact fallback at boundaries.
use super::*;
use crate::exact_scalar::{Rational, rational};
use crate::interval::I;
use num_traits::Zero;

fn interval_dot(a: [Real; 3], b: [Real; 3]) -> Option<I> {
    let mut sum = I::point(0.)?;
    for i in 0..3 {
        sum = sum.add(I::point(a[i])?.mul(I::point(b[i])?)?)?;
    }
    Some(sum)
}
fn exact_dot(a: &[Rational; 3], b: &[Rational; 3]) -> Rational {
    (0..3).map(|i| &a[i] * &b[i]).sum()
}

impl Plane {
    /// Classifies the represented direction exactly, including underflowing
    /// nonzero dot products. No angular tolerance changes a miss into a hit.
    pub fn direction_is_parallel(self, direction: Vector3) -> bool {
        let n = self.normal().as_vector().to_array();
        let d = direction.to_array();
        if let Some(dot) = interval_dot(n, d) {
            if dot.lo > 0. || dot.hi < 0. {
                return false;
            }
            if dot.lo == 0. && dot.hi == 0. {
                return true;
            }
        }
        exact_dot(&n.map(rational), &d.map(rational)).is_zero()
    }

    /// Checks Euclidean distance from a candidate to the ideal directional
    /// projection of the represented inputs. Parallel directions return false.
    /// Interval admission avoids rational work for well-conditioned queries;
    /// uncertain, subnormal and overflow cases compare squared errors exactly.
    pub fn directional_projection_within_tolerance(
        self,
        source: Point3,
        candidate: Point3,
        direction: Vector3,
        tolerance: Real,
    ) -> Result<bool, GeometryError> {
        crate::require_finite([tolerance], "directional projection tolerance")?;
        if tolerance < 0. {
            return Err(GeometryError::Degenerate {
                context: "negative directional projection tolerance",
            });
        }
        let n = self.normal().as_vector().to_array();
        let d = direction.to_array();
        let o = self.origin().to_array();
        let p = source.to_array();
        let c = candidate.to_array();
        let interval = (|| -> Option<bool> {
            let denominator = interval_dot(n, d)?;
            let mut numerator = I::point(0.)?;
            for i in 0..3 {
                numerator =
                    numerator.add(I::point(n[i])?.mul(I::point(o[i])?.sub(I::point(p[i])?)?)?)?;
            }
            let distance = numerator.div(denominator)?;
            let mut error = I::point(0.)?;
            for i in 0..3 {
                let ideal = I::point(p[i])?.add(distance.mul(I::point(d[i])?)?)?;
                let difference = ideal.sub(I::point(c[i])?)?;
                error = error.add(difference.mul(difference)?)?;
            }
            let limit = I::point(tolerance)?.mul(I::point(tolerance)?)?;
            if error.hi <= limit.lo {
                Some(true)
            } else if error.lo > limit.hi {
                Some(false)
            } else {
                None
            }
        })();
        if let Some(result) = interval {
            return Ok(result);
        }
        let n = n.map(rational);
        let d = d.map(rational);
        let o = o.map(rational);
        let p = p.map(rational);
        let c = c.map(rational);
        let denominator = exact_dot(&n, &d);
        if denominator.is_zero() {
            return Ok(false);
        }
        let displacement = std::array::from_fn(|i| &o[i] - &p[i]);
        let distance = exact_dot(&n, &displacement) / denominator;
        let error: Rational = (0..3)
            .map(|i| {
                let e = &p[i] + &d[i] * &distance - &c[i];
                &e * &e
            })
            .sum();
        let limit = rational(tolerance);
        Ok(error <= &limit * &limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn plane(origin: Point3, normal: [Real; 3]) -> Plane {
        Plane::new(
            origin,
            Vector3::try_from(normal)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
        )
    }
    #[test]
    fn one_ulp_admission_boundaries_and_zero_tolerance_are_exact() {
        let plane = plane(p(0., 0., 0.), [0., 0., 1.]);
        let d = Vector3::try_new(0., 0., 1.).unwrap();
        let s = p(1., 2., 3.);
        assert!(
            plane
                .directional_projection_within_tolerance(s, p(1., 2., 0.), d, 0.)
                .unwrap()
        );
        assert!(
            plane
                .directional_projection_within_tolerance(s, p(1., 2., 1.), d, 1.)
                .unwrap()
        );
        assert!(
            !plane
                .directional_projection_within_tolerance(s, p(1., 2., 1_f64.next_up()), d, 1.)
                .unwrap()
        );
        assert!(
            plane
                .directional_projection_within_tolerance(s, p(1., 2., 1_f64.next_down()), d, 1.)
                .unwrap()
        );
    }
    #[test]
    fn a_small_plane_residual_does_not_hide_large_near_parallel_error() {
        let plane = plane(p(0., 0., 0.), [0., 0., 1.]);
        let d = Vector3::try_new(1., 0., 1e-12).unwrap();
        let s = p(0., 0., 1.);
        assert!(
            plane
                .directional_projection_within_tolerance(s, p(-1e12, 0., 0.), d, 1e-3)
                .unwrap()
        );
        assert!(
            !plane
                .directional_projection_within_tolerance(s, p(-1e12 + 1., 0., 1e-12), d, 1e-3)
                .unwrap()
        );
    }
    #[test]
    fn nonzero_underflowing_direction_products_are_not_parallel() {
        let plane = plane(p(0., 0., 0.), [1., 0., 1e-308]);
        let d = Vector3::try_new(0., 1., 1e-308).unwrap();
        assert!(!plane.direction_is_parallel(d));
        assert!(
            plane
                .directional_projection_within_tolerance(p(0., 0., 1e-308), p(0., -1., 0.), d, 0.)
                .unwrap()
        );
        assert!(plane.direction_is_parallel(Vector3::try_new(0., 1., 0.).unwrap()));
        assert!(
            !plane
                .directional_projection_within_tolerance(
                    p(0., 0., 0.),
                    p(0., 0., 0.),
                    Vector3::try_new(0., 1., 0.).unwrap(),
                    1.
                )
                .unwrap()
        );
    }
    #[test]
    fn unrepresentable_displacements_and_subnormal_tolerances_use_exact_comparison() {
        let huge = 2_f64.powi(1023);
        let plane = plane(p(huge, huge, 2.), [0., 0., 1.]);
        let d = Vector3::try_new(0., 0., 1.).unwrap();
        assert!(
            plane
                .directional_projection_within_tolerance(
                    p(-huge, -huge, 5.),
                    p(-huge, -huge, 2.),
                    d,
                    0.
                )
                .unwrap()
        );
        let plane = super::tests::plane(p(0., 0., 0.), [0., 0., 1.]);
        let tiny = Real::from_bits(1);
        assert!(
            plane
                .directional_projection_within_tolerance(p(0., 0., 1.), p(0., 0., tiny), d, tiny)
                .unwrap()
        );
        assert!(
            !plane
                .directional_projection_within_tolerance(
                    p(0., 0., 1.),
                    p(0., 0., tiny * 2.),
                    d,
                    tiny
                )
                .unwrap()
        );
        assert!(
            plane
                .directional_projection_within_tolerance(p(0., 0., 1.), p(0., 0., 0.), d, -1.)
                .is_err()
        );
    }
}
