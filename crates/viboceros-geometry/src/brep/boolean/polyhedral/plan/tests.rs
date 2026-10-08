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
fn original_shell_classification_uses_material_sides_independently_of_raw_winding() {
    // The mathematical API treats nested shells by odd/even membership.
    // Raw input winding must not decide the optimized shell enclosure masks.
    let original = Brep::try_disjoint_union(
        vec![
            cube([[0., 4.]; 3]),
            cube([[1., 3.]; 3]),
            cube([[1.5, 2.5]; 3]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let reversed = original.reversed();
    let cutter = cube([[2., 5.]; 3]);
    for operand in [&original, &reversed] {
        let mut plan =
            BrepPolyhedralBooleanPlan::try_new(&[operand, &cutter], Tolerance::DEFAULT).unwrap();
        let region = plan.input(0).unwrap();
        let shells = plan.shells(&region).unwrap();
        assert_eq!(shells.len(), 3);
        assert_eq!(
            shells.iter().map(|s| s.inward).collect::<Vec<_>>(),
            [false, true, false]
        );
        let (mut polygons, _) = plan.boundary(&region).unwrap();
        let groups = exact_shells(&mut polygons, &mut Budget(EXACT_WORK_LIMIT)).unwrap();
        for (shell, faces) in shells.iter().zip(groups) {
            let mut reference = Budget(usize::MAX);
            for (i, sample) in plan.built.samples.iter().enumerate() {
                assert_eq!(
                    shell.region.mask[i],
                    union::contains(&sample.point, &faces, &polygons, &mut reference).unwrap()
                );
            }
        }
    }
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

#[test]
fn boundary_export_retains_nonmanifold_edges_without_claiming_a_material_solid() {
    let a = cube([[0., 1.]; 3]);
    let b = cube([[1., 2.], [1., 2.], [0., 1.]]);
    let before = (a.clone(), b.clone());
    let operands = [&a, &b];
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&operands, Tolerance::DEFAULT).unwrap();
    let left = plan.input(0).unwrap();
    let right = plan.input(1).unwrap();
    assert!(!plan.boundary_interacts(&left, &right).unwrap());
    assert!(plan.boundaries_share_line(&left, &right).unwrap());
    let region = plan
        .combine(BrepBooleanOperation::Union, &[&left, &right])
        .unwrap();
    assert!(matches!(
        plan.export(&region),
        Err(GeometryError::UnrepresentableBrepBoolean)
    ));
    let output = plan.export_boundary(&region).unwrap();
    assert_eq!(output.len(), 1);
    let exported = &output[0].brep;
    assert!(!exported.is_solid());
    assert!(!exported.is_manifold());
    assert!(exported.edge_use_counts().contains(&4));
    assert!((exported.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-10);
    assert!(output[0].boundary_equal_inputs.is_empty());
    for (face, [owner, index]) in exported.faces().iter().zip(&output[0].face_sources) {
        assert_eq!(face.surface(), operands[*owner].faces()[*index].surface());
    }
    assert_eq!((&a, &b), (&before.0, &before.1));
}

#[test]
fn boundary_export_preserves_cavity_winding_and_reports_complete_input_aliases() {
    let a = Brep::try_disjoint_union(
        vec![cube([[0., 3.]; 3]), cube([[1., 2.]; 3]).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = a.clone();
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let region = plan.input(0).unwrap();
    let outputs = plan.export_boundary(&region).unwrap();
    assert_eq!(outputs.len(), 2);
    let mut volumes = outputs
        .iter()
        .map(|o| o.brep.signed_volume(Tolerance::DEFAULT).unwrap())
        .collect::<Vec<_>>();
    volumes.sort_by(f64::total_cmp);
    assert_eq!(volumes, vec![-1., 27.]);
    assert!(outputs.iter().all(|o| o.boundary_equal_inputs.is_empty()));
    let plain = cube([[0., 1.]; 3]);
    let duplicate = plain.clone();
    let mut plan =
        BrepPolyhedralBooleanPlan::try_new(&[&plain, &duplicate], Tolerance::DEFAULT).unwrap();
    let region = plan.input(0).unwrap();
    let allowed = (0..6).map(|f| [1, f]).collect::<Vec<_>>();
    let output = plan.export_boundary_with_faces(&region, &allowed).unwrap();
    assert_eq!(output[0].boundary_equal_inputs, vec![0, 1]);
    assert!(output[0].boundary_faces.iter().all(|f| f[0] == 1));
    assert!(output[0].face_sources.iter().all(|f| f[0] == 1));
    assert!(
        plan.export_boundary_with_faces(&region, &allowed[..1])
            .is_err()
    );
}

#[test]
fn positive_length_boundary_query_excludes_point_only_contacts() {
    let a = cube([[0., 1.]; 3]);
    let b = cube([[1., 2.]; 3]);
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&a, &b], Tolerance::DEFAULT).unwrap();
    let a = plan.input(0).unwrap();
    let b = plan.input(1).unwrap();
    assert!(!plan.boundaries_share_line(&a, &b).unwrap());
}

#[test]
fn oriented_planar_regions_export_only_finite_physical_patches_and_share_one_plan() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let sheet = |hi| {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear([
                p(1., -1., -1.),
                p(1., hi, -1.),
                p(1., hi, 3.),
                p(1., -1., 3.),
            ])
            .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let full = sheet(3.);
    let solid = cube([[0., 2.]; 3]);
    let before = (full.clone(), solid.clone());
    for (full, center) in [(full.clone(), 0.5), (full.reversed(), 1.5)] {
        let mut plan =
            BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&full, &solid], Tolerance::DEFAULT)
                .unwrap();
        assert!(plan.planar_sheet_covers_input_section(0, 1).unwrap());
        let a = plan.input(0).unwrap();
        let b = plan.input(1).unwrap();
        let common = plan
            .combine(BrepBooleanOperation::Intersection, &[&a, &b])
            .unwrap();
        let pieces = plan.export_physical_boundary(&common).unwrap();
        assert_eq!(pieces.len(), 1);
        let mass = pieces[0]
            .brep
            .volume_mass_properties(Tolerance::DEFAULT)
            .unwrap();
        assert!((mass.signed_volume().unwrap() - 4.).abs() < 1e-10);
        assert!((mass.centroid().unwrap().x() - center).abs() < 1e-10);
        assert!(pieces[0].face_sources.iter().any(|s| s[0] == 0));
        assert!(pieces[0].face_sources.iter().any(|s| s[0] == 1));
        let union = plan
            .combine(BrepBooleanOperation::Union, &[&a, &b])
            .unwrap();
        let pieces = plan.export_physical_boundary(&union).unwrap();
        assert_eq!(pieces.len(), 1);
        assert!(!pieces[0].brep.is_solid());
        assert!((pieces[0].brep.area(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-9);
        let mut other =
            BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&full, &solid], Tolerance::DEFAULT)
                .unwrap();
        assert!(other.export_physical_boundary(&union).is_err());
        assert!(plan.planar_sheet_covers_input_section(2, 1).is_err());
    }
    let partial = sheet(1.);
    let mut plan =
        BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&partial, &solid], Tolerance::DEFAULT)
            .unwrap();
    assert!(!plan.planar_sheet_covers_input_section(0, 1).unwrap());
    assert_eq!((full, solid), before);
}

