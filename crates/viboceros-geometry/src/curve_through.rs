use crate::{
    ControlPointCurveClosure, CurveInterpolationOptions, CurveKnotSpacing, GeometryError,
    InterpolatedCurveClosure, NurbsCurve, Point3, Real,
};

/// Largest control-point degree exposed by Rhino's curve-through commands.
pub const MAX_CURVE_THROUGH_DEGREE: usize = 11;

/// Construction used when fitting a curve through existing point locations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveThroughConstruction {
    /// Treats the locations as the curve's control points.
    ControlPoint,
    /// Interpolates every location with the requested parameter spacing.
    Interpolated(CurveKnotSpacing),
}

/// Orders points into a greedy nearest-neighbor chain and removes exactly
/// coincident locations.
///
/// The first input point fixes the start, ties retain input order, and each
/// subsequent point is the closest unused one.
pub fn sort_and_cull_points(points: &[Point3]) -> Result<Vec<Point3>, GeometryError> {
    let mut unique = Vec::with_capacity(points.len());
    for point in points {
        if !unique.contains(point) {
            unique.push(*point);
        }
    }
    if unique.len() < 2 {
        return Ok(unique);
    }

    let mut remaining = unique;
    let mut ordered = Vec::with_capacity(remaining.len());
    ordered.push(remaining.remove(0));
    while !remaining.is_empty() {
        let current = ordered[ordered.len() - 1];
        let mut closest_index = 0;
        let mut closest_distance = current.distance_to(remaining[0])?;
        for (index, candidate) in remaining.iter().enumerate().skip(1) {
            let distance = current.distance_to(*candidate)?;
            if distance < closest_distance {
                closest_index = index;
                closest_distance = distance;
            }
        }
        ordered.push(remaining.remove(closest_index));
    }
    Ok(ordered)
}

