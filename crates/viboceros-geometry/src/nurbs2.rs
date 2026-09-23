use std::ops::RangeInclusive;

use crate::nurbs::validate_direction;
use crate::{GeometryError, Point2, Real, require_finite};
mod evaluate;
mod integration_frame;

/// A two-dimensional Euclidean control point with a finite, nonzero weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint2 {
    point: Point2,
    weight: Real,
}

impl WeightedPoint2 {
    pub fn try_new(point: Point2, weight: Real) -> Result<Self, GeometryError> {
        if weight.is_finite() && weight != 0.0 {
            Ok(Self { point, weight })
        } else {
            Err(GeometryError::InvalidWeight { index: 0 })
        }
    }

    #[inline]
    pub const fn point(self) -> Point2 {
        self.point
    }

    #[inline]
    pub const fn weight(self) -> Real {
        self.weight
    }
}

/// A finite rational B-spline curve in a surface's parameter space.
///
/// The full knot-vector convention matches [`crate::NurbsCurve`]. A separate
/// 2D type is essential for B-rep trims: its coordinates are `(u, v)`, not
/// model-space `(x, y, z)` coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve2 {
    degree: usize,
    control_points: Vec<WeightedPoint2>,
    knots: Vec<Real>,
    rational: bool,
}

impl NurbsCurve2 {
    pub fn try_new(
        degree: usize,
        control_points: Vec<Point2>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        let control_points = control_points
            .into_iter()
            .map(|point| WeightedPoint2 { point, weight: 1.0 })
            .collect();
        Self::try_new_rational(degree, control_points, knots)
    }

    pub fn try_new_rational(
        degree: usize,
        control_points: Vec<WeightedPoint2>,
        knots: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        validate_direction(degree, control_points.len(), &knots)?;
        for (index, control_point) in control_points.iter().enumerate() {
            if !control_point.weight.is_finite() || control_point.weight == 0.0 {
                return Err(GeometryError::InvalidWeight { index });
            }
        }
        let first_weight = control_points[0].weight;
        let rational = control_points
            .iter()
            .any(|control_point| control_point.weight != first_weight);
        Ok(Self {
            degree,
            control_points,
            knots,
            rational,
        })
    }

    /// Constructs a degree-one trim with a normalized parameter domain.
    pub fn try_line(start: Point2, end: Point2) -> Result<Self, GeometryError> {
        if start == end {
            return Err(GeometryError::Degenerate {
                context: "parameter-space line",
            });
        }
        Self::try_new(1, vec![start, end], vec![0.0, 0.0, 1.0, 1.0])
    }

    #[inline]
    pub const fn degree(&self) -> usize {
        self.degree
    }

    #[inline]
    pub fn control_points(&self) -> &[WeightedPoint2] {
        &self.control_points
    }

    #[inline]
    pub fn knots(&self) -> &[Real] {
        &self.knots
    }

