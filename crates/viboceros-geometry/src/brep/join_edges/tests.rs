use super::*;

fn cube() -> Brep {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], tolerance).unwrap()
}

#[test]
fn automatic_assembly_cannot_reset_its_callers_exhausted_work_budget() {
    let original = cube();
    let source = Brep::try_combine(
        (0..6)
            .map(|i| original.sub_brep(&[i], Tolerance::DEFAULT).unwrap())
            .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let snapshot = source.clone();
    let pairs = pairs(&source);
    let error = source
        .join_edge_pairs_with_budget(&pairs, 0., Tolerance::DEFAULT, &mut Budget(0))
        .unwrap_err();
    assert_eq!(error, invalid("B-rep join work budget exceeded"));
    assert_eq!(source, snapshot);
    assert!(
        source
            .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
            .unwrap()
            .is_solid()
    );
}

// Test fixture correspondence, not a production automatic matcher.
fn pairs(source: &Brep) -> Vec<(usize, usize, bool)> {
    let counts = source.edge_use_counts();
    let mut used = vec![false; source.edges.len()];
    let mut result = Vec::new();
    for a in 0..source.edges.len() {
        if used[a] || counts[a] != 1 {
            continue;
        }
        for b in a + 1..source.edges.len() {
            if used[b] || counts[b] != 1 {
                continue;
            }
            if let Some(reversed) = [false, true].into_iter().find(|&r| {
                certificate::curve_bound(&source.edges[a].curve, &source.edges[b].curve, r, 0.)
                    .is_some()
            }) {
                used[a] = true;
                used[b] = true;
                result.push((a, b, reversed));
                break;
            }
        }
    }
    result
}

#[test]
fn six_faces_reassemble_for_all_input_orientation_masks() {
    let original = cube();
    let sheets = (0..6)
        .map(|i| original.sub_brep(&[i], Tolerance::DEFAULT).unwrap())
        .collect::<Vec<_>>();
    for mask in 0..64 {
        let parts = sheets
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if mask & (1 << i) != 0 {
                    s.reversed()
                } else {
                    s.clone()
                }
            })
            .collect();
        let source = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
        let snapshot = source.clone();
        let pairs = pairs(&source);
        assert_eq!(pairs.len(), 12);
        let joined = source
            .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(source, snapshot);
        assert_eq!(
            (
                joined.vertices.len(),
                joined.edges.len(),
                joined.faces.len()
            ),
            (8, 12, 6)
        );
        assert!(joined.is_solid());
        let expected = if mask & 1 == 0 { 30. } else { -30. };
        assert!((joined.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-12);
        assert!((joined.area(Tolerance::DEFAULT).unwrap() - 62.).abs() < 1e-12);
        for (actual, old) in joined.faces.iter().zip(&source.faces) {
            assert_eq!(actual.surface, old.surface);
            for (actual, old) in actual.loops.iter().zip(&old.loops) {
                for (actual, old) in actual.trims.iter().zip(&old.trims) {
                    assert_eq!(actual.curve, old.curve);
                    assert_eq!(actual.tolerance, old.tolerance);
                    assert_eq!(actual.iso, old.iso);
                }
            }
        }
        for &(retained, removed, _) in &pairs {
            assert!(
                joined
                    .edges
                    .iter()
                    .any(|e| e.curve == source.edges[retained].curve)
            );
            assert_eq!(source.edges[removed].tolerance, 0.);
        }
    }
}

