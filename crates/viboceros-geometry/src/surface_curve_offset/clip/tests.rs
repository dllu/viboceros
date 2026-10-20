use super::*;
use crate::{Circle3, Frame3, LineSegment, UnitVector3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tol(scale: Real) -> Tolerance {
    Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap()
}
fn rectangle(x: [Real; 2], y: [Real; 2]) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        p(x[0], y[0], 0.),
        p(x[1], y[0], 0.),
        p(x[1], y[1], 0.),
        p(x[0], y[1], 0.),
    ])
    .unwrap()
}
fn circle(center: Point3, radius: Real) -> NurbsCurve {
    Circle3::try_new(
        center,
        radius,
        UnitVector3::try_new(0., 0., 1., tol(1.)).unwrap(),
        tol(1.),
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn oblique_line_clips_both_ends_and_retains_native_source_times() {
    let surface = rectangle([0., 1.], [0., 1.]);
    let source = LineSegment::try_new(p(0., 0.2, 0.), p(1., 0.8, 0.), tol(1.))
        .unwrap()
        .to_nurbs()
        .unwrap()
        .try_reparameterized(10. ..=20.)
        .unwrap();
    let before = (surface.clone(), source.clone());
    let result = try_offset_curves_on_surface(&surface, &source, 0.25, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    assert!(result.extension_attempts > 0);
    let curve = &result.curves[0];
    let length = 1_f64.hypot(0.6);
    let dx = -0.25 * 0.6 / length;
    let dy = 0.25 / length;
    let a = -dx;
    let b = (1. - 0.2 - dy) / 0.6;
    assert!((*curve.domain().start() - (10. + 10. * a)).abs() < 1e-7);
    assert!((*curve.domain().end() - (10. + 10. * b)).abs() < 1e-7);
    for i in 0..=128 {
        let t = curve.parameter_at(i as Real / 128.).unwrap();
        let f = (t - 10.) / 10.;
        let expected = p(f + dx, 0.2 + 0.6 * f + dy, 0.);
        assert!(curve.evaluate(t).unwrap().distance_to(expected).unwrap() < tol(1.).absolute());
    }
    assert_eq!((surface, source), before);
}

#[test]
fn empty_and_zero_offsets_have_distinct_results() {
    let surface = rectangle([0., 1.], [0., 1.]);
    let source = LineSegment::try_new(p(0., 0.5, 0.), p(1., 0.5, 0.), tol(1.))
        .unwrap()
        .to_nurbs()
        .unwrap();
    assert!(
        try_offset_curves_on_surface(&surface, &source, 3., tol(1.))
            .unwrap()
            .curves
            .is_empty()
    );
    let zero = try_offset_curves_on_surface(&surface, &source, 0., tol(1.)).unwrap();
    assert_eq!(zero.curves, vec![source]);
    assert_eq!(zero.geodesic_queries, 0);
    assert_eq!(zero.extension_attempts, 0);
}

#[test]
fn a_circle_can_leave_four_separate_surviving_arcs() {
    let surface = rectangle([-1., 1.], [-1., 1.]);
    let source = circle(p(0., 0., 0.), 0.8);
    let result = try_offset_curves_on_surface(&surface, &source, -0.25, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 4);
    for curve in &result.curves {
        for i in 0..=128 {
            let q = curve
                .evaluate(curve.parameter_at(i as Real / 128.).unwrap())
                .unwrap();
            assert!(
                q.x().abs() <= 1. + tol(1.).absolute() && q.y().abs() <= 1. + tol(1.).absolute(),
                "{q:?}"
            );
            assert!((q.x().hypot(q.y()) - 1.05).abs() < tol(1.).absolute());
            if i == 0 || i == 128 {
                assert!((q.x().abs().max(q.y().abs()) - 1.).abs() < tol(1.).absolute());
            }
        }
    }
}

#[test]
fn tangencies_and_boundary_overlaps_do_not_fragment_surviving_curves() {
    let surface = rectangle([-1., 1.], [-1., 1.]);
    let source = circle(p(0., 0., 0.), 0.8);
    let result = try_offset_curves_on_surface(&surface, &source, -0.2, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    let c = &result.curves[0];
    assert_eq!(
        c.evaluate(*c.domain().start()).unwrap(),
        c.evaluate(*c.domain().end()).unwrap()
    );
    let source = LineSegment::try_new(p(-1., 0.8, 0.), p(1., 0.8, 0.), tol(1.))
        .unwrap()
        .to_nurbs()
        .unwrap();
    let result = try_offset_curves_on_surface(&surface, &source, 0.2, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    assert_eq!(result.curves[0].domain(), source.domain());
    for i in 0..=32 {
        let c = &result.curves[0];
        let p = c
            .evaluate(c.parameter_at(i as Real / 32.).unwrap())
            .unwrap();
        assert!((p.y() - 1.).abs() < tol(1.).absolute());
    }
}

#[test]
fn closed_source_survivors_join_across_the_source_seam() {
    let surface = rectangle([-1., 1.], [-1., 1.]);
    let source = circle(p(0., 0.3, 0.), 0.6);
    let result = try_offset_curves_on_surface(&surface, &source, -0.25, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    let curve = &result.curves[0];
    assert!(curve.domain().end() > source.domain().end());
    for i in 0..=256 {
        let p = curve
            .evaluate(curve.parameter_at(i as Real / 256.).unwrap())
            .unwrap();
        assert!((p.x().hypot(p.y() - 0.3) - 0.85).abs() < tol(1.).absolute());
        assert!(p.y() <= 1. + tol(1.).absolute());
        if i == 0 || i == 256 {
            assert!((p.y() - 1.).abs() < tol(1.).absolute());
        }
    }
}

#[test]
fn clipping_is_invariant_under_physical_and_parameter_rescaling() {
    for exponent in [-100, 0, 100] {
        let scale = 2_f64.powi(exponent);
        let parameter = 2_f64.powi(exponent * 6);
        let surface = rectangle([0., scale], [0., scale])
            .try_reparameterized(-parameter..=parameter, parameter..=3. * parameter)
            .unwrap();
        let source = LineSegment::try_new(
            p(0., 0.2 * scale, 0.),
            p(scale, 0.8 * scale, 0.),
            tol(scale),
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
        .try_reparameterized(-parameter..=parameter)
        .unwrap();
        let result =
            try_offset_curves_on_surface(&surface, &source, 0.25 * scale, tol(scale)).unwrap();
        assert_eq!(result.curves.len(), 1);
        let c = &result.curves[0];
        for i in 0..=32 {
            let p = c
                .evaluate(c.parameter_at(i as Real / 32.).unwrap())
                .unwrap();
            assert!(p.x() >= -tol(scale).absolute() && p.y() <= scale + tol(scale).absolute());
        }
    }
}

#[test]
fn periodic_surface_seams_are_not_clipping_borders() {
    let frame = Frame3::try_from_unit_normal(
        p(0., 0., 0.),
        UnitVector3::try_new(0., 0., 1., tol(1.)).unwrap(),
    )
    .unwrap();
    let surface = NurbsSurface::try_cylinder(frame, 3., -1., 1.).unwrap();
    let source = surface.isocurve_v(0.01).unwrap();
    let result = try_offset_curves_on_surface(&surface, &source, 1., tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    for i in 0..=32 {
        let c = &result.curves[0];
        let p = c
            .evaluate(c.parameter_at(i as Real / 32.).unwrap())
            .unwrap();
        assert!((p.x().hypot(p.y()) - 3.).abs() < tol(1.).absolute());
    }
}

#[test]
fn warped_patch_clips_to_both_natural_u_boundaries() {
    let surface = NurbsSurface::try_bilinear([
        p(-5., -5., 0.),
        p(5., -5., 0.),
        p(5., 5., 3.),
        p(-5., 5., -1.),
    ])
    .unwrap();
    let source = surface.isocurve_u(0.05).unwrap();
    let result = try_offset_curves_on_surface(&surface, &source, -0.25, tol(1.)).unwrap();
    assert_eq!(result.curves.len(), 1);
    let c = &result.curves[0];
    assert!(
        c.domain().start() > source.domain().start() && c.domain().end() < source.domain().end()
    );
    assert!((c.evaluate(*c.domain().start()).unwrap().x() + 5.).abs() < tol(1.).absolute());
    assert!((c.evaluate(*c.domain().end()).unwrap().x() - 5.).abs() < tol(1.).absolute());
    for i in 0..=128 {
        let p = c
            .evaluate(c.parameter_at(i as Real / 128.).unwrap())
            .unwrap();
        assert!(
            (p.z() - 0.04 * p.x() * p.y() - 0.2 * p.x() - 0.1 * p.y() - 0.5).abs()
                < tol(1.).absolute()
        );
    }
}

#[test]
fn reconnected_planar_gaps_preserve_shape_under_world_scaling() {
    for exponent in [-100, 0, 100] {
        let scale = 2_f64.powi(exponent);
        let surface = rectangle([-scale, scale], [-scale, scale]);
        let source = Circle3::try_new(
            p(0., 0., 0.),
            0.8 * scale,
            UnitVector3::try_new(0., 0., 1., tol(1.)).unwrap(),
            tol(scale),
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let result = try_offset_curves_on_surface_with_options(
            &surface,
            &source,
            -0.25 * scale,
            tol(scale),
            SurfaceCurveOffsetOptions {
                closed_gaps: SurfaceCurveOffsetGapStyle::ConnectClosed,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.curves.len(), 1);
        assert_eq!(result.connectors, 4);
        let c = &result.curves[0];
        assert_eq!(
            c.evaluate(*c.domain().start()).unwrap(),
            c.evaluate(*c.domain().end()).unwrap()
        );
        let max_y = c.tight_bounds(tol(scale)).unwrap().max().y();
        assert!(
            (max_y / scale - 1.05125).abs() < 1e-7,
            "exponent={exponent}: {max_y}"
        );
    }
}
