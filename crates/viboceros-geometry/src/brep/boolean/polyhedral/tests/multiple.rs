use super::*;

#[test]
fn multiple_union_reports_contained_material_and_each_disconnected_contributor() {
    let a = Brep::try_disjoint_union(
        vec![cube([[0., 3.]; 3]), cube([[10., 11.]; 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = cube([[1.5, 3.5], [1.5, 3.5], [1., 2.]]);
    let c = cube([[0.75, 2.25]; 3]);
    let before = (a.clone(), b.clone(), c.clone());
    let result = union_polyhedral_breps(&[&a, &b, &c], Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 2);
    let main = result
        .iter()
        .find(|r| r.source_indices.contains(&1))
        .unwrap();
    assert_eq!(main.source_indices, vec![0, 1, 2]);
    assert_eq!(main.boundary_source_indices, vec![0, 1]);
    measure(Some(main.brep.clone()), 28.75, None);
    let separate = result
        .iter()
        .find(|r| !r.source_indices.contains(&1))
        .unwrap();
    assert_eq!(separate.source_indices, vec![0]);
    assert_eq!(separate.boundary_source_indices, vec![0]);
    measure(Some(separate.brep.clone()), 1., Some(6.));
    for r in result {
        for (f, [input, face]) in r.brep.faces.iter().zip(r.face_sources) {
            assert_eq!(f.surface, [&a, &b, &c][input].faces[face].surface);
        }
    }
    assert_eq!((a, b, c), before);
}

#[test]
fn maximal_pair_containment_is_local_to_each_disconnected_material_region() {
    let a = Brep::try_disjoint_union(
        vec![cube([[0., 3.]; 3]), cube([[4., 5.]; 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let c = cube([[3.5, 6.]; 3]);
    let b = cube([[2., 4.5]; 3]);
    let result = intersect_polyhedral_brep_sets(&[&a, &c], &[&b], Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 2);
    let left = result.iter().find(|r| r.pairs.len() == 1).unwrap();
    assert_eq!(left.pairs, vec![[0, 0]]);
    assert_eq!(left.maximal_pairs, left.pairs);
    measure(Some(left.brep.clone()), 1., Some(6.));
    let right = result.iter().find(|r| r.pairs.len() == 2).unwrap();
    assert_eq!(right.pairs, vec![[0, 0], [1, 0]]);
    assert_eq!(right.maximal_pairs, vec![[1, 0]]);
    measure(Some(right.brep.clone()), 1., Some(6.));
    for r in result {
        for (f, [input, face]) in r.brep.faces.iter().zip(r.face_sources) {
            assert_eq!(f.surface, [&a, &c, &b][input].faces[face].surface);
        }
    }
}

#[test]
fn multiple_common_and_cutters_preserve_holes_and_bounded_failures() {
    let (a, b) = source("hole");
    let c = cube([[1.75, 4.], [1.5, 3.5], [0.5, 2.5]]);
    let common = intersect_polyhedral_breps(&[&a, &b, &c], Tolerance::DEFAULT).unwrap();
    assert_eq!(common.len(), 1);
    measure(Some(common[0].brep.clone()), 1.75, None);
    let cutter = cube([[1.5, 2.5], [0.5, 1.5], [1., 2.]]);
    let remainder = subtract_polyhedral_breps(&a, &[&b, &cutter], Tolerance::DEFAULT).unwrap();
    assert_eq!(remainder.len(), 1);
    measure(Some(remainder[0].brep.clone()), 21.25, None);
    let copy = subtract_polyhedral_breps(&a, &[], Tolerance::DEFAULT).unwrap();
    assert_eq!(copy.len(), 1);
    measure(Some(copy[0].brep.clone()), 24., None);
    assert!(
        union_polyhedral_breps(&[], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert!(
        intersect_polyhedral_brep_sets(&[], &[&a], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    let excessive = vec![&a; 129];
    assert!(matches!(
        union_polyhedral_breps(&excessive, Tolerance::DEFAULT),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    assert!(matches!(
        intersect_polyhedral_brep_sets(&excessive, &[], Tolerance::DEFAULT),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    assert!(matches!(
        arrangement::build(&[&a, &b], Tolerance::DEFAULT, &mut Budget(1)),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
}

#[test]
fn polyhedral_interaction_policy_matches_convex_certificates_and_void_boundaries() {
    let a = cube([[0., 2.]; 3]);
    for bounds in [
        [[1., 3.]; 3],
        [[0., 2.]; 3],
        [[0.5, 1.5]; 3],
        [[3., 4.]; 3],
        [[2., 4.], [0., 2.], [0., 2.]],
        [[2., 4.], [2., 4.], [0., 2.]],
        [[2., 4.]; 3],
        [[1., 2.]; 3],
        [[0., 1.]; 3],
        [[-1., 0.], [0.5, 1.5], [0.5, 1.5]],
    ] {
        let b = cube(bounds);
        for (inward_a, inward_b) in [(false, false), (true, true)] {
            let a = if inward_a { a.reversed() } else { a.clone() };
            let b = if inward_b { b.reversed() } else { b.clone() };
            let refs = [&a, &b];
            assert_eq!(
                polyhedral_brep_boundary_interactions(&refs, Tolerance::DEFAULT).unwrap(),
                convex_brep_boundary_interactions(&refs).unwrap(),
                "{bounds:?}"
            );
            assert_eq!(
                polyhedral_brep_subtraction_interactions(&refs, Tolerance::DEFAULT).unwrap(),
                convex_brep_subtraction_interactions(&refs).unwrap(),
                "{bounds:?}"
            );
        }
    }
    let cavity = Brep::try_disjoint_union(
        vec![cube([[0., 4.]; 3]), cube([[1., 3.]; 3]).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let crossing = cube([[2., 2.5], [2., 2.5], [2.5, 3.5]]);
    let inside_void = cube([[1.5, 2.5]; 3]);
    assert_eq!(
        polyhedral_brep_boundary_interactions(
            &[&cavity, &crossing, &inside_void],
            Tolerance::DEFAULT
        )
        .unwrap(),
        vec![[0, 1], [1, 2]]
    );
}
