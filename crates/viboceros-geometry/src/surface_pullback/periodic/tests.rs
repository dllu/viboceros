use super::*;
use crate::{AffineTransform3, Frame3, SurfaceKnotDirection, UnitVector3, WeightedPoint3};
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn cylinder() -> NurbsSurface {
    let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    NurbsSurface::try_cylinder(
        Frame3::try_from_unit_normal(p(0., 0., 0.), axis).unwrap(),
        3.,
        0.,
        3.,
    )
    .unwrap()
}
fn crossing(surface: &NurbsSurface) -> NurbsCurve {
    let arc = surface
        .isocurve_u(1.5)
        .unwrap()
        .try_trimmed(0.2..=1.2)
        .unwrap();
    assert_eq!(arc.control_points().len(), 3);
    let rotation = AffineTransform3::try_rotation(
        p(0., 0., 0.),
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        -0.6,
    )
    .unwrap();
    let arc = arc.transformed(rotation).unwrap();
    NurbsCurve::try_new_rational(
        arc.degree(),
        arc.control_points()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                WeightedPoint3::try_new(
                    p(c.point().x(), c.point().y(), 0.5 + i as Real),
                    c.weight(),
                )
                .unwrap()
            })
            .collect(),
        arc.knots().to_vec(),
    )
    .unwrap()
}
fn check(surface: &NurbsSurface, source: &NurbsCurve, tolerance: Tolerance) -> NurbsCurve2 {
    check_n(surface, source, tolerance, 1)
}
fn check_n(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    tolerance: Tolerance,
    expected: usize,
) -> NurbsCurve2 {
    let before = (surface.clone(), source.clone());
    let uv = surface
        .try_pullback_curve_certified(source, tolerance)
        .unwrap();
    assert_eq!(uv.domain(), source.domain());
    assert!(
        surface
            .parameter_curve_deviation_bound(&uv, source, tolerance.absolute())
            .unwrap()
            .is_some()
    );
    let order = uv.degree() + 1;
    let seams = uv
        .knots()
        .chunk_by(|a, b| a == b)
        .filter(|k| k.len() == order && k[0] > *uv.domain().start() && k[0] < *uv.domain().end())
        .map(|k| k[0])
        .collect::<Vec<_>>();
    assert_eq!(seams.len(), expected);
    for t in seams {
        let a = uv.evaluate_on_side(t, crate::ParameterSide::Left).unwrap();
        let b = uv.evaluate_on_side(t, crate::ParameterSide::Right).unwrap();
        assert!(
            (a.x() - b.x()).abs() > (*surface.domain_u().end() - *surface.domain_u().start()) * 0.9
        );
        assert!(
            surface
                .evaluate(a.x(), a.y())
                .unwrap()
                .distance_to(surface.evaluate(b.x(), b.y()).unwrap())
                .unwrap()
                < tolerance.absolute() * 1e-3
        );
    }
    for i in 0..=128 {
        let t = source.parameter_at(i as Real / 128.).unwrap();
        let q = uv.evaluate(t).unwrap();
        assert!(
            surface
                .evaluate(q.x(), q.y())
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                <= tolerance.absolute()
        );
    }
    assert_eq!((surface, source), (&before.0, &before.1));
    uv
}
#[test]
fn repeated_crossings_keep_each_independent_event_in_both_directions() {
    let surface = cylinder();
    let circle = surface
        .isocurve_u(1.5)
        .unwrap()
        .transformed(
            AffineTransform3::try_rotation(
                p(0., 0., 0.),
                UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
                -0.6,
            )
            .unwrap(),
        )
        .unwrap();
    let lap = |start: Real| {
        NurbsCurve::try_new_rational(
            circle.degree(),
            circle
                .control_points()
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    WeightedPoint3::try_new(
                        p(
                            c.point().x(),
                            c.point().y(),
                            0.25 + start + i as Real * 0.125,
                        ),
                        c.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            circle.knots().to_vec(),
        )
        .unwrap()
        .try_reparameterized(start..=start + 1.)
        .unwrap()
    };
    let source = lap(0.).try_append_clamped(&lap(1.)).unwrap();
    for source in [source.clone(), source.reversed().unwrap()] {
        check_n(&surface, &source, Tolerance::DEFAULT, 2);
    }
}
#[test]
fn a_single_rational_span_crosses_the_seam_in_both_directions() {
    let surface = cylinder();
    let source = crossing(&surface);
    for source in [source.clone(), source.reversed().unwrap()] {
        check(&surface, &source, Tolerance::DEFAULT);
    }
}
#[test]
fn a_seam_at_an_original_source_knot_keeps_independent_limits() {
    let surface = cylinder();
    let pi = std::f64::consts::PI;
    let shifted = surface
        .try_change_closed_seam(SurfaceKnotDirection::U, pi, 0.)
        .unwrap();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(2. * pi - 0.3, 0.5).unwrap(),
        Point2::try_new(2. * pi + 0.4, 2.5).unwrap(),
    )
    .unwrap();
    let source = shifted
        .try_pushup_curve_certified(&uv, Tolerance::try_new(1e-8, 1e-14, 1e-9).unwrap())
        .unwrap();
    check(&surface, &source, Tolerance::DEFAULT);
}
#[test]
fn reciprocal_surface_charts_and_native_curve_times_keep_the_same_image() {
    let surface = cylinder();
    let source = crossing(&surface).try_reparameterized(-7. ..=11.).unwrap();
    let surface = surface
        .try_reparameterized(
            2_f64.powi(600)..=2_f64.powi(601),
            -2_f64.powi(-600)..=2_f64.powi(-600),
        )
        .unwrap();
    check(&surface, &source, Tolerance::DEFAULT);
}

