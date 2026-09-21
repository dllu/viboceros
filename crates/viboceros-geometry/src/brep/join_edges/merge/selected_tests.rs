use super::*;
use tests::{cube, frame};

#[test]
fn limited_scopes_visit_original_ends_once_in_either_spatial_orientation() {
    use BrepEdgeMergeScope::{Both, Chain, End, Start};
    let original = cube();
    let source = original
        .try_split_edges_at_parameters(&[(0, vec![0.25, 0.75, 1.75])], Tolerance::DEFAULT)
        .unwrap();
    // The splitter appends pieces from the far end; this is the middle piece.
    let seed = original.edges.len() + 1;
    for reversed in [false, true] {
        let mut source = source.clone();
        if reversed {
            let edge = &mut source.edges[seed];
            edge.vertices.reverse();
            edge.curve = edge.curve.reversed().unwrap();
            for face in &mut source.faces {
                for boundary in &mut face.loops {
                    for trim in &mut boundary.trims {
                        if trim.edge == Some(seed) {
                            trim.reversed_3d = !trim.reversed_3d;
                        }
                    }
                }
            }
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let before = source.clone();
        for (scope, removed, low, high) in [
            (Start, 1, 0.25, 1.75),
            (End, 1, 0.75, 2.),
            (Both, 2, 0.25, 2.),
            (Chain, 3, 0., 2.),
        ] {
            let scope = if reversed {
                match scope {
                    Start => End,
                    End => Start,
                    other => other,
                }
            } else {
                scope
            };
            let result = source
                .try_merge_edge_with_scope(seed, scope, 1e-10, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(source, before);
            assert_eq!(result.edges.len(), source.edges.len() - removed);
            assert_eq!(result.vertices.len(), source.vertices.len() - removed);
            assert!(result.is_solid());
            let edge = result.edges.last().unwrap();
            let ends = if reversed { [high, low] } else { [low, high] };
            for (vertex, x) in edge.vertices.into_iter().zip(ends) {
                assert_eq!(
                    result.vertices[vertex].point,
                    Point3::try_new(x, 0., 0.).unwrap()
                );
            }
            for (a, b) in result.faces.iter().zip(&source.faces) {
                assert_eq!(a.surface, b.surface);
                assert_eq!(a.reversed, b.reversed);
            }
            assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-10);
        }
    }
}

#[test]
fn limited_seam_scopes_update_both_uses_and_stop_before_new_endpoints() {
    use BrepEdgeMergeScope::{Both, Chain, End, Start};
    let original = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    for edge in 0..original.edges.len() {
        let cuts =
            [0.125, 0.375, 0.875].map(|t| original.edges[edge].curve.parameter_at(t).unwrap());
        let source = original
            .try_split_edges_at_parameters(&[(edge, cuts.to_vec())], Tolerance::DEFAULT)
            .unwrap();
        let seed = original.edges.len() + 1;
        for (scope, removed) in [(Start, 1), (End, 1), (Both, 2), (Chain, 3)] {
            let result = source
                .try_merge_edge_with_scope(seed, scope, 1e-10, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(result.edges.len(), source.edges.len() - removed);
            assert_eq!(result.edge_use_counts(), vec![2; result.edges.len()]);
            assert!(result.is_solid());
            assert!(
                (result.signed_volume(Tolerance::DEFAULT).unwrap()
                    - original.signed_volume(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-10
            );
        }
    }
}

#[test]
fn limited_scopes_skip_branches_without_advancing_to_other_neighbors() {
    use BrepEdgeMergeScope::{Both, Chain, End, Start};
    let original = cube();
    let source = original
        .try_split_edges_at_parameters(&[(0, vec![0.25, 0.75, 1.75])], Tolerance::DEFAULT)
        .unwrap();
    for scope in [Start, End, Both, Chain] {
        let blocked = source
            .try_merge_edge_with_scope(2, scope, 1e-10, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(blocked, source);
        for edge in [source.edges.len(), usize::MAX] {
            assert!(
                source
                    .try_merge_edge_with_scope(edge, scope, 1e-10, Tolerance::DEFAULT)
                    .is_err()
            );
        }
        for angle in [
            -1.,
            Real::NAN,
            Real::INFINITY,
            std::f64::consts::PI.next_up(),
        ] {
            assert!(
                source
                    .try_merge_edge_with_scope(0, scope, angle, Tolerance::DEFAULT)
                    .is_err()
            );
        }
        for work in [0, 16, 100, 1000] {
            assert!(
                merge_with_cleanup(
                    &source,
                    1e-10,
                    Tolerance::DEFAULT,
                    &mut Budget(work),
                    false,
                    Some((original.edges.len() + 1, scope))
                )
                .is_err()
            );
        }
    }
    assert_eq!(
        source
            .try_merge_edge_with_scope(0, Start, 1e-10, Tolerance::DEFAULT)
            .unwrap(),
        source
    );
    for scope in [End, Both] {
        let result = source
            .try_merge_edge_with_scope(0, scope, 1e-10, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(result.edges.len(), source.edges.len() - 1);
    }
}

#[test]
fn every_seed_reaches_both_ends_but_leaves_other_split_chains_untouched() {
    for original in [cube(), cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap()] {
        let cuts = [0, 2].map(|e| {
            (
                e,
                [0.125, 0.375, 0.875]
                    .map(|t| original.edges[e].curve.parameter_at(t).unwrap())
                    .to_vec(),
            )
        });
        let source = original
            .try_split_edges_at_parameters(&cuts, Tolerance::DEFAULT)
            .unwrap();
        let before = source.clone();
        let n = original.edges.len();
        let chain = [0, n, n + 1, n + 2];
        for seed in chain {
            let result = source
                .try_merge_edge(seed, 1e-10, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(result.edges.len(), source.edges.len() - 3);
            assert_eq!(result.vertices.len(), source.vertices.len() - 3);
            assert_eq!(source, before);
            assert_eq!(result.is_solid(), original.is_solid());
            let untouched = source
                .edges
                .iter()
                .enumerate()
                .filter(|(i, _)| !chain.contains(i));
            for ((_, a), b) in untouched.zip(&result.edges) {
                assert_eq!(a.curve, b.curve);
                assert_eq!(a.tolerance, b.tolerance);
                for end in 0..2 {
                    assert_eq!(
                        source.vertices[a.vertices[end]],
                        result.vertices[b.vertices[end]]
                    );
                }
            }
            for (a, b) in result.faces.iter().zip(&source.faces) {
                assert_eq!(a.surface, b.surface);
                assert_eq!(a.reversed, b.reversed);
            }
            assert!(
                (result.area(Tolerance::DEFAULT).unwrap()
                    - original.area(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-10
            );
            // The untouched chain remains removable by the all-edges operation.
            assert_eq!(
                result
                    .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
                    .unwrap()
                    .edges
                    .len(),
                original.edges.len()
            );
            let last = result.edges.len() - 1;
            assert_eq!(
                result
                    .try_merge_edge(last, 1e-10, Tolerance::DEFAULT)
                    .unwrap(),
                result
            );
        }
    }
}

#[test]
fn selected_closed_and_seam_chains_preserve_both_incident_faces() {
    let original = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    let n = original.edges.len();
    for edge in 0..n {
        let cuts =
            [0.125, 0.375, 0.875].map(|t| original.edges[edge].curve.parameter_at(t).unwrap());
        let source = original
            .try_split_edges_at_parameters(&[(edge, cuts.to_vec())], Tolerance::DEFAULT)
            .unwrap();
        for seed in [edge, n, n + 1, n + 2] {
            let result = source
                .try_merge_edge(seed, 1e-10, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(result.edges.len(), n);
            assert_eq!(result.vertices, original.vertices);
            assert!(result.is_solid());
            assert_eq!(result.edge_use_counts(), vec![2; n]);
            assert!(
                (result.signed_volume(Tolerance::DEFAULT).unwrap()
                    - original.signed_volume(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-10
            );
        }
    }
}

#[test]
fn bad_seeds_angles_and_exhausted_selected_work_leave_the_source_intact() {
    let original = cube();
    let source = original
        .try_split_edges_at_parameters(&[(0, vec![0.25, 0.5, 0.75])], Tolerance::DEFAULT)
        .unwrap();
    let before = source.clone();
    for seed in [source.edges.len(), usize::MAX] {
        assert!(matches!(
            source.try_merge_edge(seed, 1e-10, Tolerance::DEFAULT),
            Err(GeometryError::InvalidBrepTopology {
                context: "edge merge references a missing edge"
            })
        ));
    }
    for angle in [
        -1.,
        Real::NAN,
        Real::INFINITY,
        std::f64::consts::PI.next_up(),
    ] {
        assert!(source.try_merge_edge(0, angle, Tolerance::DEFAULT).is_err());
    }
    for work in [0, 16, 100, 1000] {
        assert!(
            merge_with_cleanup(
                &source,
                1e-10,
                Tolerance::DEFAULT,
                &mut Budget(work),
                false,
                Some((0, BrepEdgeMergeScope::Chain))
            )
            .is_err()
        );
    }
    // An untouched box corner is a branch, so no other chain is cleaned.
    assert_eq!(
        source.try_merge_edge(2, 1e-10, Tolerance::DEFAULT).unwrap(),
        source
    );
    assert_eq!(source, before);
}
