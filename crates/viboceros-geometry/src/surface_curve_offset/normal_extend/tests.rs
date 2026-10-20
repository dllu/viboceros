use super::*;
use crate::{Point2, Point3, UnitVector3};
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn cylinder(scale: Real) -> NurbsSurface {
    NurbsSurface::try_cylinder(
        Frame3::try_from_unit_normal(
            p(0., 0., 0.),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        )
        .unwrap(),
        3. * scale,
        0.,
        6. * scale,
    )
    .unwrap()
}
#[test]
fn requested_ends_preserve_native_times_under_reversal_and_reciprocal_charts() {
    for scale in [2_f64.powi(-100), 1., 2_f64.powi(100)] {
        let tolerance = Tolerance::try_new(scale * 1e-7, 1e-12, 1e-9).unwrap();
        let surface = cylinder(scale);
        let full = surface.isocurve_v(0.6).unwrap();
        let source = full
            .try_trimmed(1.5 * scale..=4.5 * scale)
            .unwrap()
            .try_reparameterized(-7. ..=11.)
            .unwrap();
        let surface = surface
            .try_reparameterized(
                2_f64.powi(600)..=2_f64.powi(601),
                -2_f64.powi(-600)..=2_f64.powi(-600),
            )
            .unwrap();
        for source in [source.clone(), source.reversed().unwrap()] {
            for side in [
                CurveExtensionSide::Start,
                CurveExtensionSide::End,
                CurveExtensionSide::Both,
            ] {
                let before = (surface.clone(), source.clone());
                let result =
                    try_extend_curve_normally_on_surface(&surface, &source, side, tolerance, None)
                        .unwrap_or_else(|e| {
                            panic!(
                                "scale={scale:e}, side={side:?}, domain={:?}: {e}",
                                source.domain()
                            )
                        });
                assert_eq!((&surface, &source), (&before.0, &before.1));
                for i in 0..=16 {
                    let t = source.parameter_at(i as Real / 16.).unwrap();
                    assert!(
                        source
                            .evaluate(t)
                            .unwrap()
                            .distance_to(result.curve.evaluate(t).unwrap())
                            .unwrap()
                            < tolerance.absolute()
                    );
                }
                if !matches!(side, CurveExtensionSide::End) {
                    assert!(*result.curve.domain().start() < *source.domain().start());
                }
                if !matches!(side, CurveExtensionSide::Start) {
                    assert!(*result.curve.domain().end() > *source.domain().end());
                }
            }
        }
    }
}
#[test]
fn a_closed_normal_section_stops_after_half_a_turn_then_can_reach_the_seam() {
    let surface = cylinder(1.);
    let source = surface
        .isocurve_u(3.)
        .unwrap()
        .try_trimmed(0.3..=1.3)
        .unwrap();
    let end = source.evaluate(*source.domain().end()).unwrap();
    let result = try_extend_curve_normally_on_surface(
        &surface,
        &source,
        CurveExtensionSide::Both,
        Tolerance::DEFAULT,
        None,
    )
    .unwrap();
    assert!(!result.curve.is_closed().unwrap());
    assert!(
        result
            .curve
            .evaluate(*result.curve.domain().start())
            .unwrap()
            .distance_to(p(3., 0., 3.))
            .unwrap()
            < 1e-7
    );
    assert!(
        result
            .curve
            .evaluate(*result.curve.domain().end())
            .unwrap()
            .distance_to(p(-end.x(), -end.y(), end.z()))
            .unwrap()
            < 1e-7
    );
    let next = try_extend_curve_normally_on_surface(
        &surface,
        &result.curve,
        CurveExtensionSide::Both,
        Tolerance::DEFAULT,
        None,
    )
    .unwrap();
    assert!(next.curve.is_closed().unwrap());
    assert!(next.seam_parameters().len() == 0);
}
#[test]
fn unsupported_interior_sections_fail_with_immutable_inputs() {
    let controls = (0..3)
        .flat_map(|u| (0..3).map(move |v| p(u as Real, v as Real, (u * v * v) as Real * 0.2)))
        .collect();
    let surface = NurbsSurface::try_clamped_uniform(2, 2, 3, 3, controls).unwrap();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(0.3, 0.3).unwrap(),
        Point2::try_new(0.7, 0.7).unwrap(),
    )
    .unwrap();
    let source = surface
        .try_pushup_curve_certified(&uv, Tolerance::DEFAULT)
        .unwrap();
    let before = (surface.clone(), source.clone());
    assert!(matches!(
        try_extend_curve_normally_on_surface(
            &surface,
            &source,
            CurveExtensionSide::Both,
            Tolerance::DEFAULT,
            None
        ),
        Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
    ));
    assert_eq!((surface, source), before);
}

#[test]
fn natural_endpoints_retain_one_complete_curve_and_two_certified_chart_portions() {
    let surface = cylinder(1.);
    let arc = surface
        .isocurve_u(3.)
        .unwrap()
        .try_trimmed(0.2..=1.2)
        .unwrap()
        .transformed(
            crate::AffineTransform3::try_rotation(
                p(0., 0., 0.),
                UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
                -0.6,
            )
            .unwrap(),
        )
        .unwrap();
    let source = NurbsCurve::try_new_rational(
        arc.degree(),
        arc.control_points()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                WeightedPoint3::try_new(p(c.point().x(), c.point().y(), i as Real * 3.), c.weight())
                    .unwrap()
            })
            .collect(),
        arc.knots().to_vec(),
    )
    .unwrap()
    .try_reparameterized(-7. ..=11.)
    .unwrap();
    let result = try_extend_curve_normally_on_surface(
        &surface,
        &source,
        CurveExtensionSide::Both,
        Tolerance::DEFAULT,
        None,
    )
    .unwrap();
    assert_eq!(result.curve, source);
    let cuts = result.seam_parameters().collect::<Vec<_>>();
    assert_eq!(cuts.len(), 1);
    let parts = result.parts(Tolerance::DEFAULT).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].domain(), -7. ..=cuts[0]);
    assert_eq!(parts[1].domain(), cuts[0]..=11.);
    for part in parts {
        for i in 0..=16 {
            let t = part.parameter_at(i as Real / 16.).unwrap();
            assert!(
                part.evaluate(t)
                    .unwrap()
                    .distance_to(source.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
}
