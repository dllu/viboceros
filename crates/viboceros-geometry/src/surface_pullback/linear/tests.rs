use super::*;
use crate::{Frame3, WeightedPoint3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-14, 1e-12).unwrap()
}

fn frame() -> Frame3 {
    Frame3::try_from_x_and_normal(
        p(7., -3., 5.),
        Vector3::try_new(2., 1., 0.).unwrap(),
        Vector3::try_new(1., -2., 3.).unwrap(),
        tolerance(),
    )
    .unwrap()
}

fn verify(surface: &NurbsSurface, curve: &NurbsCurve) -> NurbsCurve2 {
    let sources = (surface.clone(), curve.clone());
    let (uv, bound) = surface
        .try_pullback_curve_certified_with_bound(curve, None, tolerance())
        .unwrap();
    assert!(bound.is_finite() && bound >= 0. && bound <= tolerance().absolute());
    assert_eq!(uv.degree(), 1);
    assert_eq!(uv.control_points().len(), 2);
    assert_eq!(uv.domain(), curve.domain());
    assert!(
        surface
            .parameter_curve_deviation_bound(&uv, curve, tolerance().absolute())
            .unwrap()
            .is_some()
    );
    for fraction in [0., 0.125, 0.25, 0.5, 0.75, 0.875, 1.] {
        let point = uv.evaluate(uv.parameter_at(fraction).unwrap()).unwrap();
        assert!(
            surface
                .evaluate(point.x(), point.y())
                .unwrap()
                .distance_to(
                    curve
                        .evaluate(curve.parameter_at(fraction).unwrap())
                        .unwrap()
                )
                .unwrap()
                < tolerance().absolute()
        );
    }
    assert_eq!((&sources.0, &sources.1), (surface, curve));
    uv
}

#[test]
fn reported_bounds_cover_exact_inverse_proposals_and_all_assembled_fit_spans() {
    let affine =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0.5, 0.2, 0.), p(1., 1., 0.)],
        vec![2., 2., 2., 5., 5., 5.],
    )
    .unwrap();
    let (uv, bound) = affine
        .try_pullback_curve_certified_with_bound(&curve, None, tolerance())
        .unwrap();
    assert_eq!(uv.degree(), 2);
    assert_eq!(bound, 0.);
    assert_eq!(
        affine
            .parameter_curve_deviation_bound(&uv, &curve, tolerance().absolute())
            .unwrap(),
        Some(bound)
    );

    let graph =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(0.75, 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(7. / 16., 0., 0.), p(7. / 8., 0.5, 0.)],
        vec![2., 2., 2., 5., 5., 5.],
    )
    .unwrap();
    let sources = (graph.clone(), curve.clone());
    let (uv, bound) = graph
        .try_pullback_curve_certified_with_bound(&curve, None, tolerance())
        .unwrap();
    assert_eq!(uv.degree(), 3);
    assert!(uv.control_points().len() > 4);
    assert!(bound <= tolerance().absolute());
    assert_eq!(
        graph
            .parameter_curve_deviation_bound(&uv, &curve, tolerance().absolute())
            .unwrap(),
        Some(bound)
    );
    assert_eq!((graph, curve), sources);
}

#[test]
fn discovers_unconstrained_isocurves_in_both_directions_on_relocated_reversed_and_swapped_charts() {
    for surface in [
        NurbsSurface::try_cylinder(frame(), 2., 0., 3.).unwrap(),
        NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        NurbsSurface::try_torus(frame(), 4., 1.).unwrap(),
    ] {
        for chart in [
            surface.clone(),
            surface.try_reparameterized(-7. ..=19., -3. ..=6.).unwrap(),
            surface.try_reversed_u().unwrap(),
            surface.try_reversed_v().unwrap(),
            surface.try_swapped_uv().unwrap(),
        ] {
            for along_u in [true, false] {
                let curve = if along_u {
                    chart
                        .isocurve_u(chart.parameter_at_v(0.5).unwrap())
                        .unwrap()
                } else {
                    chart
                        .isocurve_v(chart.parameter_at_u(0.5).unwrap())
                        .unwrap()
                };
                for source in [curve.clone(), curve.reversed().unwrap()] {
                    verify(&chart, &source.try_reparameterized(2. ..=5.).unwrap());
                }
            }
        }
    }
}

#[test]
fn discovers_a_nonisocurve_with_a_singular_endpoint_without_primitive_recognition() {
    // S(u,v)=(u²,v,uv), C(t)=(t²,t,t²). Its starting U derivative
    // vanishes, so a Jacobian-inverting Hermite endpoint node cannot be built.
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(0., 1., 0.),
            p(0., 1., 0.5),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0., 0.5, 0.), p(1., 1., 1.)],
        vec![2., 2., 2., 5., 5., 5.],
    )
    .unwrap();
    for source in [curve.clone(), curve.reversed().unwrap()] {
        let uv = verify(&surface, &source);
        assert_eq!(
            surface
                .parameter_curve_deviation_bound(&uv, &source, 0.)
                .unwrap(),
            Some(0.)
        );
        assert_ne!(uv.start_point().unwrap().x(), uv.end_point().unwrap().x());
        assert_ne!(uv.start_point().unwrap().y(), uv.end_point().unwrap().y());
    }
}

