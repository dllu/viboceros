use super::*;
use crate::{Frame3, Point3, UnitVector3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-7, 1e-12, 1e-9).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_unit_normal(
        p(0., 0., 0.),
        UnitVector3::try_new(0., 0., 1., tolerance()).unwrap(),
    )
    .unwrap()
}
fn plane() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        p(-10., -10., 0.),
        p(10., -10., 0.),
        p(10., 10., 0.),
        p(-10., 10., 0.),
    ])
    .unwrap()
}
fn profile() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        vec![
            p(-4., 0., 0.),
            p(-2., 3., 0.),
            p(2., -3., 0.),
            p(4., 0., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}
fn plane_offset(source: &NurbsCurve, t: Real, distance: Real) -> Point3 {
    let s = CurveRef::NurbsCurve(source)
        .evaluate_with_tangent(t)
        .unwrap();
    let v = s.tangent().as_vector().to_array();
    s.point()
        .translated(crate::Vector3::try_new(-v[1] * distance, v[0] * distance, 0.).unwrap())
        .unwrap()
}

#[test]
fn complete_planar_profiles_keep_signed_distance_and_the_source_parameter() {
    let s = plane();
    let c = profile();
    let original = (s.clone(), c.clone());
    for distance in [-1., -0.25, 0.25, 1.] {
        let result = try_offset_curve_on_surface(&s, &c, distance, tolerance()).unwrap();
        assert_eq!(result.curve.domain(), c.domain());
        assert!(result.geodesic_queries > 16);
        assert!(result.maximum_integration_estimate < tolerance().absolute());
        for i in 0..=1024 {
            let t = i as Real / 1024.;
            let expected = plane_offset(&c, t, distance);
            assert!(
                result
                    .curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < tolerance().absolute(),
                "distance={distance} t={t}"
            );
        }
    }
    assert_eq!((s, c), original);
}

#[test]
fn self_intersections_retain_independent_source_tangents() {
    // x=r²-1, y=r³-r, with r=3t-1.5. At r=±1 the source
    // passes through the same point with two independent tangent directions.
    let c = NurbsCurve::try_new(
        3,
        vec![
            p(1.25, -1.875, 0.),
            p(-1.75, 3.875, 0.),
            p(-1.75, -3.875, 0.),
            p(1.25, 1.875, 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let offset = try_offset_curve_on_surface(&plane(), &c, 0.3, tolerance()).unwrap();
    let mut points = Vec::new();
    for t in [1. / 6., 5. / 6.] {
        assert!(c.evaluate(t).unwrap().distance_to(p(0., 0., 0.)).unwrap() < 1e-14);
        let point = offset.curve.evaluate(t).unwrap();
        assert!(point.distance_to(plane_offset(&c, t, 0.3)).unwrap() < tolerance().absolute());
        points.push(point);
    }
    assert!(points[0].distance_to(points[1]).unwrap() > 0.4);
}

#[test]
fn cylinder_generator_offsets_travel_surface_arc_length() {
    let s = NurbsSurface::try_cylinder(frame(), 3., -4., 4.).unwrap();
    let c = s.isocurve_v(6.1).unwrap();
    let source = CurveRef::NurbsCurve(&c).start_point().unwrap();
    let angle = source.y().atan2(source.x());
    for distance in [-1., 1.] {
        let result = try_offset_curve_on_surface(&s, &c, distance, tolerance()).unwrap();
        for i in 0..=64 {
            let t = c.parameter_at(i as Real / 64.).unwrap();
            let q = result.curve.evaluate(t).unwrap();
            // U × V points outward; outward × vertical points clockwise.
            let theta = angle - distance / 3.;
            let expected = p(
                3. * theta.cos(),
                3. * theta.sin(),
                c.evaluate(t).unwrap().z(),
            );
            assert!(q.distance_to(expected).unwrap() < tolerance().absolute());
        }
    }
}

#[test]
fn closed_cylinder_rings_and_sphere_latitudes_retain_their_seam() {
    for sphere in [false, true] {
        let s = if sphere {
            NurbsSurface::try_sphere(frame(), 4.).unwrap()
        } else {
            NurbsSurface::try_cylinder(frame(), 3., -4., 4.).unwrap()
        };
        let c = s.isocurve_u(0.2).unwrap();
        let result = try_offset_curve_on_surface(&s, &c, 0.25, tolerance()).unwrap();
        assert!(
            result.geodesic_queries < 1000,
            "rational circles must not require a large polynomial refit: {} queries",
            result.geodesic_queries
        );
        assert_eq!(
            CurveRef::NurbsCurve(&result.curve).start_point().unwrap(),
            CurveRef::NurbsCurve(&result.curve).end_point().unwrap()
        );
        let start = CurveRef::NurbsCurve(&c).start_point().unwrap();
        let latitude = (start.z() / 4.).asin() + 0.25 / 4.;
        for i in 0..=512 {
            let t = c.parameter_at(i as Real / 512.).unwrap();
            let point = result.curve.evaluate(t).unwrap();
            let original = c.evaluate(t).unwrap();
            let angle = original.y().atan2(original.x());
            let expected = if sphere {
                p(
                    4. * latitude.cos() * angle.cos(),
                    4. * latitude.cos() * angle.sin(),
                    4. * latitude.sin(),
                )
            } else {
                p(original.x(), original.y(), original.z() + 0.25)
            };
            assert!(
                point.distance_to(expected).unwrap() < tolerance().absolute(),
                "sphere={sphere} t={t} {point:?} {expected:?}"
            );
        }
    }
}

#[test]
fn warped_offsets_match_an_independent_cartesian_integration_between_fit_knots() {
    let s = NurbsSurface::try_bilinear([
        p(-5., -5., 0.),
        p(5., -5., 0.),
        p(5., 5., 3.),
        p(-5., 5., -1.),
    ])
    .unwrap();
    let c = s.isocurve_u(0.4).unwrap();
    let result = try_offset_curve_on_surface(&s, &c, 0.75, tolerance()).unwrap();
    // Embedded implicit geodesic equation, independent of spline jets or UV.
    let rhs = |y: [Real; 6]| {
        let n = [-0.04 * y[1] - 0.2, -0.04 * y[0] - 0.1, 1.];
        let f = 0.08 * y[3] * y[4] / n.iter().map(|x| x * x).sum::<Real>();
        [y[3], y[4], y[5], f * n[0], f * n[1], f * n[2]]
    };
    let add = |a: [Real; 6], b: [Real; 6], h: Real| std::array::from_fn(|i| a[i] + h * b[i]);
    for i in 0..=128 {
        let t = c.parameter_at(i as Real / 128.).unwrap();
        let point = c.evaluate(t).unwrap();
        let dx = 0.04 * point.y() + 0.2;
        let dy = 0.04 * point.x() + 0.1;
        let direction = crate::Vector3::try_new(-dx * dy, 1. + dx * dx, dy)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
            .as_vector()
            .to_array();
        let mut y = [
            point.x(),
            point.y(),
            point.z(),
            direction[0],
            direction[1],
            direction[2],
        ];
        let h = 0.75 / 64.;
        for _ in 0..64 {
            let a = rhs(y);
            let b = rhs(add(y, a, h * 0.5));
            let c = rhs(add(y, b, h * 0.5));
            let d = rhs(add(y, c, h));
            y = std::array::from_fn(|i| y[i] + h * (a[i] + 2. * b[i] + 2. * c[i] + d[i]) / 6.);
        }
        let expected = p(y[0], y[1], y[2]);
        let actual = result.curve.evaluate(t).unwrap();
        assert!(
            actual.distance_to(expected).unwrap() < tolerance().absolute(),
            "t={t}: {actual:?} {expected:?}"
        );
        assert!(
            (actual.z()
                - 0.04 * actual.x() * actual.y()
                - 0.2 * actual.x()
                - 0.1 * actual.y()
                - 0.5)
                .abs()
                < 2. * tolerance().absolute()
        );
    }
}

#[test]
fn parameter_scaling_and_reversal_preserve_the_offset_locus() {
    let s = plane();
    let c = profile();
    for exponent in [-600, 0, 600] {
        let k = 2_f64.powi(exponent);
        let surface = s.try_reparameterized(-k..=k, 2. * k..=5. * k).unwrap();
        let source = c.try_reparameterized(-k..=k).unwrap();
        let offset = try_offset_curve_on_surface(&surface, &source, -0.3, tolerance()).unwrap();
        assert_eq!(offset.curve.domain(), source.domain());
        let reversed = source.reversed().unwrap();
        let reverse_offset =
            try_offset_curve_on_surface(&surface, &reversed, 0.3, tolerance()).unwrap();
        for i in 0..=64 {
            let fraction = i as Real / 64.;
            let expected = plane_offset(&c, fraction, -0.3);
            let point = offset
                .curve
                .evaluate(offset.curve.parameter_at(fraction).unwrap())
                .unwrap();
            let back = reverse_offset
                .curve
                .evaluate(reverse_offset.curve.parameter_at(1. - fraction).unwrap())
                .unwrap();
            assert!(point.distance_to(expected).unwrap() < tolerance().absolute());
            assert!(point.distance_to(back).unwrap() < tolerance().absolute());
        }
    }
}

#[test]
fn zero_distance_preserves_the_definition_and_invalid_sources_fail_atomically() {
    let s = plane();
    let c = profile();
    let zero = try_offset_curve_on_surface(&s, &c, 0., tolerance()).unwrap();
    assert_eq!(zero.curve, c);
    assert_eq!(zero.geodesic_queries, 0);
    let floating = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.1), p(1., 0., 0.1)],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(try_offset_curve_on_surface(&s, &floating, 0.2, tolerance()).is_err());
    assert!(try_offset_curve_on_surface(&s, &c, Real::NAN, tolerance()).is_err());
    assert!(matches!(
        try_offset_curve_on_surface(&s, &c, 30., tolerance()),
        Err(GeometryError::ParameterCurveOutsideSurfaceDomain)
    ));
    let kink = NurbsCurve::try_new(
        1,
        vec![p(-1., 0., 0.), p(0., 0., 0.), p(0., 1., 0.)],
        vec![0., 0., 0.5, 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        try_offset_curve_on_surface(&s, &kink, 0.2, tolerance()),
        Err(GeometryError::InvalidSurfaceCurveOffset {
            context: "source offset is discontinuous at an interior knot"
        })
    ));
}

#[test]
fn physical_scaling_retains_the_locus_and_native_parameter_roundoff_is_rejected() {
    for exponent in [-100, 100] {
        let scale = 2_f64.powi(exponent);
        let original_surface = plane();
        let surface = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            original_surface
                .control_points()
                .iter()
                .map(|q| {
                    let a = q.point().to_array();
                    p(a[0] * scale, a[1] * scale, a[2] * scale)
                })
                .collect(),
            original_surface.knots_u().to_vec(),
            original_surface.knots_v().to_vec(),
        )
        .unwrap();
        let original = profile();
        let curve = NurbsCurve::try_new(
            3,
            original
                .control_points()
                .iter()
                .map(|q| {
                    let a = q.point().to_array();
                    p(a[0] * scale, a[1] * scale, a[2] * scale)
                })
                .collect(),
            original.knots().to_vec(),
        )
        .unwrap();
        let tol = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
        let result = try_offset_curve_on_surface(&surface, &curve, 0.3 * scale, tol).unwrap();
        for i in 0..=64 {
            let t = i as Real / 64.;
            let expected = plane_offset(&curve, t, 0.3 * scale);
            assert!(
                result
                    .curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < tol.absolute()
            );
        }
    }
    let narrow = profile()
        .try_reparameterized(32. ..=32. + 2_f64.powi(-40))
        .unwrap();
    assert!(matches!(
        try_offset_curve_on_surface(&plane(), &narrow, 0.3, tolerance()),
        Err(GeometryError::InvalidSurfaceCurveOffset {
            context: "native curve parameters cannot represent the fitted locus accurately"
        })
    ));
}