#[test]
fn simultaneous_torus_seams_share_one_chart_event_inside_one_span() {
    let ring = [
        (1., 0.),
        (1., 1.),
        (0., 1.),
        (-1., 1.),
        (-1., 0.),
        (-1., -1.),
        (0., -1.),
        (1., -1.),
        (1., 0.),
    ];
    let knots = vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.];
    let make = |phase: Real| {
        let (s, c) = phase.sin_cos();
        let rotate = |(x, y): (Real, Real)| (c.mul_add(x, -s * y), s.mul_add(x, c * y));
        let controls = ring
            .iter()
            .enumerate()
            .flat_map(|(v, &minor)| {
                let minor = rotate(minor);
                ring.iter().enumerate().map(move |(u, &major)| {
                    let major = rotate(major);
                    let weight = (if u % 2 == 0 { 1. } else { 0.5_f64.sqrt() })
                        * (if v % 2 == 0 { 1. } else { 0.5_f64.sqrt() });
                    WeightedPoint3::try_new(
                        p((4. + minor.0) * major.0, (4. + minor.0) * major.1, minor.1),
                        weight,
                    )
                    .unwrap()
                })
            })
            .collect();
        NurbsSurface::try_new_rational(2, 2, 9, 9, controls, knots.clone(), knots.clone()).unwrap()
    };
    let target = make(0.);
    let shifted = make(-0.6);
    let uv = NurbsCurve2::try_line(
        Point2::try_new(0.05, 0.05).unwrap(),
        Point2::try_new(0.2, 0.2).unwrap(),
    )
    .unwrap();
    let source = shifted
        .try_pushup_curve_certified(&uv, Tolerance::try_new(1e-8, 1e-14, 1e-9).unwrap())
        .unwrap();
    assert_eq!(source.spans().count(), 1);
    let pulled = check(&target, &source, Tolerance::DEFAULT);
    let t = pulled
        .knots()
        .chunk_by(|a, b| a == b)
        .find(|g| {
            g.len() == pulled.degree() + 1
                && g[0] > *pulled.domain().start()
                && g[0] < *pulled.domain().end()
        })
        .unwrap()[0];
    let a = pulled
        .evaluate_on_side(t, crate::ParameterSide::Left)
        .unwrap();
    let b = pulled
        .evaluate_on_side(t, crate::ParameterSide::Right)
        .unwrap();
    assert!((a.y() - b.y()).abs() > 0.9);
}