/// Constructs a Rhino-style curve through an ordered point sequence.
///
/// Control-point curves accept degrees 1 through 11 and lower the degree when
/// there are too few locations. Interpolated curves currently accept the same
/// degree-one and degree-three modes as [`NurbsCurve::try_interpolate`]. Open
/// control-point output is parameterized over its integer span count; periodic
/// control-point output uses one unit per unique input point.
pub fn try_curve_through_points(
    points: &[Point3],
    requested_degree: usize,
    construction: CurveThroughConstruction,
    closed: bool,
) -> Result<NurbsCurve, GeometryError> {
    if requested_degree == 0 || requested_degree > MAX_CURVE_THROUGH_DEGREE {
        return Err(GeometryError::InvalidCurveThroughDegree {
            actual: requested_degree,
            maximum: MAX_CURVE_THROUGH_DEGREE,
        });
    }

    match construction {
        CurveThroughConstruction::ControlPoint => {
            let closure = if closed {
                ControlPointCurveClosure::Smooth
            } else {
                ControlPointCurveClosure::Open
            };
            let curve = NurbsCurve::try_control_point_curve_with_closure(
                requested_degree,
                points.to_vec(),
                closure,
            )?;
            let domain_length = if curve.is_periodic() {
                let duplicate_endpoint =
                    usize::from(points.len() > 1 && points.first() == points.last());
                points.len() - duplicate_endpoint
            } else {
                curve.control_points().len() - curve.degree()
            };
            curve.try_reparameterized(0.0..=domain_length as Real)
        }
        CurveThroughConstruction::Interpolated(knot_spacing) => {
            let closure = if closed {
                InterpolatedCurveClosure::Smooth
            } else {
                InterpolatedCurveClosure::Open
            };
            NurbsCurve::try_interpolate_for_curve_through(
                points,
                CurveInterpolationOptions::new(requested_degree, knot_spacing, closure),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn assert_near(actual: Real, expected: Real, epsilon: Real) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn assert_point_near(actual: Point3, expected: [Real; 3], epsilon: Real) {
        for (actual, expected) in actual.to_array().into_iter().zip(expected) {
            assert_near(actual, expected, epsilon);
        }
    }

    #[test]
    fn sort_and_cull_matches_rhino_nearest_neighbor_order() {
        let input = [
            point(10.0, 0.0, 0.0),
            point(8.0, 1.0, 0.0),
            point(6.0, -2.0, 1.0),
            point(4.0, 3.0, 0.0),
            point(2.0, -1.0, 1.0),
            point(1.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, -1.0, 1.0),
        ];
        let ordered = sort_and_cull_points(&input).unwrap();
        assert_eq!(
            ordered,
            [
                input[0], input[1], input[2], input[4], input[6], input[5], input[3]
            ]
        );

        let distinct_within_document_tolerance =
            [point(1.0, 1.0, 0.0), point(1.0 + 1.0e-12, 1.0, 0.0)];
        assert_eq!(
            sort_and_cull_points(&distinct_within_document_tolerance).unwrap(),
            distinct_within_document_tolerance
        );
    }

    #[test]
    fn open_control_point_curve_uses_integer_span_domain() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 1.0, 0.0),
            point(4.0, -1.0, 0.0),
            point(7.0, 2.0, 1.0),
            point(10.0, 0.0, 0.0),
        ];
        let curve =
            try_curve_through_points(&points, 3, CurveThroughConstruction::ControlPoint, false)
                .unwrap();
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.domain(), 0.0..=2.0);
        assert_eq!(
            curve.knots(),
            &[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0]
        );
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            points
        );
    }

    #[test]
    fn uniform_cubic_interpolation_matches_rhino_curve_through_points() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 1.0, 0.0),
            point(4.0, -1.0, 0.0),
            point(7.0, 2.0, 1.0),
            point(10.0, 0.0, 0.0),
        ];
        let curve = try_curve_through_points(
            &points,
            3,
            CurveThroughConstruction::Interpolated(CurveKnotSpacing::Uniform),
            false,
        )
        .unwrap();
        let expected = [
            [0.0, 0.0, 0.0],
            [0.532_539_658_070_798_5, 0.521_495_031_747_563_8, 0.0],
            [
                2.135_634_291_212_022,
                2.367_606_557_879_314_6,
                0.127_889_853_550_423_5,
            ],
            [
                3.726_470_493_651_726_6,
                -3.068_865_500_198_947_8,
                -0.447_614_487_426_482_3,
            ],
            [
                6.958_483_734_181_074,
                3.907_855_442_916_475_6,
                1.662_568_096_155_505_8,
            ],
            [
                9.279_224_291_143_01,
                0.927_580_966_660_855_2,
                0.419_084_100_588_141_44,
            ],
            [10.0, 0.0, 0.0],
        ];
        for (control, expected) in curve.control_points().iter().zip(expected) {
            assert_point_near(control.point(), expected, 2.0e-14);
        }
        assert_eq!(curve.domain(), 0.0..=4.0);
    }

    #[test]
    fn interpolation_retains_distinct_points_below_document_tolerance() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(1.0, 2.0, 0.0),
            point(2.0, -1.0, 1.0),
            point(2.0 + 2.0e-10, -1.0, 1.0),
            point(4.0, 3.0, 0.0),
            point(6.0, -2.0, 1.0),
        ];
        let curve = try_curve_through_points(
            &points,
            3,
            CurveThroughConstruction::Interpolated(CurveKnotSpacing::Uniform),
            false,
        )
        .unwrap();
        assert_eq!(curve.domain(), 0.0..=5.0);
        for (parameter, expected) in points.into_iter().enumerate() {
            assert!(
                curve
                    .evaluate(parameter as Real)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1.0e-12
            );
        }
    }

    #[test]
    fn closed_modes_are_periodic_and_remove_a_repeated_endpoint() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
        ];
        for construction in [
            CurveThroughConstruction::ControlPoint,
            CurveThroughConstruction::Interpolated(CurveKnotSpacing::Uniform),
        ] {
            let curve = try_curve_through_points(&points, 3, construction, true).unwrap();
            assert!(curve.is_periodic());
            assert!(curve.is_closed().unwrap());
            assert_eq!(curve.domain(), 0.0..=4.0);
            assert_eq!(curve.control_points().len(), 7);
        }
    }

    #[test]
    fn rejects_invalid_and_unsupported_degrees() {
        let points = [point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)];
        assert!(
            try_curve_through_points(&points, 0, CurveThroughConstruction::ControlPoint, false,)
                .is_err()
        );
        assert!(
            try_curve_through_points(
                &points,
                5,
                CurveThroughConstruction::Interpolated(CurveKnotSpacing::Uniform),
                false,
            )
            .is_err()
        );
    }
}
