use super::*;
use tests::{cube, frame};

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
                Some(0)
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
