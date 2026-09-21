use super::*;

fn sheet(y: [Real; 2], z: Real) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_clamped_uniform(
            1,
            1,
            2,
            2,
            y.into_iter()
                .flat_map(|y| [0., 4.].map(|x| Point3::try_new(x, y, z).unwrap()))
                .collect(),
        )
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn close_natural_boundaries_and_incident_edges_move_without_refitting_surfaces() {
    let a = sheet([0., 3.], 0.);
    let b = sheet([3.0005, 6.], 0.);
    let before = [a.clone(), b.clone()];
    let tol = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let joined = join_breps(&[&a, &b], 0.002, tol).unwrap();
    assert_eq!([a, b], before);
    assert_eq!(joined.len(), 1);
    let b = &joined[0].brep;
    assert_eq!(joined[0].joined_edge_count, 1);
    for (face, original) in b.faces.iter().zip(&before) {
        assert_eq!(face.surface, original.faces[0].surface);
        for (trim, old) in face.loops[0]
            .trims
            .iter()
            .zip(&original.faces[0].loops[0].trims)
        {
            assert_eq!(trim.curve, old.curve);
        }
    }
    let moved = b
        .vertices
        .iter()
        .filter(|v| v.point.y() > 3. && v.point.y() < 3.0005)
        .collect::<Vec<_>>();
    assert_eq!(moved.len(), 2);
    for vertex in moved {
        assert_eq!(vertex.point.y(), 3_f64.midpoint(3.0005));
        assert!((vertex.tolerance - 0.00025025).abs() < 1e-14);
    }
    assert_eq!(b.edges.iter().filter(|e| e.tolerance > 0.).count(), 5);
    for edge in &b.edges {
        assert!(edge.tolerance < 0.000251);
        for (end, &v) in edge.vertices.iter().enumerate() {
            let t = if end == 0 {
                *edge.curve.domain().start()
            } else {
                *edge.curve.domain().end()
            };
            assert_eq!(edge.curve.evaluate(t).unwrap(), b.vertices[v].point);
        }
    }
}

#[test]
fn exact_means_do_not_overflow_and_do_not_depend_on_input_order() {
    let point = |x| Point3::try_new(x, 0., 0.).unwrap();
    assert_eq!(
        mean([point(Real::MAX), point(Real::MAX)].into_iter()).unwrap(),
        point(Real::MAX)
    );
    for values in [
        [Real::MAX, -Real::MAX, 3.],
        [3., Real::MAX, -Real::MAX],
        [-Real::MAX, 3., Real::MAX],
    ] {
        assert_eq!(mean(values.into_iter().map(point)).unwrap(), point(1.));
    }
    assert_eq!(
        mean([point(Real::from_bits(1)), point(Real::from_bits(3))].into_iter()).unwrap(),
        point(Real::from_bits(2))
    );
}

#[test]
fn transitive_clusters_cannot_move_vertices_past_the_join_limit() {
    let sheets = (0..4)
        .map(|i| sheet([0., 3. + i as Real], i as Real * 0.0015))
        .collect::<Vec<_>>();
    let originals = sheets.clone();
    let tol = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    assert!(join_breps(&sheets.iter().collect::<Vec<_>>(), 0.002, tol).is_err());
    assert_eq!(sheets, originals);
}

#[test]
fn identical_geometry_is_a_noop_and_budget_exhaustion_is_atomic() {
    let a = sheet([0., 3.], 0.);
    let b = Brep::try_combine(vec![a.clone(), a.clone()], Tolerance::DEFAULT).unwrap();
    let candidates = vec![(0, a.vertices.len())];
    assert!(
        apply(
            &b,
            &candidates,
            0.,
            Tolerance::DEFAULT,
            &mut Budget(MAX_WORK)
        )
        .unwrap()
        .is_none()
    );
    assert!(apply(&b, &candidates, 0., Tolerance::DEFAULT, &mut Budget(0)).is_err());
}

#[test]
fn chord_adjustment_uses_spatial_projection_not_control_index_or_parameter() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let controls = [p(0., 0., 0.), p(1., 2., 0.), p(3., -2., 0.), p(4., 0., 0.)];
    let moved =
        crate::opennurbs_chord_adjust::adjust(&controls, [p(0., 0., 1.), p(4., 0., 3.)]).unwrap();
    assert_eq!(
        moved,
        [
            p(0., 0., 1.),
            p(1., 2., 1.5),
            p(3., -2., 2.5),
            p(4., 0., 3.)
        ]
    );
    let closed = [p(0., 0., 0.), p(1., 2., 0.), p(0., 0., 0.)];
    let moved = crate::opennurbs_chord_adjust::adjust(&closed, [p(0., 0., 1.); 2]).unwrap();
    assert_eq!(moved, [p(0., 0., 1.), p(1., 2., 1.), p(0., 0., 1.)]);
    let short = [p(0., 0., 0.), p(1., 2., 0.), p(0.001, 0., 0.)];
    let moved =
        crate::opennurbs_chord_adjust::adjust(&short, [p(0., 0., 1.), p(0.001, 0., 3.)]).unwrap();
    assert_eq!(moved[1], short[1]);
}

#[test]
fn joined_uncertainty_never_erases_original_component_tolerances() {
    let mut a = sheet([0., 3.], 0.);
    let b = sheet([3.0005, 6.], 0.);
    for edge in &mut a.edges {
        edge.tolerance = 0.1;
    }
    for vertex in &mut a.vertices {
        vertex.tolerance = 0.2;
    }
    let tol = Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap();
    let result = join_breps(&[&a, &b], 0.002, tol).unwrap();
    for &count in &result[0].brep.edge_use_counts() {
        assert!(count == 1 || count == 2);
    }
    let joined = result[0]
        .brep
        .edge_use_counts()
        .iter()
        .position(|&n| n == 2)
        .unwrap();
    assert_eq!(result[0].brep.edges[joined].tolerance, 0.1);
    assert!(
        result[0]
            .brep
            .vertices
            .iter()
            .filter(|v| v.tolerance == 0.2)
            .count()
            >= 4
    );
}
