use super::*;

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn plane() -> NurbsSurface {
    NurbsSurface::try_bilinear([p(-10., -10.), p(10., -10.), p(10., 10.), p(-10., 10.)])
        .unwrap()
        .try_reparameterized(-10. ..=10., -10. ..=10.)
        .unwrap()
}
fn line(a: Point3, b: Point3) -> NurbsCurve {
    NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap()
}
fn network(curves: Vec<NurbsCurve>) -> Result<Vec<Brep>, GeometryError> {
    Brep::try_split_rectangular_surface_face_with_curve_network(
        plane(),
        -10. ..=10.,
        -10. ..=10.,
        curves,
        false,
        Tolerance::DEFAULT,
    )
}
fn assert_partition(pieces: &[Brep], count: usize) {
    assert_eq!(pieces.len(), count);
    let mut area = 0.;
    for piece in pieces {
        assert_eq!(piece.faces().len(), 1);
        assert_eq!(piece.faces()[0].surface(), &plane());
        assert!(piece.edge_use_counts().iter().all(|&n| n == 1));
        area += piece.area(Tolerance::DEFAULT).unwrap();
    }
    assert!((area - 400.).abs() < 1e-8, "partition area: {area}");
}

#[test]
fn curve_network_splits_an_interior_t_junction_without_a_dangling_face() {
    let curves = vec![line(p(-10., 0.), p(10., 0.)), line(p(0., 0.), p(0., 10.))];
    assert!(
        Brep::try_split_rectangular_surface_face_with_curves(
            plane(),
            -10. ..=10.,
            -10. ..=10.,
            curves.clone(),
            false,
            Tolerance::DEFAULT,
        )
        .is_err()
    );
    assert_partition(&network(curves).unwrap(), 3);
}

#[test]
fn curve_network_splits_rational_loops_and_spokes_at_every_crossing() {
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = |radius| {
        let c = crate::Circle3::try_new(p(0., 0.), radius, normal, Tolerance::DEFAULT).unwrap();
        crate::CurveRef::Circle(&c).to_nurbs().unwrap()
    };
    let pieces = network(vec![
        circle(4.),
        circle(2.),
        line(p(4., 0.), p(0., 2.)),
        line(p(4., 0.), p(0., -2.)),
        line(p(4., 0.), p(-2., 0.)),
    ])
    .unwrap();
    assert_partition(&pieces, 8);
    for expected in [p(1.6, 1.2), p(1.6, -1.2)] {
        assert!(pieces.iter().any(|b| {
            b.vertices()
                .iter()
                .any(|v| v.point().distance_to(expected).unwrap() < 1e-8)
        }));
    }
}

#[test]
fn curve_network_rejects_unattached_and_dangling_ends() {
    for curves in [
        vec![line(p(-2., -2.), p(2., 2.))],
        vec![line(p(-10., 0.), p(10., 0.)), line(p(0., 0.), p(0., 2.))],
    ] {
        let error = network(curves).unwrap_err();
        assert!(error.to_string().contains("dangling endpoint"), "{error}");
    }
}

#[test]
fn curve_network_preserves_source_split_parameters_with_nonunit_weights() {
    let base = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(-10., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(10., 0.), 4.).unwrap(),
        ],
        vec![12., 12., 20., 20.],
    )
    .unwrap();
    let graph = build(
        &plane(),
        [[-10., 10.]; 2],
        vec![base, line(p(0., 0.), p(0., 10.))],
        Tolerance::DEFAULT,
        true,
        64,
        |_| Some(Tolerance::DEFAULT.angular()),
    )
    .unwrap();
    let domains = graph
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind,
                SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
            )
        })
        .map(|edge| edge.spatial.domain())
        .collect::<Vec<_>>();
    assert_eq!(domains.len(), 2);
    assert!((*domains[0].start() - 12.).abs() < 1e-10);
    assert!((*domains[0].end() - 13.6).abs() < 1e-10);
    assert!((*domains[1].start() - 13.6).abs() < 1e-10);
    assert!((*domains[1].end() - 20.).abs() < 1e-10);
}

