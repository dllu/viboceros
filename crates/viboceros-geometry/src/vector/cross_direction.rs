//! Unit normals without first rounding an unrepresentable cross product.
use super::{GeometryError, UnitVector3, Vector3};
use crate::{
    Point3,
    exact_scalar::{rational, scalar},
};
use num_traits::{Signed, Zero};

impl Vector3 {
    /// Direction of `self × (end - start)`, with an exact range-recovery path.
    /// None means exact collinearity of the stored inputs. A large coordinate
    /// origin or a quadratic cross-product scale must not choose that branch.
    pub fn unit_cross_point_difference(
        self,
        start: Point3,
        end: Point3,
    ) -> Result<Option<UnitVector3>, GeometryError> {
        let from = start.to_array();
        let to = end.to_array();
        let difference: [_; 3] =
            std::array::from_fn(|i| crate::parameter::exact_difference(to[i], from[i]));
        if let [Some(x), Some(y), Some(z)] = difference
            && let Ok(cross) = self.cross(Vector3::try_new(x, y, z)?)
            && cross
                .to_array()
                .into_iter()
                .map(f64::abs)
                .fold(0., f64::max)
                >= f64::MIN_POSITIVE
        {
            return Ok(Some(cross.normalized_nonzero()?));
        }
        // Normalize exact determinants before converting to f64. Scaling the
        // complete input vectors first could erase a tiny transverse offset.
        let a = self.to_array().map(rational);
        let delta: [_; 3] = std::array::from_fn(|i| rational(to[i]) - rational(from[i]));
        let cross: [_; 3] = std::array::from_fn(|i| {
            &a[(i + 1) % 3] * &delta[(i + 2) % 3] - &a[(i + 2) % 3] * &delta[(i + 1) % 3]
        });
        let scale = cross.iter().map(Signed::abs).max().unwrap();
        if scale.is_zero() {
            return Ok(None);
        }
        Ok(Some(
            Vector3::try_new(
                scalar(&(&cross[0] / &scale))?,
                scalar(&(&cross[1] / &scale))?,
                scalar(&(&cross[2] / &scale))?,
            )?
            .normalized_nonzero()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(v: [f64; 3]) -> Point3 {
        Point3::try_from(v).unwrap()
    }

    #[test]
    fn cross_directions_keep_the_same_plane_when_raw_products_leave_f64_range() {
        let expected = Vector3::try_new(12., -8., 13.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        for exponent in [-1000, -600, -200, 0, 200, 600, 1000] {
            let scale = 2_f64.powi(exponent);
            let vector = Vector3::try_new(2. * scale, 3. * scale, 0.).unwrap();
            let a = point([5. * scale, -7. * scale, 11. * scale]);
            let b = point([2. * scale, -5. * scale, 15. * scale]);
            let normal = vector.unit_cross_point_difference(a, b).unwrap().unwrap();
            assert!(normal.as_vector().dot(expected.as_vector()).unwrap() > 1. - 8. * f64::EPSILON);
            assert!(
                normal
                    .as_vector()
                    .dot(vector.normalized_nonzero().unwrap().as_vector())
                    .unwrap()
                    .abs()
                    < 8. * f64::EPSILON
            );
            let opposite = vector.unit_cross_point_difference(b, a).unwrap().unwrap();
            assert!(
                opposite.as_vector().dot(expected.as_vector()).unwrap() < -1. + 8. * f64::EPSILON
            );
            assert_eq!(
                vector
                    .unit_cross_point_difference(
                        point([0.; 3]),
                        point([4. * scale, 6. * scale, 0.])
                    )
                    .unwrap(),
                None
            );
        }
    }

    #[test]
    fn an_overflowing_difference_retains_the_small_coordinate_that_defines_the_plane() {
        let v = Vector3::try_new(0., 2_f64.powi(977), 0.).unwrap();
        let normal = v
            .unit_cross_point_difference(point([0., -1e308, 1e-200]), point([0., 1e308, -1e-200]))
            .unwrap()
            .unwrap();
        assert_eq!(normal.as_vector().to_array(), [-1., 0., 0.]);
        assert_eq!(
            v.unit_cross_point_difference(point([0., -1e308, 0.]), point([0., 1e308, 0.]))
                .unwrap(),
            None
        );
    }

    #[test]
    fn subnormal_products_are_normalized_before_rounding_to_zero() {
        let v = Vector3::try_new(0.25, 0., 0.).unwrap();
        let tiny = f64::from_bits(1);
        let normal = v
            .unit_cross_point_difference(point([0.; 3]), point([0., tiny, tiny]))
            .unwrap()
            .unwrap();
        let expected = Vector3::try_new(0., -1., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        assert_eq!(normal, expected);
    }
    #[test]
    fn rounded_point_subtraction_cannot_create_a_plane_for_exact_collinearity() {
        let components: [f64; 3] = [1.2345678901234567, -0.9876543210987654, 0.3333333333333333];
        let scale = 1.456_789_012_345_679e150;
        let end = components.map(|v| v * scale);
        let start = std::array::from_fn(|i| components[i].mul_add(-scale, end[i]));
        let vector = Vector3::try_from(components).unwrap();
        assert_eq!(
            vector
                .unit_cross_point_difference(point(start), point(end))
                .unwrap(),
            None
        );
    }
}