#[test]
fn finite_coplanar_sheet_sets_preserve_holes_and_reject_crossing_supports() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let sheet = |lo, hi| {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear([
                p(1., lo, lo),
                p(1., hi, lo),
                p(1., hi, hi),
                p(1., lo, hi),
            ])
            .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let a = sheet(-1., 3.);
    let b = sheet(0., 2.);
    let mut plan =
        BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&a, &b], Tolerance::DEFAULT).unwrap();
    assert!(plan.inputs_are_coplanar(0, 1).unwrap());
    for (operation, expected) in [
        (BrepBooleanOperation::Union, 16.),
        (BrepBooleanOperation::Intersection, 4.),
        (BrepBooleanOperation::Difference, 12.),
    ] {
        let outputs = plan.export_coplanar_sheet_boolean(operation, 0, 1).unwrap();
        assert_eq!(outputs.len(), 1);
        assert!((outputs[0].brep.area(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
        if operation == BrepBooleanOperation::Difference {
            let merged = outputs[0]
                .brep
                .try_merge_coplanar_polygon_faces_in_groups(
                    &vec![0; outputs[0].brep.faces().len()],
                    Tolerance::DEFAULT,
                )
                .unwrap()
                .unwrap_or_else(|| outputs[0].brep.clone());
            assert!(merged.faces().iter().any(|f| f.loops().len() == 2));
        }
    }
    assert!(
        plan.export_coplanar_sheet_boolean(BrepBooleanOperation::Difference, 1, 0)
            .unwrap()
            .is_empty()
    );
    let cube = cube([[0., 2.]; 3]);
    let mut mixed =
        BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&a, &cube], Tolerance::DEFAULT)
            .unwrap();
    assert!(!mixed.inputs_are_coplanar(0, 1).unwrap());
    assert!(
        mixed
            .export_coplanar_sheet_boolean(BrepBooleanOperation::Union, 0, 1)
            .is_err()
    );
}

