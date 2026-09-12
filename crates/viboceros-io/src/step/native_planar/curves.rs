//! Native adapters for the supported straight STEP edge and trim representations.
use super::StepError;
use monstertruck::step::load::step_geometry::{Curve2D, Curve3D, ElementarySurface, Surface};
use viboceros_geometry::{NurbsCurve, NurbsCurve2, Point2, Point3, WeightedPoint2, WeightedPoint3};

pub(super) fn linear_edge(curve: &Curve3D, shell: u64) -> Result<NurbsCurve, StepError> {
    let unsupported = |reason| StepError::UnsupportedPlanarShell { shell, reason };
    let mut curve = curve;
    loop {
        match curve {
            Curve3D::SurfaceCurve(surface_curve) => curve = surface_curve.leader(),
            Curve3D::IntersectionCurve(intersection)
                if matches!(
                    intersection.surface0().as_ref(),
                    Surface::ElementarySurface(ElementarySurface::Plane(_))
                ) && matches!(
                    intersection.surface1().as_ref(),
                    Surface::ElementarySurface(ElementarySurface::Plane(_))
                ) =>
            {
                curve = intersection.leader()
            }
            _ => break,
        }
    }
    let curve = match curve {
        Curve3D::NurbsCurve(curve) if curve.degree() == 1 && curve.control_points().len() == 2 => {
            let mut points = Vec::with_capacity(2);
            for point in curve.control_points() {
                if !point.w.is_finite() || point.w <= 0.0 {
                    return Err(unsupported(
                        "rational edge weights must be finite and positive",
                    ));
                }
                points.push(WeightedPoint3::try_new(
                    Point3::try_new(point.x / point.w, point.y / point.w, point.z / point.w)?,
                    point.w,
                )?);
            }
            NurbsCurve::try_new_rational(1, points, curve.knot_vector().iter().copied().collect())?
        }
        _ => {
            let (points, knots) = match curve {
                Curve3D::Line(line) => (vec![line.0, line.1], vec![0.0, 0.0, 1.0, 1.0]),
                Curve3D::Polyline(curve) if curve.len() == 2 => {
                    (curve.0.clone(), vec![0., 0., 1., 1.])
                }
                Curve3D::BsplineCurve(curve)
                    if curve.degree() == 1 && curve.control_points().len() == 2 =>
                {
                    (
                        curve.control_points().clone(),
                        curve.knot_vector().iter().copied().collect(),
                    )
                }
                _ => return Err(unsupported("3D edge is not a single linear span")),
            };
            NurbsCurve::try_new(
                1,
                points
                    .into_iter()
                    .map(|point| Point3::try_new(point.x, point.y, point.z))
                    .collect::<Result<Vec<_>, _>>()?,
                knots,
            )?
        }
    };
    Ok(curve)
}

