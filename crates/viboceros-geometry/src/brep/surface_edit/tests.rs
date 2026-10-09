use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap()
}
fn triangle() -> Brep {
    let curve = crate::Polyline3::try_new(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 0., 0.)],
        tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    Brep::try_planar_face(&curve, tolerance()).unwrap()
}
fn warped(source: &NurbsSurface) -> NurbsSurface {
    let u = source.domain_u();
    let v = source.domain_v();
    let controls = (0..=2)
        .flat_map(|j| (0..=2).map(move |i| p(i as Real / 2., j as Real / 2., (i * j) as Real / 4.)))
        .collect();
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        controls,
        [vec![*u.start(); 3], vec![*u.end(); 3]].concat(),
        [vec![*v.start(); 3], vec![*v.end(); 3]].concat(),
    )
    .unwrap()
}
fn assert_images(brep: &Brep) {
    let surface = brep.faces()[0].surface();
    for trim in brep.faces()[0].loops().iter().flat_map(|l| l.trims()) {
        if let Some(edge) = trim.edge() {
            let curve = if trim.is_reversed_3d() {
                brep.edges()[edge].curve().reversed().unwrap()
            } else {
                brep.edges()[edge].curve().clone()
            };
            let bound = surface
                .parameter_curve_deviation_bound(
                    trim.curve(),
                    &curve,
                    tolerance().absolute() * 0.25,
                )
                .unwrap();
            assert!(bound.is_some(), "edge={edge}, bound={bound:?}");
            for i in 0..=32 {
                let t = i as Real / 32.;
                let uv = trim
                    .curve()
                    .evaluate(trim.curve().parameter_at(t).unwrap())
                    .unwrap();
                let expected = surface.evaluate(uv.x(), uv.y()).unwrap();
                assert!(
                    curve
                        .evaluate(curve.parameter_at(t).unwrap())
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        <= tolerance().absolute() * 0.25
                );
            }
        }
    }
}
#[test]
fn edited_diagonal_boundaries_have_continuous_images_and_keep_trim_domains_and_orientation() {
    for reversed in [false, true] {
        let mut source = triangle();
        if reversed {
            source = source.reversed();
        }
        let before = source.clone();
        let target = warped(source.faces()[0].surface());
        let result = source
            .try_with_edited_single_surface(target.clone(), tolerance())
            .unwrap();
        assert_eq!(source, before);
        assert_eq!(result.faces()[0].surface(), &target);
        assert_eq!(result.faces()[0].is_reversed(), reversed);
        assert_eq!(
            result.faces()[0].loops().len(),
            source.faces()[0].loops().len()
        );
        for (a, b) in result.faces()[0].loops()[0]
            .trims()
            .iter()
            .zip(source.faces()[0].loops()[0].trims())
        {
            assert_eq!(a.curve(), b.curve());
            assert_eq!(a.is_reversed_3d(), b.is_reversed_3d());
        }
        assert_images(&result);
    }
}
#[test]
fn edited_circular_holes_are_certified_and_sources_are_unchanged() {
    let outer = crate::Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(4., 0., 0.),
            p(4., 4., 0.),
            p(0., 4., 0.),
            p(0., 0., 0.),
        ],
        tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let normal = UnitVector3::try_new(0., 0., 1., tolerance()).unwrap();
    let hole = crate::Circle3::try_new(p(2., 2., 0.), 0.5, normal, tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let source = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance()).unwrap();
    let before = source.clone();
    let target = warped(source.faces()[0].surface());
    let result = source
        .try_with_edited_single_surface(target, tolerance())
        .unwrap();
    assert_eq!(source, before);
    assert_eq!(result.faces()[0].loops().len(), 2);
    assert_eq!(
        result.faces()[0].loops()[1].loop_type(),
        BrepLoopType::Inner
    );
    assert_images(&result);
}
#[test]
fn unsupported_certificate_weights_reject_the_complete_edit_without_mutating_sources() {
    let source = triangle();
    let before = source.clone();
    let mut controls = source.faces()[0].surface().control_points().to_vec();
    controls[0] = WeightedPoint3::try_new(controls[0].point(), -0.01).unwrap();
    let s = source.faces()[0].surface();
    let target = NurbsSurface::try_new_rational(
        s.degree_u(),
        s.degree_v(),
        s.control_point_count_u(),
        s.control_point_count_v(),
        controls,
        s.knots_u().to_vec(),
        s.knots_v().to_vec(),
    )
    .unwrap();
    assert!(
        source
            .try_with_edited_single_surface(target, tolerance())
            .is_err()
    );
    assert_eq!(source, before);
}

