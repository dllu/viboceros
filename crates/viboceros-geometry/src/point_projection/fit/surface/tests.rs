use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn plane_fit_rectangle_encloses_projected_data_and_has_physical_domains() {
    let points = [p(0., 0., 2.), p(4., 0., 2.), p(4., 3., 2.), p(0., 3., 2.)];
    for scale in [1e-100, 1., 1e100] {
        let points = points.map(|q| p(q.x() * scale, q.y() * scale, q.z() * scale));
        let surface = NurbsSurface::try_fit_plane(&points).unwrap().unwrap();
        assert_eq!(surface.domain_u(), 0. ..=3. * scale);
        assert_eq!(surface.domain_v(), 0. ..=4. * scale);
        assert_eq!([surface.degree_u(), surface.degree_v()], [1, 1]);
        for q in surface.control_points() {
            assert_eq!(q.point().z(), 2. * scale);
            assert_eq!(q.weight(), 1.);
        }
        for q in &points {
            assert!(surface.control_points().iter().any(|c| c.point() == *q));
        }
    }
}

#[test]
fn plane_fit_symmetric_nonplanar_data_uses_the_least_squares_plane() {
    let points = [
        p(-3., -2., -0.1),
        p(-3., -2., 0.1),
        p(3., -2., -0.1),
        p(3., -2., 0.1),
        p(3., 2., -0.1),
        p(3., 2., 0.1),
        p(-3., 2., -0.1),
        p(-3., 2., 0.1),
    ];
    let surface = NurbsSurface::try_fit_plane(&points).unwrap().unwrap();
    assert!(
        surface
            .control_points()
            .iter()
            .all(|p| p.point().z().abs() < 1e-14)
    );
    assert!((surface.domain_u().end() - 4.).abs() < 1e-14);
    assert!((surface.domain_v().end() - 6.).abs() < 1e-14);
}

#[test]
fn plane_fit_degenerate_sets_and_resource_limits_are_explicit() {
    assert!(NurbsSurface::try_fit_plane(&[p(0., 0., 0.); 2]).is_err());
    assert!(
        NurbsSurface::try_fit_plane(&[p(1., 2., 3.); 3])
            .unwrap()
            .is_none()
    );
    assert!(
        NurbsSurface::try_fit_plane(&[p(0., 0., 0.), p(1., 0., 0.), p(2., 0., 0.)])
            .unwrap()
            .is_none()
    );
    assert!(
        NurbsSurface::try_fit_plane(&[p(1., 2., 3.), p(2., 4., 6.), p(3., 6., 9.)])
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        NurbsSurface::try_fit_plane(&vec![p(0., 0., 0.); MAX_PLANE_FIT_POINTS + 1]),
        Err(GeometryError::PlaneFitResourceLimit { .. })
    ));
    assert!(
        PointProjection3::onto_line(p(0., 0., 0.), p(1., 2., 3.))
            .unwrap()
            .as_plane()
            .is_err()
    );
}

#[test]
fn plane_representation_normalizes_exact_cross_products_outside_float_range() {
    let h = 2_f64.powi(1000);
    let projection =
        PointProjection3::onto_three_point_plane([p(0., 0., 0.), p(h, 0., h), p(0., h, h)])
            .unwrap();
    let plane = projection.as_plane().unwrap();
    let n = plane.normal().as_vector().to_array();
    assert!((n[0].abs() - 1. / 3_f64.sqrt()).abs() < 1e-15);
    assert!((n[0] + n[2]).abs() < 1e-15 && (n[1] + n[2]).abs() < 1e-15);
}
