use super::*;

fn quad(x: f64) -> TriangleMesh {
    TriangleMesh::try_new_faces(
        [[x, 0., 0.], [x + 2., 0., 0.], [x + 2., 2., 0.], [x, 2., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
}
fn options(disjoint: bool, tolerance: f64) -> MeshJoinOptions {
    MeshJoinOptions {
        join_disjoint: disjoint,
        alignment_tolerance: tolerance,
        single_precision_matching: false,
    }
}

#[test]
fn joining_preserves_ngons_through_alignment_and_vertex_compaction() {
    let first = TriangleMesh::try_new(
        [
            [-10., 0., 0.],
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![[1, 2, 3], [1, 3, 4]],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(vec![1, 2, 3, 4], vec![0, 1])])
    .unwrap();
    let second = quad(2.);
    for disjoint in [false, true] {
        let joined = join_meshes(&[&first, &second], options(disjoint, 0.))
            .unwrap()
            .remove(0)
            .mesh;
        assert_eq!(joined.ngons().len(), 1);
        assert_eq!(joined.ngons()[0].faces(), &[0, 1]);
        assert_eq!(
            joined.ngons()[0].vertices(),
            if disjoint {
                &[1, 2, 3, 4]
            } else {
                &[0, 1, 2, 3]
            }
        );
        assert!(
            joined
                .clone()
                .try_with_ngons(joined.ngons().to_vec())
                .is_ok()
        );
    }
}

#[test]
fn direct_anchor_matches_do_not_take_transitive_closure_or_exceed_tolerance() {
    let meshes = [quad(0.), quad(0.125), quad(0.25), quad(0.375)];
    let references = meshes.iter().collect::<Vec<_>>();
    let joined = join_meshes(&references, options(true, 0.15))
        .unwrap()
        .remove(0)
        .mesh;
    assert_eq!(joined.vertices()[0].x(), 0.);
    assert_eq!(joined.vertices()[4].x(), 0.25);
    assert_eq!(joined.vertices()[8].x(), 0.25);
    assert_eq!(joined.vertices()[12].x(), 0.25);
    for (before, after) in meshes
        .iter()
        .flat_map(|m| m.vertices())
        .zip(joined.vertices())
    {
        assert!(before.distance_to(*after).unwrap() <= 0.15);
    }
    assert_eq!(joined.vertices().len(), 16);
    assert_eq!(joined.faces().len(), 4);
}

#[test]
fn float_matching_is_explicit_and_retains_exact_anchor_coordinates() {
    let meshes = [quad(1e6), quad(1e6 + 2.01)];
    let references = meshes.iter().collect::<Vec<_>>();
    let exact = join_meshes(&references, options(true, 0.))
        .unwrap()
        .remove(0)
        .mesh;
    assert_eq!(exact.vertices()[4], meshes[1].vertices()[0]);
    let mut policy = options(true, 0.);
    policy.single_precision_matching = true;
    let repaired = join_meshes(&references, policy).unwrap().remove(0).mesh;
    assert_eq!(repaired.vertices()[4], meshes[0].vertices()[1]);
    assert_eq!(repaired.vertices()[5], meshes[1].vertices()[1]);
}

#[test]
fn shuffled_components_preserve_sources_and_orient_edges_without_welding() {
    let meshes = [quad(0.), quad(8.), quad(2.).reversed(), quad(10.)];
    let components = join_meshes(&meshes.iter().collect::<Vec<_>>(), options(false, 0.)).unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].source_indices, [0, 2]);
    assert_eq!(components[1].source_indices, [1, 3]);
    for component in &components {
        assert_eq!(component.mesh.faces().len(), 2);
        assert_eq!(component.mesh.vertices().len(), 8);
        assert_eq!(component.mesh.topology().orientation_conflict_edge_count, 0);
        assert_eq!(component.mesh.area().unwrap(), 8.);
    }
}

#[test]
fn invalid_policies_and_collapsed_aligned_faces_are_errors_without_source_edits() {
    let mesh = quad(0.);
    for tolerance in [-1., f64::INFINITY, f64::NAN] {
        assert!(matches!(
            join_meshes(&[&mesh], options(true, tolerance)),
            Err(GeometryError::InvalidMeshJoinTolerance)
        ));
    }
    let large = quad(1e8);
    let before = large.clone();
    let mut policy = options(true, 0.);
    policy.single_precision_matching = true;
    assert!(join_meshes(&[&mesh, &large], policy).is_err());
    assert_eq!(large, before);
    assert!(matches!(
        join_meshes(&vec![&mesh; MAX_INPUTS + 1], options(true, 0.)),
        Err(GeometryError::MeshJoinResourceLimit)
    ));
    assert!(join_meshes(&[], options(false, 0.)).unwrap().is_empty());
}

#[test]
fn thousands_of_disconnected_meshes_are_each_emitted_once_in_source_order() {
    let meshes = (0..2000)
        .map(|i| quad(f64::from(i) * 4.))
        .collect::<Vec<_>>();
    let result = join_meshes(&meshes.iter().collect::<Vec<_>>(), options(false, 0.)).unwrap();
    assert_eq!(result.len(), meshes.len());
    for (i, component) in result.iter().enumerate() {
        assert_eq!(component.source_indices, [i]);
        assert_eq!(component.mesh, meshes[i]);
    }
}

#[test]
fn spatial_alignment_matches_an_independent_literal_anchor_scan() {
    let mut state = 4321_u64;
    for _ in 0..256 {
        let meshes = (0..6)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                quad(((state >> 32) % 32) as f64 / 64.)
            })
            .collect::<Vec<_>>();
        let points = meshes
            .iter()
            .flat_map(|m| m.vertices().iter().copied())
            .collect::<Vec<_>>();
        let mut target = (0..points.len()).collect::<Vec<_>>();
        for anchor in 0..points.len() {
            if target[anchor] != anchor {
                continue;
            }
            for (neighbor, &point) in points.iter().enumerate() {
                if points[anchor].distance_to(point).unwrap() <= 0.1 {
                    target[neighbor] = anchor;
                }
            }
        }
        let expected = target.into_iter().map(|i| points[i]).collect::<Vec<_>>();
        let joined = join_meshes(&meshes.iter().collect::<Vec<_>>(), options(true, 0.1))
            .unwrap()
            .remove(0)
            .mesh;
        assert_eq!(joined.vertices(), expected);
    }
}