#[test]
fn coplanar_partial_area_queries_preserve_operand_order_and_exact_coverage() {
    let p = |y, z| Point3::try_new(1., y, z).unwrap();
    let sheet = |ylo, yhi, zlo, zhi| {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear([p(ylo, zlo), p(yhi, zlo), p(yhi, zhi), p(ylo, zhi)])
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    };
    let a = sheet(-1., 3., -1., 3.);
    let b = sheet(1., 4., 0., 2.);
    let before = (a.clone(), b.clone());
    let mut plan =
        BrepPolyhedralBooleanPlan::try_with_planar_sheets(&[&a, &b], Tolerance::DEFAULT).unwrap();
    for (operation, first, second, area) in [
        (BrepBooleanOperation::Union, 0, 1, 18.),
        (BrepBooleanOperation::Intersection, 0, 1, 4.),
        (BrepBooleanOperation::Difference, 0, 1, 12.),
        (BrepBooleanOperation::Difference, 1, 0, 2.),
    ] {
        let pieces = plan
            .export_coplanar_sheet_boolean(operation, first, second)
            .unwrap();
        assert_eq!(pieces.len(), 1);
        let actual = pieces
            .iter()
            .map(|p| p.brep.area(Tolerance::DEFAULT).unwrap())
            .sum::<f64>();
        assert!((actual - area).abs() < 1e-9);
        if operation == BrepBooleanOperation::Difference {
            assert!(
                pieces
                    .iter()
                    .flat_map(|p| &p.face_sources)
                    .all(|s| s[0] == first)
            );
        }
    }
    let union = plan.export_coplanar_sheet_partition(0, 1).unwrap();
    assert_eq!(union.len(), 1);
    assert!((union[0].brep.area(Tolerance::DEFAULT).unwrap() - 18.).abs() < 1e-9);
    assert!(union[0].face_sources.iter().any(|s| s[0] == 0));
    assert!(union[0].face_sources.iter().any(|s| s[0] == 1));
    assert_eq!(union[0].face_sources.len(), union[0].face_categories.len());
    assert_eq!(
        union[0]
            .face_categories
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3])
    );
    assert!(plan.export_coplanar_sheet_partition(0, 2).is_err());
    plan.exported_faces = MAX_OUTPUT_FACES;
    assert!(matches!(
        plan.export_coplanar_sheet_partition(0, 1),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    plan.budget = Budget(0);
    assert!(matches!(
        plan.export_coplanar_sheet_partition(0, 1),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    assert_eq!((a, b), before);
}
