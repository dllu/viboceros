//! Single-rounding affine endpoints without first rounding a knot ratio.
use super::*;
use crate::exact_scalar::{rational, scalar};

impl NurbsCurve {
    pub(super) fn try_trim_polynomial_line(
        &self,
        interval: &RangeInclusive<Real>,
    ) -> Result<Option<Self>, GeometryError> {
        if self.degree != 1
            || self.control_points.len() != 2
            || self.control_points[0].weight == 0.
            || self.control_points[0].weight != self.control_points[1].weight
        {
            return Ok(None);
        }
        let domain = self.domain();
        let a = rational(*domain.start());
        let width = rational(*domain.end()) - &a;
        let point_at = |t: Real| -> Result<WeightedPoint3, GeometryError> {
            if t == *domain.start() {
                return Ok(self.control_points[0]);
            }
            if t == *domain.end() {
                return Ok(self.control_points[1]);
            }
            let delta = rational(t) - &a;
            let remaining = &width - &delta;
            let mut point = [0.; 3];
            for (i, (left, right)) in self.control_points[0]
                .point
                .to_array()
                .into_iter()
                .zip(self.control_points[1].point.to_array())
                .enumerate()
            {
                point[i] =
                    scalar(&((rational(left) * &remaining + rational(right) * &delta) / &width))?;
            }
            WeightedPoint3::try_new(Point3::try_from(point)?, self.control_points[0].weight)
        };
        Ok(Some(Self::try_new_rational(
            1,
            vec![point_at(*interval.start())?, point_at(*interval.end())?],
            vec![
                *interval.start(),
                *interval.start(),
                *interval.end(),
                *interval.end(),
            ],
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimmed_line_preserves_exact_zero_crossings_and_common_weight_gauges() {
        for weight in [1., -2., 1e-280, 1e280] {
            let controls = [-1., 2.].map(|x| {
                WeightedPoint3::try_new(Point3::try_new(x, 2. * x, 7.).unwrap(), weight).unwrap()
            });
            for origin in [0., -10., 1e9] {
                let line = NurbsCurve::try_new_rational(
                    1,
                    controls.to_vec(),
                    vec![origin, origin, origin + 3., origin + 3.],
                )
                .unwrap();
                let cropped = line.try_trimmed(origin + 1. ..=origin + 2.).unwrap();
                assert_eq!(
                    cropped.control_points[0].point,
                    Point3::try_new(0., 0., 7.).unwrap()
                );
                assert_eq!(
                    cropped.control_points[1].point,
                    Point3::try_new(1., 2., 7.).unwrap()
                );
                assert!(cropped.control_points.iter().all(|c| c.weight == weight));
                assert_eq!(cropped.domain(), origin + 1. ..=origin + 2.);
                assert_eq!(line.try_trimmed(line.domain()).unwrap(), line);
            }
        }
    }

    #[test]
    fn affine_trim_handles_overflowing_and_subnormal_coordinate_differences() {
        for scale in [Real::MAX, Real::from_bits(2)] {
            let line = NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(-scale, 0., 0.).unwrap(),
                    Point3::try_new(scale, 0., 0.).unwrap(),
                ],
                vec![0., 0., 2., 2.],
            )
            .unwrap();
            let trimmed = line.try_trimmed(0.5..=1.).unwrap();
            assert_eq!(trimmed.control_points[0].point.x(), -scale / 2.);
            assert_eq!(trimmed.control_points[1].point.x(), 0.);
        }
    }
}