#[test]
fn edited_sphere_cap_keeps_shared_seam_uses_and_exact_pole_collapse() {
    let frame = Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance(),
    )
    .unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
    let source = Brep::try_rectangular_surface_face(
        sphere.clone(),
        sphere.domain_u(),
        sphere.parameter_at_v(0.7).unwrap()..=*sphere.domain_v().end(),
        tolerance(),
    )
    .unwrap();
    let before = source.clone();
    let mut controls = sphere.control_points().to_vec();
    for c in &mut controls {
        let a = c.point().to_array();
        *c = WeightedPoint3::try_new(p(a[0], a[1], a[2] + 0.125 * (4. - a[2] * a[2])), c.weight())
            .unwrap();
    }
    let target = NurbsSurface::try_new_rational(
        sphere.degree_u(),
        sphere.degree_v(),
        sphere.control_point_count_u(),
        sphere.control_point_count_v(),
        controls,
        sphere.knots_u().to_vec(),
        sphere.knots_v().to_vec(),
    )
    .unwrap();
    let result = source
        .try_with_edited_single_surface(target, tolerance())
        .unwrap();
    assert_eq!(source, before);
    let trims = result.faces()[0].loops()[0].trims();
    let poles = trims
        .iter()
        .filter(|t| t.trim_type() == BrepTrimType::Singular)
        .collect::<Vec<_>>();
    assert_eq!(poles.len(), 1);
    assert!(poles[0].edge().is_none());
    assert_eq!(poles[0].vertices()[0], poles[0].vertices()[1]);
    let seams = trims
        .iter()
        .filter(|t| t.trim_type() == BrepTrimType::Seam)
        .collect::<Vec<_>>();
    assert_eq!(seams.len(), 2);
    assert_eq!(seams[0].edge(), seams[1].edge());
    assert_images(&result);
}

#[test]
fn edited_boundary_never_accepts_an_excursion_hidden_at_old_fit_stations() {
    let source = triangle();
    let before = source.clone();
    let s = source.faces()[0].surface();
    let excursion = crate::surface_pullback::certified_tests::station_excursion_surface()
        .try_reparameterized(s.domain_u(), s.domain_v())
        .unwrap();
    // Along the diagonal, every old uniform validation station has z=0.
    // A point strictly between them supplies an independent nonzero witness.
    assert!(
        excursion
            .evaluate(
                excursion.parameter_at_u(1. / 32.).unwrap(),
                excursion.parameter_at_v(1. / 32.).unwrap()
            )
            .unwrap()
            .z()
            .abs()
            > 1e-5
    );
    match source.try_with_edited_single_surface(excursion, tolerance()) {
        Ok(result) => {
            assert_images(&result);
            assert!(
                result
                    .edges()
                    .iter()
                    .flat_map(|e| e.curve().control_points())
                    .any(|c| c.point().z().abs() > 1e-5)
            );
        }
        Err(
            GeometryError::SurfaceCurveCertificateWorkLimit
            | GeometryError::SurfacePushupDidNotConverge { .. }
            | GeometryError::TooManySurfacePushupControlPoints { .. },
        ) => {}
        Err(error) => panic!("unexpected edit failure: {error:?}"),
    }
    assert_eq!(source, before);
}
