use super::*;

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn cube() -> Brep {
    Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap()
}
fn face(bounds: [[Real; 2]; 3], i: usize) -> Brep {
    Brep::try_box(frame(), bounds, Tolerance::DEFAULT)
        .unwrap()
        .sub_brep(&[i], Tolerance::DEFAULT)
        .unwrap()
}

#[test]
fn automatic_box_assembly_is_outward_for_all_input_face_senses() {
    let b = cube();
    for mask in 0..64 {
        let sheets = (0..6)
            .map(|i| {
                let s = b.sub_brep(&[i], Tolerance::DEFAULT).unwrap();
                if mask & (1 << i) == 0 {
                    s
                } else {
                    s.reversed()
                }
            })
            .collect::<Vec<_>>();
        let before = sheets.clone();
        let result =
            join_breps(&sheets.iter().collect::<Vec<_>>(), 1e-9, Tolerance::DEFAULT).unwrap();
        assert_eq!(sheets, before);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(result[0].joined_edge_count, 12);
        assert!(result[0].brep.is_solid());
        assert!((result[0].brep.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-12);
    }
}

#[test]
fn partial_edges_split_automatically_and_retain_underlying_surfaces() {
    let a = face([[0., 2.], [0., 3.], [0., 5.]], 0);
    let b = face([[0.5, 1.5], [0., 3.], [0., 5.]], 2);
    let result = join_breps(&[&a, &b], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 1);
    let part = &result[0];
    assert_eq!(part.joined_edge_count, 1);
    assert_eq!(
        (
            part.brep.vertices.len(),
            part.brep.edges.len(),
            part.brep.faces.len()
        ),
        (8, 9, 2)
    );
    assert_eq!(part.brep.faces[0].surface, a.faces[0].surface);
    assert_eq!(part.brep.faces[1].surface, b.faces[0].surface);
    let shared = part
        .brep
        .edge_use_counts()
        .iter()
        .position(|&n| n == 2)
        .unwrap();
    assert_eq!(part.brep.edges[shared].curve, b.edges[0].curve);
    assert!((part.brep.area(Tolerance::DEFAULT).unwrap() - 11.).abs() < 1e-12);
}

#[test]
fn one_long_boundary_can_join_two_short_boundaries() {
    let a = face([[0., 2.], [0., 3.], [0., 5.]], 0);
    let b = face([[0., 1.], [0., 3.], [0., 5.]], 2);
    let c = face([[1., 2.], [0., 3.], [0., 5.]], 2);
    let result = join_breps(&[&a, &b, &c], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].joined_edge_count, 3);
    assert_eq!(
        (result[0].brep.vertices.len(), result[0].brep.edges.len()),
        (8, 10)
    );
    assert!((result[0].brep.area(Tolerance::DEFAULT).unwrap() - 16.).abs() < 1e-12);
}

#[test]
fn straight_edges_join_across_different_degrees_and_rational_speeds() {
    let a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let mut b = cube().sub_brep(&[2], Tolerance::DEFAULT).unwrap();
    b.edges[0].curve = NurbsCurve::try_new_rational(
        3,
        [0., 0.25, 0.75, 2.]
            .into_iter()
            .zip([1., 0.3, 2., 1.])
            .map(|(x, w)| WeightedPoint3::try_new(Point3::try_new(x, 0., 0.).unwrap(), w).unwrap())
            .collect(),
        vec![100., 100., 100., 100., 500., 500., 500., 500.],
    )
    .unwrap();
    b.validate(Tolerance::DEFAULT).unwrap();
    let result = join_breps(&[&a, &b], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].joined_edge_count, 1);
    assert_eq!(result[0].brep.edges[0].curve, a.edges[0].curve);
}

#[test]
fn components_preserve_source_order_and_split_disconnected_source_membership() {
    let a = cube().sub_brep(&[0, 1], Tolerance::DEFAULT).unwrap();
    let b = face([[0., 2.], [0., 3.], [0., 2.]], 2);
    let result = join_breps(&[&a, &b], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].source_indices, vec![0, 1]);
    assert_eq!(result[1].source_indices, vec![0]);
    assert_eq!(result[0].brep.faces.len(), 2);
    assert_eq!(result[1].brep.faces.len(), 1);
    assert_eq!(result[0].joined_edge_count, 1);
    assert_eq!(result[1].joined_edge_count, 0);
}

#[test]
fn vertex_only_contacts_and_crossing_edges_do_not_join() {
    let a = face([[0., 2.], [0., 3.], [0., 5.]], 0);
    let b = face([[2., 4.], [3., 6.], [0., 5.]], 0);
    let c = face([[1., 3.], [1., 4.], [0., 5.]], 0);
    for b in [b, c] {
        let result = join_breps(&[&a, &b], 1e-9, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].brep, a);
        assert_eq!(result[1].brep, b);
    }
}

#[test]
fn many_disconnected_sheets_use_bounded_search_and_compact_component_extraction() {
    let sheets = (0..1000)
        .map(|i| {
            face(
                [[i as Real * 4., i as Real * 4. + 2.], [0., 3.], [0., 5.]],
                0,
            )
        })
        .collect::<Vec<_>>();
    let result = join_breps(&sheets.iter().collect::<Vec<_>>(), 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), sheets.len());
    for (i, (actual, expected)) in result.iter().zip(&sheets).enumerate() {
        assert_eq!(actual.brep, *expected);
        assert_eq!(actual.source_indices, vec![i]);
    }
}

