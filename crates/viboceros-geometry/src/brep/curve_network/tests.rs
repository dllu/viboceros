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