fn two_poles() -> NurbsSurface {
    // S(u,v)=(4uv(1-v),4v(1-v),v). Both V boundaries collapse,
    // losing the constant U coordinate of an isocurve in endpoint searches.
    NurbsSurface::try_new(
        1,
        2,
        2,
        3,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(0., 2., 0.5),
            p(2., 2., 0.5),
            p(0., 0., 1.),
            p(0., 0., 1.),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn interior_witness_recovers_the_branch_when_both_endpoints_are_singular() {
    let surface = two_poles();
    let source = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0.75, 2., 0.5), p(0., 0., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        surface
            .try_pullback_curve(&source, tolerance())
            .unwrap()
            .degree(),
        1
    );
    for chart in [surface.clone(), surface.try_swapped_uv().unwrap()] {
        for curve in [source.clone(), source.reversed().unwrap()] {
            let uv = verify(&chart, &curve);
            assert_eq!(
                chart
                    .parameter_curve_deviation_bound(&uv, &curve, 0.)
                    .unwrap(),
                Some(0.)
            );
        }
    }
}

#[test]
fn fractional_witnesses_preserve_adjacent_subnormal_and_wide_source_domains() {
    let surface = two_poles();
    let controls = vec![p(0., 0., 0.), p(0.75, 2., 0.5), p(0., 0., 1.)];
    let big = 2_f64.powi(53);
    for [a, b] in [
        [big, big.next_up()],
        [0., Real::from_bits(1)],
        [-Real::MAX, Real::MAX],
    ] {
        let source = NurbsCurve::try_new(2, controls.clone(), vec![a, a, a, b, b, b]).unwrap();
        let (uv, bound) = surface
            .try_pullback_curve_certified_with_bound(&source, None, tolerance())
            .unwrap();
        assert_eq!(uv.degree(), 1);
        assert_eq!(uv.domain(), source.domain());
        assert_eq!(bound, 0.);
        assert_eq!(
            surface
                .parameter_curve_deviation_bound(&uv, &source, 0.)
                .unwrap(),
            Some(0.)
        );
        assert_eq!(
            source.parameter_sampler().unwrap().evaluate(0.5).unwrap(),
            p(0.375, 1., 0.5)
        );
    }
}

#[test]
fn lossless_uv_frames_preserve_a_certified_diagonal_on_an_adjacent_float_surface_domain() {
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(0., 1., 0.),
            p(0., 1., 0.5),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let big = 2_f64.powi(53);
    let surface = surface
        .try_reparameterized(big..=big.next_up(), -3. ..=6.)
        .unwrap();
    let source = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0., 0.5, 0.), p(1., 1., 1.)],
        vec![2., 2., 2., 5., 5., 5.],
    )
    .unwrap();
    let (uv, bound) = surface
        .try_pullback_curve_certified_with_bound(&source, None, tolerance())
        .unwrap();
    assert_eq!(uv.degree(), 1);
    assert_eq!(
        uv.start_point().unwrap(),
        Point2::try_new(big, -3.).unwrap()
    );
    assert_eq!(
        uv.end_point().unwrap(),
        Point2::try_new(big.next_up(), 6.).unwrap()
    );
    assert_eq!(bound, 0.);
    assert_eq!(
        surface
            .parameter_curve_deviation_bound(&uv, &source, 0.)
            .unwrap(),
        Some(0.)
    );
}

#[test]
fn discovered_paths_certify_signed_extreme_weight_gauges_without_changing_the_inputs() {
    let surface = two_poles();
    let source = surface.isocurve_v(0.375).unwrap();
    let surface = NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        surface
            .control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), -1e-180).unwrap())
            .collect(),
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )
    .unwrap();
    let source = NurbsCurve::try_new_rational(
        source.degree(),
        source
            .control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), -1e180).unwrap())
            .collect(),
        source.knots().to_vec(),
    )
    .unwrap();
    verify(&surface, &source);
}

#[test]
fn automatic_discovery_does_not_override_fixed_endpoints_or_use_relative_acceptance() {
    let surface = NurbsSurface::try_cylinder(frame(), 2., 0., 3.).unwrap();
    let curve = surface.isocurve_u(1.5).unwrap();
    verify(&surface, &curve);
    let endpoint = Point2::try_new(*surface.domain_u().start(), 1.5).unwrap();
    assert!(
        surface
            .try_pullback_curve_certified_with_endpoints(&curve, [endpoint; 2], tolerance())
            .is_err()
    );
    let displaced = NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    control
                        .point()
                        .translated(frame().x_axis().as_vector().scaled(0.01).unwrap())
                        .unwrap(),
                    control.weight(),
                )
                .unwrap()
            })
            .collect(),
        curve.knots().to_vec(),
    )
    .unwrap();
    assert!(
        surface
            .try_pullback_curve_certified(
                &displaced,
                Tolerance::try_new(1e-6, 100., 1e-12).unwrap()
            )
            .is_err()
    );
}