#[test]
fn bad_distances_work_limits_and_empty_inputs_are_explicit() {
    assert!(join_breps(&[], 0., Tolerance::DEFAULT).unwrap().is_empty());
    for distance in [-1., Real::NAN, Real::INFINITY] {
        assert!(join_breps(&[], distance, Tolerance::DEFAULT).is_err());
    }
    let b = cube();
    assert!(join_breps(&vec![&b; MAX_SOURCES + 1], 0., Tolerance::DEFAULT).is_err());
    let mut budget = Budget(0);
    assert!(budget.charge(1).is_err());
    let sheet = b.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    assert!(search::find(&sheet, 1., &mut budget).is_err());
}

#[test]
fn boundaries_within_an_original_source_are_not_implicitly_sewn() {
    let a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let b = cube().sub_brep(&[2], Tolerance::DEFAULT).unwrap();
    let combined = Brep::try_combine(vec![a.clone(), b.clone()], Tolerance::DEFAULT).unwrap();
    let parts = join_breps(&[&combined], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(parts.len(), 2);
    for (part, expected) in parts.iter().zip([a, b]) {
        assert_eq!(part.brep, expected);
        assert_eq!(part.source_indices, vec![0]);
        assert_eq!(part.joined_edge_count, 0);
    }
    let closed = cube().reversed();
    assert_eq!(
        join_breps(&[&closed], 0., Tolerance::DEFAULT).unwrap()[0].brep,
        closed
    );
}

#[test]
fn partial_overlap_inverts_rational_edge_speed_across_negative_and_shifted_domains() {
    for origin in [-500., -100., 1e9] {
        let mut a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
        a.edges[0].curve = NurbsCurve::try_new_rational(
            3,
            [0., 0.25, 0.75, 2.]
                .into_iter()
                .zip([1., 0.3, 2., 1.])
                .map(|(x, w)| {
                    WeightedPoint3::try_new(Point3::try_new(x, 0., 0.).unwrap(), w).unwrap()
                })
                .collect(),
            vec![
                origin,
                origin,
                origin,
                origin,
                origin + 400.,
                origin + 400.,
                origin + 400.,
                origin + 400.,
            ],
        )
        .unwrap();
        a.validate(Tolerance::DEFAULT).unwrap();
        let b = face([[0.5, 1.5], [0., 3.], [0., 5.]], 2);
        let parts = join_breps(&[&a, &b], 1e-9, Tolerance::DEFAULT).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].joined_edge_count, 1);
        let result = &parts[0].brep;
        assert_eq!((result.vertices.len(), result.edges.len()), (8, 9));
        assert_eq!(result.faces[0].surface, a.faces[0].surface);
        assert_eq!(result.faces[1].surface, b.faces[0].surface);
        assert!((result.area(Tolerance::DEFAULT).unwrap() - 11.).abs() < 1e-9);
    }
}

fn sheet(vector: [Real; 2]) -> Brep {
    sheet_at(vector, 0.)
}

fn sheet_at(vector: [Real; 2], z: Real) -> Brep {
    let surface = NurbsSurface::try_clamped_uniform(
        1,
        1,
        2,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|v| {
                [0., 2.]
                    .into_iter()
                    .map(move |x| Point3::try_new(x, v * vector[0], z + v * vector[1]).unwrap())
            })
            .collect(),
    )
    .unwrap();
    Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()
}

#[test]
fn competing_boundaries_are_not_arbitrarily_paired_by_input_order() {
    for planes in [
        [sheet([3., 0.]), sheet([0., 5.]), sheet([1., 1.])],
        [sheet([3., 0.]), sheet([-3., 0.]), sheet([0., 5.])],
    ] {
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let sources = order.map(|i| &planes[i]);
            let pieces = join_breps(&sources, 1e-9, Tolerance::DEFAULT).unwrap();
            assert_eq!(pieces.len(), 3);
            for (i, (piece, original)) in pieces.iter().zip(sources).enumerate() {
                assert_eq!(piece.brep, *original);
                assert_eq!(piece.source_indices, vec![i]);
                assert_eq!(piece.joined_edge_count, 0);
            }
        }
    }
}

#[test]
fn all_certified_candidates_participate_in_ambiguity_even_when_one_is_closer() {
    let a = sheet([3., 0.]);
    let b = sheet([0., 5.]);
    let c = sheet_at([1., 1.], 0.0005);
    let report = join_breps_with_report(&[&a, &b, &c], 0.002, Tolerance::DEFAULT).unwrap();
    assert_eq!(report.candidate_source_pairs, vec![[0, 1], [0, 2], [1, 2]]);
    assert_eq!(report.components.len(), 3);
    assert!(report.components.iter().all(|c| c.joined_edge_count == 0));
}

#[test]
fn other_unambiguous_edges_can_resolve_a_competing_boundary_within_one_component() {
    let a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let b = cube().sub_brep(&[2], Tolerance::DEFAULT).unwrap();
    let parts = join_breps(&[&a, &b, &b], 1e-9, Tolerance::DEFAULT).unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].brep, a);
    assert_eq!(parts[0].source_indices, vec![0]);
    assert_eq!(parts[1].source_indices, vec![1, 2]);
    assert_eq!(parts[1].joined_edge_count, 4);
    assert!(parts[1].brep.is_solid());
    assert_eq!(parts[1].brep.signed_volume(Tolerance::DEFAULT).unwrap(), 0.);
}