#[test]
fn curve_network_retains_face_orientation_and_shared_junction_geometry() {
    for reverse_curves in [false, true] {
        let mut curves = vec![line(p(-10., 0.), p(10., 0.)), line(p(0., 0.), p(0., 10.))];
        if reverse_curves {
            curves.reverse();
            curves = curves
                .iter()
                .map(|curve| curve.reversed().unwrap())
                .collect();
        }
        let pieces = Brep::try_split_rectangular_surface_face_with_curve_network(
            plane(),
            -10. ..=10.,
            -10. ..=10.,
            curves,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_partition(&pieces, 3);
        for piece in pieces {
            assert!(piece.faces()[0].is_reversed());
            let junction = piece
                .vertices()
                .iter()
                .filter(|v| v.point().distance_to(p(0., 0.)).unwrap() < 1e-9)
                .count();
            assert_eq!(junction, 1);
        }
    }
}

#[test]
fn curve_network_bounds_input_collection_and_graph_growth() {
    use std::cell::Cell;
    let consumed = Cell::new(0usize);
    let curves = std::iter::repeat_with(|| {
        consumed.set(consumed.get() + 1);
        line(p(-10., 0.), p(10., 0.))
    });
    let error = Brep::try_split_rectangular_surface_face_with_curve_network(
        plane(),
        -10. ..=10.,
        -10. ..=10.,
        curves,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("storage estimate limit"),
        "{error}"
    );
    assert!(consumed.get() < crate::MAX_SURFACE_WIRES);

    let error = build(
        &plane(),
        [[-10., 10.]; 2],
        vec![line(p(-10., 0.), p(10., 0.)), line(p(0., 0.), p(0., 10.))],
        Tolerance::DEFAULT,
        true,
        5,
        |_| Some(Tolerance::DEFAULT.angular()),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("work limit"), "{error}");
}

#[test]
fn curve_network_hull_search_is_conservative_for_relative_tolerance_and_missing_hulls() {
    let mut index = index::HullIndex::default();
    let bounds = |x| line(p(x, 0.), p(x, 1.)).control_point_bounds();
    index.insert(Some(bounds(1e12)));
    index.insert(None);
    index.insert(Some(bounds(-1e12)));
    // The intersection solver uses max(absolute, relative * control scale).
    assert_eq!(
        index.candidates(Some(bounds(1e12 + 0.5)), Tolerance::DEFAULT),
        vec![0, 1]
    );
    assert_eq!(
        index.candidates(Some(bounds(0.)), Tolerance::DEFAULT),
        vec![1]
    );
    assert_eq!(index.candidates(None, Tolerance::DEFAULT), vec![0, 1, 2]);
    // Extreme boxes also take the conservative path, without overflowing the tree.
    index.insert(Some(bounds(1e100)));
    assert_eq!(
        index.candidates(Some(bounds(1e100)), Tolerance::DEFAULT),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn dense_radial_network_reuses_the_shared_endpoint_without_extra_contacts() {
    let mut rays = Vec::new();
    for side in 0..4 {
        for i in 0..64 {
            let t = -10. + 20. * i as Real / 64.;
            let end = match side {
                0 => p(t, -10.),
                1 => p(10., t),
                2 => p(-t, 10.),
                _ => p(-10., -t),
            };
            rays.push(line(p(0., 0.), end));
        }
    }
    let pieces = network(rays).unwrap();
    assert_partition(&pieces, 256);
    assert!(
        pieces
            .iter()
            .all(|piece| piece.vertices.len() == 3 && piece.edges.len() == 3)
    );
}

#[test]
fn shared_endpoint_certificate_keeps_collinear_near_parallel_and_rational_fallbacks() {
    let a = line(p(0., 0.), p(1., 0.));
    assert!(
        index::only_common_endpoint(&a, &line(p(0., 0.), p(1., 1.)), Tolerance::DEFAULT).unwrap()
    );
    assert!(
        index::only_common_endpoint(&a, &line(p(1., 1.), p(0., 0.)), Tolerance::DEFAULT).unwrap()
    );
    for b in [
        line(p(0., 0.), p(2., 0.)),
        line(p(0., 0.), p(-1., 0.)),
        line(p(0., 0.), p(1., 1e-12)),
        line(p(0., 1.), p(1., 2.)),
    ] {
        assert!(!index::only_common_endpoint(&a, &b, Tolerance::DEFAULT).unwrap());
    }
    let rational = |w| {
        NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 1.), w).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    assert!(index::only_common_endpoint(&a, &rational(4.), Tolerance::DEFAULT).unwrap());
    assert!(!index::only_common_endpoint(&a, &rational(-4.), Tolerance::DEFAULT).unwrap());
}

#[test]
fn caller_kink_policy_preserves_small_fitted_turns_without_extra_vertices() {
    let curve = NurbsCurve::try_new(
        2,
        vec![p(-10., 0.), p(-5., 0.), p(0., 0.), p(5., 1e-4), p(10., 0.)],
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    for (angle, expected_nodes, expected_edges) in [
        (Some(Tolerance::DEFAULT.angular()), 7, 2),
        (Some(std::f64::consts::PI / 180.), 6, 1),
        (None, 6, 1),
    ] {
        let graph = build(
            &plane(),
            [[-10., 10.]; 2],
            vec![curve.clone()],
            Tolerance::DEFAULT,
            true,
            64,
            |_| angle,
        )
        .unwrap();
        assert_eq!(graph.nodes.len(), expected_nodes);
        let cutters = graph
            .edges
            .iter()
            .filter(|e| matches!(e.kind, SurfaceCutArrangementEdgeKind::Cut { .. }))
            .collect::<Vec<_>>();
        assert_eq!(cutters.len(), expected_edges);
        assert_eq!(graph.regions.len(), 2);
        if expected_edges == 1 {
            assert_eq!(cutters[0].spatial, curve);
        }
        let pieces = graph
            .regions
            .iter()
            .map(|r| {
                try_surface_cut_arrangement_face(
                    plane(),
                    false,
                    &graph.nodes,
                    &graph.edges,
                    r,
                    &graph.source_info,
                    Tolerance::DEFAULT,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_partition(&pieces, 2);
    }
}