    #[inline]
    pub const fn is_rational(&self) -> bool {
        self.rational
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        self.knots[self.degree]..=self.knots[self.control_points.len()]
    }

    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        self.knots
            .windows(2)
            .skip(self.degree)
            .take(self.control_points.len() - self.degree)
            .filter_map(|knots| (knots[0] < knots[1]).then_some((knots[0], knots[1])))
    }

    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        if !normalized.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "normalized parameter-space NURBS parameter",
            });
        }
        if !(0.0..=1.0).contains(&normalized) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter: normalized,
                domain_start: 0.0,
                domain_end: 1.0,
            });
        }
        let domain = self.domain();
        let parameter = domain
            .start()
            .mul_add(1.0 - normalized, domain.end() * normalized);
        require_finite([parameter], "parameter-space NURBS parameter")?;
        Ok(parameter)
    }

    pub fn start_point(&self) -> Result<Point2, GeometryError> {
        self.evaluate(*self.domain().start())
    }

    pub fn end_point(&self) -> Result<Point2, GeometryError> {
        self.evaluate(*self.domain().end())
    }

    /// Proves that the entire curve traces its endpoint segment.
    ///
    /// Clamped endpoints, C0 continuity, same-sign weights, and controls in
    /// the closed endpoint segment make this a convex-hull certificate. The
    /// control collinearity test uses exact binary64 rational arithmetic.
    pub fn is_straight_segment(&self) -> bool {
        use crate::exact_scalar::rational;

        let degree = self.degree;
        let controls = &self.control_points;
        let knots = &self.knots;
        if knots[0] != knots[degree] || knots[controls.len()] != *knots.last().unwrap() {
            return false;
        }
        let mut multiplicity = 1;
        for pair in knots[degree + 1..controls.len()].windows(2) {
            multiplicity = if pair[0] == pair[1] {
                multiplicity + 1
            } else {
                1
            };
            if multiplicity > degree {
                return false;
            }
        }
        let sign = controls[0].weight().is_sign_positive();
        if controls
            .iter()
            .any(|control| control.weight().is_sign_positive() != sign)
        {
            return false;
        }
        let a = controls[0].point().to_array();
        let b = controls.last().unwrap().point().to_array();
        if a == b {
            return false;
        }
        controls.iter().all(|control| {
            let p = control.point().to_array();
            if (0..2).any(|axis| p[axis] < a[axis].min(b[axis]) || p[axis] > a[axis].max(b[axis])) {
                return false;
            }
            if a[0] == b[0] || a[1] == b[1] || p == a || p == b {
                return true;
            }
            let [ax, ay] = a.map(rational);
            let [bx, by] = b.map(rational);
            let [px, py] = p.map(rational);
            (px - &ax) * (by - &ay) == (py - &ay) * (bx - &ax)
        })
    }

    /// Reverses direction and negates the knot vector, matching OpenNURBS.
    pub fn reversed(&self) -> Result<Self, GeometryError> {
        let control_points = self.control_points.iter().rev().copied().collect();
        let knots = self.knots.iter().rev().map(|knot| -*knot).collect();
        Self::try_new_rational(self.degree, control_points, knots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tolerance;

    fn point(x: Real, y: Real) -> Point2 {
        Point2::try_new(x, y).unwrap()
    }

    #[test]
    fn parameter_curve_evaluates_lines_and_rational_arcs() {
        let line = NurbsCurve2::try_line(point(-2.0, 3.0), point(4.0, 9.0)).unwrap();
        assert_eq!(line.degree(), 1);
        assert_eq!(line.domain(), 0.0..=1.0);
        assert_eq!(line.evaluate(0.25).unwrap(), point(-0.5, 4.5));

        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let arc = NurbsCurve2::try_new_rational(
            2,
            vec![
                WeightedPoint2::try_new(point(1.0, 0.0), 1.0).unwrap(),
                WeightedPoint2::try_new(point(1.0, 1.0), diagonal_weight).unwrap(),
                WeightedPoint2::try_new(point(0.0, 1.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let middle = arc.evaluate(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(middle.x(), diagonal_weight));
        assert!(Tolerance::DEFAULT.approx_eq(middle.y(), diagonal_weight));

        let parameter = 0.35;
        let (evaluated, derivative) = arc.evaluate_with_derivative(parameter).unwrap();
        assert_eq!(evaluated, arc.evaluate(parameter).unwrap());
        let step = 1.0e-6;
        let before = arc.evaluate(parameter - step).unwrap();
        let after = arc.evaluate(parameter + step).unwrap();
        assert!((derivative[0] - (after.x() - before.x()) / (2.0 * step)).abs() < 1.0e-9);
        assert!((derivative[1] - (after.y() - before.y()) / (2.0 * step)).abs() < 1.0e-9);
    }

    #[test]
    fn parameter_curve_reversal_preserves_locus_and_swaps_ends() {
        let curve = NurbsCurve2::try_line(point(2.0, 3.0), point(5.0, 7.0)).unwrap();
        let reversed = curve.reversed().unwrap();
        assert_eq!(reversed.domain(), -1.0..=0.0);
        assert_eq!(reversed.start_point().unwrap(), curve.end_point().unwrap());
        assert_eq!(reversed.end_point().unwrap(), curve.start_point().unwrap());
        for index in 0..=8 {
            let normalized = index as Real / 8.0;
            assert_eq!(
                reversed
                    .evaluate(reversed.parameter_at(normalized).unwrap())
                    .unwrap(),
                curve
                    .evaluate(curve.parameter_at(1.0 - normalized).unwrap())
                    .unwrap()
            );
        }
    }

    #[test]
    fn parameter_curve_rejects_invalid_structure_and_weights() {
        assert!(NurbsCurve2::try_line(point(1.0, 2.0), point(1.0, 2.0)).is_err());
        assert!(NurbsCurve2::try_new(0, vec![point(0.0, 0.0)], vec![0.0, 0.0]).is_err());
        assert!(
            NurbsCurve2::try_new_rational(
                1,
                vec![
                    WeightedPoint2::try_new(point(0.0, 0.0), 1.0).unwrap(),
                    WeightedPoint2 {
                        point: point(1.0, 0.0),
                        weight: Real::NAN,
                    },
                ],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .is_err()
        );
    }

    #[test]
    fn parameter_curve_supports_negative_projective_weights() {
        assert!(WeightedPoint2::try_new(point(0.0, 0.0), 0.0).is_err());
        let curve = NurbsCurve2::try_new_rational(
            2,
            vec![
                WeightedPoint2::try_new(point(0.0, 0.0), 1.0).unwrap(),
                WeightedPoint2::try_new(point(2.0, 3.0), -0.2).unwrap(),
                WeightedPoint2::try_new(point(5.0, 0.0), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let middle = curve.evaluate(0.5).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(middle.x(), 2.625));
        assert!(Tolerance::DEFAULT.approx_eq(middle.y(), -0.75));
    }

    #[test]
    fn exact_segment_certificate_accepts_collinear_splines_and_rejects_bulges() {
        let controls = |middle_y, middle_weight| {
            vec![
                WeightedPoint2::try_new(point(0., 0.), 1.).unwrap(),
                WeightedPoint2::try_new(point(0.5, middle_y), middle_weight).unwrap(),
                WeightedPoint2::try_new(point(1., 1.), 1.).unwrap(),
            ]
        };
        let knots = vec![0., 0., 0., 1., 1., 1.];
        let straight = NurbsCurve2::try_new_rational(2, controls(0.5, 0.3), knots.clone()).unwrap();
        assert!(straight.is_straight_segment());
        assert!(straight.reversed().unwrap().is_straight_segment());
        let bulge = f64::from_bits(0.5_f64.to_bits() + 1);
        assert!(
            !NurbsCurve2::try_new_rational(2, controls(bulge, 0.3), knots.clone())
                .unwrap()
                .is_straight_segment()
        );
        assert!(
            !NurbsCurve2::try_new_rational(2, controls(0.5, -0.3), knots)
                .unwrap()
                .is_straight_segment()
        );
        let polyline = NurbsCurve2::try_new(
            1,
            vec![point(0., 0.), point(0.5, 0.5), point(1., 1.)],
            vec![0., 0., 0.5, 1., 1.],
        )
        .unwrap();
        assert!(polyline.is_straight_segment());
    }
}