#[test]
fn retained_edge_can_follow_removed_edge_and_pairs_are_order_independent() {
    let original = cube();
    let source = Brep::try_combine(
        (0..6)
            .map(|i| original.sub_brep(&[i], Tolerance::DEFAULT).unwrap())
            .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let pairs = pairs(&source);
    let reversed = pairs.iter().rev().copied().collect::<Vec<_>>();
    assert_eq!(
        source
            .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
            .unwrap(),
        source
            .try_join_edge_pairs(&reversed, 0., Tolerance::DEFAULT)
            .unwrap()
    );
    let pairs = pairs
        .into_iter()
        .map(|(a, b, r)| (b, a, r))
        .collect::<Vec<_>>();
    let joined = source
        .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
        .unwrap();
    assert!(joined.is_solid());
    assert!((joined.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-12);
}

#[test]
fn later_retained_reversed_edges_keep_their_own_domains_weights_and_controls() {
    let first = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let mut later = first.clone();
    for edge in &mut later.edges {
        let reversed = edge.curve.reversed().unwrap();
        edge.curve = NurbsCurve::try_new_rational(
            reversed.degree(),
            reversed
                .control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * 2.).unwrap())
                .collect(),
            reversed.knots().iter().map(|k| 1e9 + k * 8.).collect(),
        )
        .unwrap();
        edge.vertices.swap(0, 1);
    }
    for trim in later
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
    {
        trim.reversed_3d = !trim.reversed_3d;
    }
    later.validate(Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(vec![first, later.clone()], Tolerance::DEFAULT).unwrap();
    let requested = (0..4).map(|i| (i + 4, i, true)).collect::<Vec<_>>();
    let joined = source
        .try_join_edge_pairs(&requested, 0., Tolerance::DEFAULT)
        .unwrap();
    assert!(joined.is_solid());
    assert_eq!(joined.vertices, source.vertices[..4]);
    for (actual, expected) in joined.edges.iter().zip(&later.edges) {
        assert_eq!(actual.curve, expected.curve);
    }
    for (actual, expected) in joined.faces.iter().zip(&source.faces) {
        for (actual, expected) in actual.loops.iter().zip(&expected.loops) {
            for (actual, expected) in actual.trims.iter().zip(&expected.trims) {
                assert_eq!(actual.curve, expected.curve);
            }
        }
    }
}

#[test]
fn rejects_bad_pairs_and_invalid_distances_without_mutation() {
    let original = cube();
    let source = Brep::try_combine(
        vec![original.sub_brep(&[0], Tolerance::DEFAULT).unwrap(); 2],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let snapshot = source.clone();
    for pairs in [
        vec![(0, 0, false)],
        vec![(0, usize::MAX, false)],
        vec![(0, 4, false), (0, 5, false)],
        vec![(0, 4, true)],
        vec![(0, 5, false)],
    ] {
        assert!(
            source
                .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
                .is_err()
        );
    }
    for distance in [-1., Real::NAN, Real::INFINITY] {
        assert!(
            source
                .try_join_edge_pairs(&[], distance, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert!(
        original
            .try_join_edge_pairs(&[(0, 1, false)], 0., Tolerance::DEFAULT)
            .is_err()
    );
    assert_eq!(
        source
            .try_join_edge_pairs(&[], 0., Tolerance::DEFAULT)
            .unwrap(),
        source
    );
    assert_eq!(source, snapshot);
    assert!(
        source
            .try_join_edge_pairs(
                &vec![(0, 4, false); MAX_JOIN_PAIRS + 1],
                0.,
                Tolerance::DEFAULT
            )
            .is_err()
    );
}

#[test]
fn duplicate_sheets_are_oriented_but_not_reinterpreted_as_a_volume() {
    let sheet = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(vec![sheet.clone(), sheet], Tolerance::DEFAULT).unwrap();
    let joined = source
        .try_join_edge_pairs(&pairs(&source), 0., Tolerance::DEFAULT)
        .unwrap();
    assert_eq!((joined.vertices.len(), joined.edges.len()), (4, 4));
    assert!(joined.is_solid()); // Topologically solid, zero signed volume.
    assert_eq!(joined.signed_volume(Tolerance::DEFAULT).unwrap(), 0.);
    assert_ne!(joined.faces[0].reversed, joined.faces[1].reversed);
}

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn gap_join_retains_geometry_and_propagates_uncertainty_without_widening_acceptance() {
    let a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let b = a
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(0., 0., 0.001).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    let mut source = Brep::try_combine(vec![a, b], Tolerance::DEFAULT).unwrap();
    let pairs = (0..4).map(|i| (i, i + 4, false)).collect::<Vec<_>>();
    for old_tolerance in [0., 0.1] {
        for v in &mut source.vertices {
            v.tolerance = old_tolerance;
        }
        for e in &mut source.edges {
            e.tolerance = old_tolerance;
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let snapshot = source.clone();
        let joined = source
            .try_join_edge_pairs(&pairs, 0.001, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(source, snapshot);
        assert_eq!(
            joined.vertices.iter().map(|v| v.point).collect::<Vec<_>>(),
            source.vertices[..4]
                .iter()
                .map(|v| v.point)
                .collect::<Vec<_>>()
        );
        let expected = old_tolerance.max(Tolerance::DEFAULT.absolute()) + 0.001;
        for v in &joined.vertices {
            assert!(v.tolerance >= expected);
        }
        for (e, old) in joined.edges.iter().zip(&source.edges) {
            assert_eq!(e.curve, old.curve);
            assert!(e.tolerance >= expected);
        }
        for (f, old) in joined.faces.iter().zip(&source.faces) {
            assert_eq!(f.surface, old.surface);
        }
        assert!(
            source
                .try_join_edge_pairs(&pairs, (0.001_f64).next_down(), Tolerance::DEFAULT)
                .is_err()
        );
    }
}

#[test]
fn split_partial_overlap_can_be_joined_without_changing_either_surface() {
    let bottom = cube()
        .sub_brep(&[0], Tolerance::DEFAULT)
        .unwrap()
        .try_split_edges_at_parameters(&[(0, vec![0.5, 1.5])], Tolerance::DEFAULT)
        .unwrap();
    let side = Brep::try_box(
        frame(),
        [[0.5, 1.5], [0., 3.], [0., 5.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .sub_brep(&[2], Tolerance::DEFAULT)
    .unwrap();
    let source = Brep::try_combine(vec![bottom, side], Tolerance::DEFAULT).unwrap();
    let pairs = pairs(&source);
    assert_eq!(pairs.len(), 1);
    let joined = source
        .try_join_edge_pairs(&pairs, 0., Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(
        (
            joined.vertices.len(),
            joined.edges.len(),
            joined.faces.len()
        ),
        (8, 9, 2)
    );
    assert_eq!(
        joined.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        1
    );
    assert_eq!(joined.faces[0].surface, source.faces[0].surface);
    assert_eq!(joined.faces[1].surface, source.faces[1].surface);
    assert!((joined.area(Tolerance::DEFAULT).unwrap() - 11.).abs() < 1e-12);
}

#[test]
fn curved_rational_boundaries_seams_and_singular_vertices_reassemble() {
    for original in [
        Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap(),
        Brep::try_cone(frame(), 2., 5., Tolerance::DEFAULT).unwrap(),
    ] {
        let parts = (0..original.faces.len())
            .map(|i| original.sub_brep(&[i], Tolerance::DEFAULT).unwrap())
            .collect();
        let source = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
        let joined = source
            .try_join_edge_pairs(&pairs(&source), 0., Tolerance::DEFAULT)
            .unwrap();
        assert!(joined.is_solid());
        assert_eq!(joined.vertices.len(), original.vertices.len());
        assert_eq!(joined.edges.len(), original.edges.len());
        assert!(
            (joined.signed_volume(Tolerance::DEFAULT).unwrap()
                - original.signed_volume(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-10
        );
        // Open an existing same-face seam by giving its second trim an exact
        // independent spatial edge. Rejoining must restore Seam, not Mated.
        let mut opened = original.clone();
        let seam = opened
            .faces
            .iter()
            .flat_map(|f| &f.loops)
            .flat_map(|l| &l.trims)
            .find(|t| t.trim_type == BrepTrimType::Seam)
            .unwrap()
            .edge
            .unwrap();
        let replacement = opened.edges.len();
        opened.edges.push(opened.edges[seam].clone());
        let mut first = true;
        for t in opened
            .faces
            .iter_mut()
            .flat_map(|f| &mut f.loops)
            .flat_map(|l| &mut l.trims)
            .filter(|t| t.edge == Some(seam))
        {
            t.trim_type = BrepTrimType::Boundary;
            if first {
                first = false;
            } else {
                t.edge = Some(replacement);
            }
        }
        opened.validate(Tolerance::DEFAULT).unwrap();
        let restored = opened
            .try_join_edge_pairs(&[(seam, replacement, false)], 0., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(restored, original);
    }
}

#[test]
fn independently_parameterized_uv_trims_remain_bit_for_bit_unchanged() {
    let mut a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    for t in a
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
    {
        let start = t.curve.start_point().unwrap();
        let end = t.curve.end_point().unwrap();
        t.curve = NurbsCurve2::try_new_rational(
            1,
            vec![
                WeightedPoint2::try_new(start, 1.).unwrap(),
                WeightedPoint2::try_new(end, 9.).unwrap(),
            ],
            vec![1e9, 1e9, 1e9 + 128., 1e9 + 128.],
        )
        .unwrap();
    }
    a.validate(Tolerance::DEFAULT).unwrap();
    let b = cube().sub_brep(&[2], Tolerance::DEFAULT).unwrap();
    let source = Brep::try_combine(vec![a, b], Tolerance::DEFAULT).unwrap();
    let joined = source
        .try_join_edge_pairs(&pairs(&source), 0., Tolerance::DEFAULT)
        .unwrap();
    for (f, old) in joined.faces.iter().zip(&source.faces) {
        assert_eq!(f.surface, old.surface);
        for (l, old) in f.loops.iter().zip(&old.loops) {
            for (t, old) in l.trims.iter().zip(&old.trims) {
                assert_eq!(t.curve, old.curve);
            }
        }
    }
}

#[test]
fn transitive_vertex_cluster_must_fit_the_declared_join_distance() {
    // Three sheets form a chain of individually permissible boundary joins,
    // but the shared corner of sheet 3 is too far from retained sheet 1.
    let a = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    let mut sheets = Vec::new();
    for height in [0., 0.00075, 0.0015] {
        sheets.push(
            a.transformed(
                AffineTransform3::from_translation(Vector3::try_new(0., 0., height).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    let source = Brep::try_combine(sheets, Tolerance::DEFAULT).unwrap();
    let error = source
        .try_join_edge_pairs(&[(0, 4, false), (5, 9, false)], 0.001, Tolerance::DEFAULT)
        .unwrap_err();
    assert_eq!(
        error,
        invalid("joined vertex cluster exceeds the join distance")
    );
}

#[test]
fn nonmanifold_sources_are_rejected_even_when_no_edges_are_requested() {
    let vertices = [
        [0., 0., 0.],
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [0., -1., 0.],
    ]
    .map(|p| Point3::try_from(p).unwrap())
    .to_vec();
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = Brep::try_from_mesh(&mesh, true, Tolerance::DEFAULT).unwrap();
    assert!(!source.is_manifold());
    assert_eq!(
        source
            .try_join_edge_pairs(&[], 0., Tolerance::DEFAULT)
            .unwrap_err(),
        invalid("edge joining requires manifold input")
    );
}