pub(super) fn linear_trim(curve: &Curve2D, shell: u64) -> Result<NurbsCurve2, StepError> {
    match curve {
        Curve2D::Line(line) => Ok(NurbsCurve2::try_line(
            Point2::try_new(line.0.x, line.0.y)?,
            Point2::try_new(line.1.x, line.1.y)?,
        )?),
        Curve2D::Polyline(curve) if curve.len() == 2 => Ok(NurbsCurve2::try_line(
            Point2::try_new(curve[0].x, curve[0].y)?,
            Point2::try_new(curve[1].x, curve[1].y)?,
        )?),
        Curve2D::BsplineCurve(curve)
            if curve.degree() == 1 && curve.control_points().len() == 2 =>
        {
            Ok(NurbsCurve2::try_new(
                1,
                curve
                    .control_points()
                    .iter()
                    .map(|p| Point2::try_new(p.x, p.y))
                    .collect::<Result<Vec<_>, _>>()?,
                curve.knot_vector().iter().copied().collect(),
            )?)
        }
        Curve2D::NurbsCurve(curve) if curve.degree() == 1 && curve.control_points().len() == 2 => {
            let mut points = Vec::with_capacity(2);
            for point in curve.control_points() {
                // Monstertruck stores homogeneous (u*w, v*w, w); the native
                // kernel stores Euclidean control points and separate weights.
                if !point.z.is_finite() || point.z <= 0.0 {
                    return Err(StepError::UnsupportedPlanarShell {
                        shell,
                        reason: "rational UV trim weights must be finite and positive",
                    });
                }
                points.push(WeightedPoint2::try_new(
                    Point2::try_new(point.x / point.z, point.y / point.z)?,
                    point.z,
                )?);
            }
            Ok(NurbsCurve2::try_new_rational(
                1,
                points,
                curve.knot_vector().iter().copied().collect(),
            )?)
        }
        _ => Err(StepError::UnsupportedPlanarShell {
            shell,
            reason: "UV trim is not a single linear span",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monstertruck::meshing::prelude::ParametricCurve;
    use monstertruck::modeling::{BsplineCurve, KnotVector, Point2 as TruckPoint2};

    #[test]
    fn two_point_polylines_preserve_linear_edge_and_trim_evaluation() {
        use monstertruck::meshing::prelude::PolylineCurve;
        use monstertruck::modeling::Point3 as TruckPoint3;
        for reversed in [false, true] {
            let mut points = vec![
                TruckPoint3::new(2., -5., 7.),
                TruckPoint3::new(11., 4., -2.),
            ];
            if reversed {
                points.reverse();
            }
            let uv = points.iter().map(|p| TruckPoint2::new(p.x, p.y)).collect();
            let source_edge = Curve3D::Polyline(PolylineCurve(points));
            let source_trim = Curve2D::Polyline(PolylineCurve(uv));
            let edge = linear_edge(&source_edge, 456).unwrap();
            let trim = linear_trim(&source_trim, 456).unwrap();
            assert_eq!(edge.domain(), 0.0..=1.0);
            assert_eq!(trim.domain(), 0.0..=1.0);
            for i in 0..=8 {
                let t = f64::from(i) / 8.;
                let expected = source_edge.evaluate(t);
                let actual = edge.evaluate(t).unwrap().to_array();
                for (a, b) in actual.into_iter().zip([expected.x, expected.y, expected.z]) {
                    assert!((a - b).abs() < 1e-12);
                }
                let expected = source_trim.evaluate(t);
                let actual = trim.evaluate(t).unwrap();
                assert!((actual.x() - expected.x).abs() < 1e-12);
                assert!((actual.y() - expected.y).abs() < 1e-12);
            }
        }
        for count in [0, 1, 3, 4] {
            let points = (0..count)
                .map(|i| TruckPoint3::new(f64::from(i), f64::from(i % 2), 0.))
                .collect::<Vec<_>>();
            let uv = points.iter().map(|p| TruckPoint2::new(p.x, p.y)).collect();
            assert!(linear_edge(&Curve3D::Polyline(PolylineCurve(points)), 456).is_err());
            assert!(linear_trim(&Curve2D::Polyline(PolylineCurve(uv)), 456).is_err());
        }
    }

    #[test]
    fn rational_edges_preserve_euclidean_controls_weights_and_parameterization() {
        use monstertruck::modeling::{NurbsCurve as TruckNurbs, Vector4 as TruckVector4};
        for weights in [[1., 4.], [4., 1.], [1e-100, 4e-100], [1e100, 4e100]] {
            for reversed in [false, true] {
                let mut points = [[2., -5., 7.], [11., 4., -2.]];
                if reversed {
                    points.reverse();
                }
                let controls = points
                    .iter()
                    .zip(weights)
                    .map(|(p, w)| TruckVector4::new(p[0] * w, p[1] * w, p[2] * w, w))
                    .collect();
                let source = Curve3D::NurbsCurve(TruckNurbs::new(BsplineCurve::new(
                    KnotVector::from(vec![-3., -3., 7., 7.]),
                    controls,
                )));
                let native = linear_edge(&source, 321).unwrap();
                assert_eq!(native.domain(), -3.0..=7.0);
                for (control, (point, weight)) in native
                    .control_points()
                    .iter()
                    .zip(points.into_iter().zip(weights))
                {
                    assert_eq!(control.weight(), weight);
                    for (actual, expected) in control.point().to_array().into_iter().zip(point) {
                        assert!((actual - expected).abs() < 1e-12);
                    }
                }
                for i in 0..=8 {
                    let fraction = f64::from(i) / 8.;
                    let a = (1. - fraction) * weights[0];
                    let b = fraction * weights[1];
                    let actual = native.evaluate(-3. + 10. * fraction).unwrap().to_array();
                    for axis in 0..3 {
                        let expected = (points[0][axis] * a + points[1][axis] * b) / (a + b);
                        assert!((actual[axis] - expected).abs() < 1e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn rational_edge_adapter_rejects_invalid_weights_coordinates_and_non_linear_spans() {
        use monstertruck::modeling::{NurbsCurve as TruckNurbs, Vector4 as TruckVector4};
        let curve = |knots, controls| {
            Curve3D::NurbsCurve(TruckNurbs::new(BsplineCurve::new(
                KnotVector::from(knots),
                controls,
            )))
        };
        for weight in [0., -1., f64::INFINITY, f64::NAN] {
            let source = curve(
                vec![0., 0., 1., 1.],
                vec![
                    TruckVector4::new(0., 0., 0., 1.),
                    TruckVector4::new(1., 0., 0., weight),
                ],
            );
            assert!(matches!(
                linear_edge(&source, 321),
                Err(StepError::UnsupportedPlanarShell { shell: 321, .. })
            ));
        }
        let unrepresentable = curve(
            vec![0., 0., 1., 1.],
            vec![
                TruckVector4::new(0., 0., 0., 1.),
                TruckVector4::new(f64::MAX, 0., 0., f64::MIN_POSITIVE),
            ],
        );
        assert!(matches!(
            linear_edge(&unrepresentable, 321),
            Err(StepError::Geometry(_))
        ));
        for knots in [vec![0., 0., 0., 1., 1., 1.], vec![0., 0., 0.5, 1., 1.]] {
            let source = curve(
                knots,
                vec![
                    TruckVector4::new(0., 0., 0., 1.),
                    TruckVector4::new(1., 2., 3., 1.),
                    TruckVector4::new(4., 0., 0., 1.),
                ],
            );
            assert!(matches!(
                linear_edge(&source, 321),
                Err(StepError::UnsupportedPlanarShell { shell: 321, .. })
            ));
        }
    }

    #[test]
    fn rational_linear_uv_trims_preserve_weights_and_nonuniform_parameterization() {
        use monstertruck::modeling::{NurbsCurve as TruckNurbs, Vector3 as TruckVector3};
        for weights in [[1., 4.], [4., 1.], [1e-100, 4e-100], [1e100, 4e100]] {
            let source = Curve2D::NurbsCurve(TruckNurbs::new(BsplineCurve::new(
                KnotVector::from(vec![-3., -3., 7., 7.]),
                vec![
                    TruckVector3::new(2. * weights[0], -5. * weights[0], weights[0]),
                    TruckVector3::new(11. * weights[1], 4. * weights[1], weights[1]),
                ],
            )));
            let native = linear_trim(&source, 123).unwrap();
            assert_eq!(native.domain(), -3.0..=7.0);
            assert_eq!(
                native
                    .control_points()
                    .iter()
                    .map(|p| p.weight())
                    .collect::<Vec<_>>(),
                weights
            );
            for i in 0..=8 {
                let fraction = f64::from(i) / 8.;
                let t = -3. + 10. * fraction;
                let a = (1. - fraction) * weights[0];
                let b = fraction * weights[1];
                let expected = [(2. * a + 11. * b) / (a + b), (-5. * a + 4. * b) / (a + b)];
                let actual = native.evaluate(t).unwrap();
                assert!((actual.x() - expected[0]).abs() < 1e-12);
                assert!((actual.y() - expected[1]).abs() < 1e-12);
            }
            assert!((native.evaluate(2.).unwrap().x() - 6.5).abs() > 1.);
        }
        for weight in [0., -1., f64::INFINITY, f64::NAN] {
            let source = Curve2D::NurbsCurve(TruckNurbs::new(BsplineCurve::new(
                KnotVector::from(vec![0., 0., 1., 1.]),
                vec![
                    TruckVector3::new(0., 0., 1.),
                    TruckVector3::new(1., 0., weight),
                ],
            )));
            assert!(linear_trim(&source, 123).is_err());
        }
    }

    #[test]
    fn linear_bspline_uv_trims_preserve_domain_direction_and_evaluation() {
        for interval in [[0., 1.], [-3., 7.], [100., 101.]] {
            for reversed in [false, true] {
                let mut points = vec![TruckPoint2::new(2., -5.), TruckPoint2::new(11., 4.)];
                if reversed {
                    points.reverse();
                }
                let source = Curve2D::BsplineCurve(BsplineCurve::new(
                    KnotVector::from(vec![interval[0], interval[0], interval[1], interval[1]]),
                    points,
                ));
                let native = linear_trim(&source, 123).unwrap();
                assert_eq!(native.domain(), interval[0]..=interval[1]);
                for station in 0..=8 {
                    let t = interval[0] + (interval[1] - interval[0]) * f64::from(station) / 8.;
                    let expected = source.evaluate(t);
                    let actual = native.evaluate(t).unwrap();
                    assert!((actual.x() - expected.x).abs() < 1e-12);
                    assert!((actual.y() - expected.y).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn linear_trim_adapter_rejects_curved_and_multispan_bspline_trims() {
        for knots in [vec![0., 0., 0., 1., 1., 1.], vec![0., 0., 0.5, 1., 1.]] {
            let source = Curve2D::BsplineCurve(BsplineCurve::new(
                KnotVector::from(knots),
                vec![
                    TruckPoint2::new(0., 0.),
                    TruckPoint2::new(1., 1.),
                    TruckPoint2::new(2., 0.),
                ],
            ));
            assert!(matches!(
                linear_trim(&source, 123),
                Err(StepError::UnsupportedPlanarShell { shell: 123, .. })
            ));
        }
    }
}
