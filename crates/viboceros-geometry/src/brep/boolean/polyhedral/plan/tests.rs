use super::super::super::tests::{cube, measure, tetra};
use super::*;

#[test]
fn distinguishes_boundary_enclosure_from_volume_containment_and_keeps_shell_signs() {
    let cavity = Brep::try_disjoint_union(
        vec![cube([[0., 3.]; 3]), cube([[1., 2.]; 3]).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surrounds_hole = cube([[0.75, 2.25]; 3]);
    let before = cavity.clone();
    let mut plan =
        BrepPolyhedralBooleanPlan::try_new(&[&cavity, &surrounds_hole], Tolerance::DEFAULT)
            .unwrap();
    let a = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    assert!(plan.boundary_covered_by(&b, &a).unwrap());
    assert!(!plan.covered_by(&b, &a).unwrap());
    assert!(!plan.boundary_interacts(&a, &b).unwrap());
    let shells = plan.shells(&a).unwrap();
    assert_eq!(shells.len(), 2);
    for shell in shells {
        let output = plan.export(&shell.region).unwrap();
        assert_eq!(output.len(), 1);
        measure(
            Some(output[0].brep.clone()),
            if shell.inward { 1. } else { 27. },
            None,
        );
    }
    assert_eq!(cavity, before);
}

#[test]
fn staged_rational_cuts_round_only_at_export_and_preserve_original_surfaces() {
    let stretch = AffineTransform3::try_new(
        [[3., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let a = tetra([0.; 3])
        .transformed(stretch, Tolerance::DEFAULT)
        .unwrap();
    let first = cube([[0.5, 10.], [0., 4.], [0., 4.]]);
    let second = cube([[1., 10.], [0., 4.], [0., 4.]]);
    let operands = [&a, &first, &second];
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&operands, Tolerance::DEFAULT).unwrap();
    let a_region = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    let c = plan.input(2).unwrap();
    let initial = plan
        .combine(BrepBooleanOperation::Intersection, &[&a_region, &b])
        .unwrap();
    let initial_shells = plan.shells(&initial).unwrap();
    assert_eq!(initial_shells.len(), 1);
    let first_export = plan.export(&initial_shells[0].region).unwrap();
    measure(Some(first_export[0].brep.clone()), 4913. / 432., None);
    assert!(
        first_export[0]
            .brep
            .vertices()
            .iter()
            .any(|v| (v.point().y() - 17. / 6.).abs() < 1e-14)
    );
    // Continue with the exact shell, even after an earlier rounded export.
    let clipped = plan
        .combine(
            BrepBooleanOperation::Intersection,
            &[&initial_shells[0].region, &c],
        )
        .unwrap();
    let result = plan.export(&clipped).unwrap();
    assert_eq!(result.len(), 1);
    measure(Some(result[0].brep.clone()), 256. / 27., None);
    for (face, [owner, index]) in result[0].brep.faces().iter().zip(&result[0].face_sources) {
        assert_eq!(face.surface(), operands[*owner].faces()[*index].surface());
    }
}

#[test]
fn equivalent_boundary_faces_are_retained_and_foreign_regions_rejected() {
    let a = cube([[0., 1.]; 3]);
    let b = a.clone();
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let region = plan.input(1).unwrap();
    assert_eq!(plan.boundary_sources(&region).unwrap(), vec![0, 1]);
    assert_eq!(plan.boundary_faces(&region).unwrap().len(), 12);
    let mut foreign = BrepPolyhedralBooleanPlan::try_new(&[&a], Tolerance::DEFAULT).unwrap();
    let other = foreign.input(0).unwrap();
    assert!(
        plan.combine(BrepBooleanOperation::Union, &[&region, &other])
            .is_err()
    );
    assert!(plan.covered_by(&region, &other).is_err());
    assert!(plan.input(2).is_err());
    let result = plan.export(&region).unwrap();
    assert!(result[0].face_sources.iter().all(|s| s[0] == 0));
    plan.exported_faces = MAX_OUTPUT_FACES;
    assert!(matches!(
        plan.export(&region),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    plan.budget = Budget(0);
    assert!(matches!(
        plan.export(&region),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
}

#[test]
fn derived_shell_membership_matches_independent_rays_with_islands_and_disjoint_parts() {
    let a = Brep::try_disjoint_union(
        vec![
            cube([[0., 4.]; 3]),
            cube([[1., 3.]; 3]).reversed(),
            cube([[1.5, 2.5]; 3]),
            cube([[10., 11.]; 3]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = cube([[2., 5.]; 3]);
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let a = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    let united = plan
        .combine(BrepBooleanOperation::Union, &[&a, &b])
        .unwrap();
    for region in [a, united] {
        let shells = plan.shells(&region).unwrap();
        let (mut polygons, _) = plan.boundary(&region).unwrap();
        let groups = exact_shells(&mut polygons, &mut Budget(EXACT_WORK_LIMIT)).unwrap();
        assert_eq!(groups.len(), shells.len());
        for (shell, faces) in shells.iter().zip(groups) {
            let mut reference = Budget(usize::MAX);
            for (i, s) in plan.built.samples.iter().enumerate() {
                assert_eq!(
                    shell.region.mask[i],
                    union::contains(&s.point, &faces, &polygons, &mut reference).unwrap()
                );
            }
        }
    }
}

#[test]
fn rejects_singular_intermediate_shells_before_export() {
    let a = cube([[0., 1.]; 3]);
    let b = cube([[1., 2.], [1., 2.], [0., 1.]]);
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let a = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    let union = plan
        .combine(BrepBooleanOperation::Union, &[&a, &b])
        .unwrap();
    assert!(matches!(
        plan.shells(&union),
        Err(GeometryError::UnrepresentableBrepBoolean)
    ));
}

#[test]
fn staged_face_ownership_excludes_unrelated_coplanar_cavity_faces() {
    let a = Brep::try_disjoint_union(
        vec![cube([[0., 3.]; 3]), cube([[1., 2.]; 3]).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = cube([[1.5, 3.5], [1.5, 3.5], [1., 2.]]);
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let a = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    let outer = plan
        .shells(&a)
        .unwrap()
        .into_iter()
        .find(|s| !s.inward)
        .unwrap();
    let region = plan
        .combine(BrepBooleanOperation::Intersection, &[&outer.region, &b])
        .unwrap();
    let allowed = (0..6)
        .map(|f| [0, f])
        .chain((0..6).map(|f| [1, f]))
        .collect::<Vec<_>>();
    let output = plan.export_with_boundary_faces(&region, &allowed).unwrap();
    assert!(output[0].face_sources.iter().all(|s| s[0] != 0 || s[1] < 6));
    measure(Some(output[0].brep.clone()), 2.25, None);
    assert!(matches!(
        plan.export_with_boundary_faces(&region, &[[0, 0]]),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
    ));
}
