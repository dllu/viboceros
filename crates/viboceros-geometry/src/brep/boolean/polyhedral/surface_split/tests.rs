use super::super::super::tests::cube;
use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn x(lo: f64, hi: f64) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., lo, -1.), p(1., hi, -1.), p(1., hi, 3.), p(1., lo, 3.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn y() -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            p(-1., 1., -1.),
            p(-1., 1., 3.),
            p(3., 1., 3.),
            p(3., 1., -1.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn volumes(pieces: &[BrepSurfaceSplitComponent]) -> Vec<f64> {
    let mut values = pieces
        .iter()
        .map(|p| {
            assert!(p.brep.is_solid());
            p.brep.signed_volume(Tolerance::DEFAULT).unwrap()
        })
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    values
}

#[test]
fn finite_sheet_partitions_require_complete_coverage_at_each_current_region() {
    let target = cube([[0., 2.]; 3]);
    let partial = x(-1., 1.);
    let full = y();
    for (cutters, expected) in [
        (vec![&partial], vec![8.]),
        (vec![&partial, &full], vec![4., 4.]),
        (vec![&full, &partial], vec![2., 2., 4.]),
    ] {
        let result =
            split_polyhedral_brep_with_surfaces(&target, &cutters, Tolerance::DEFAULT).unwrap();
        assert_eq!(volumes(&result), expected);
        assert!(
            result
                .iter()
                .all(|p| p.branch_component_counts.iter().all(|&n| n == 1))
        );
        if cutters.len() == 1 {
            assert_eq!(result[0].cut_sides, [None]);
        }
    }
}

#[test]
fn coplanar_sheet_union_keeps_actual_face_ownership_and_normal_relative_sides() {
    let target = cube([[0., 2.]; 3]);
    let low = x(-1., 1.);
    let high = x(1., 3.).reversed();
    let before = (target.clone(), low.clone(), high.clone());
    let inputs = [&target, &low, &high];
    let result =
        split_polyhedral_brep_with_surfaces(&target, &[&low, &high], Tolerance::DEFAULT).unwrap();
    assert_eq!(volumes(&result), [4., 4.]);
    for piece in result {
        assert_eq!(piece.branch_component_counts, [1]);
        assert_ne!(piece.cut_sides[0], piece.cut_sides[1]);
        let mut cap_sources = BTreeSet::new();
        for (face, [owner, index]) in piece.brep.faces.iter().zip(piece.face_sources) {
            assert_eq!(face.surface, inputs[owner].faces[index].surface);
            if owner > 0 {
                cap_sources.insert(owner);
            }
        }
        assert_eq!(cap_sources, BTreeSet::from([1, 2]));
    }
    assert_eq!((target, low, high), before);
}

#[test]
fn planning_components_attach_cavities_and_separate_nested_islands_without_exporting() {
    let input = Brep::try_disjoint_union(
        vec![
            cube([[0., 4.]; 3]),
            cube([[1., 3.]; 3]).reversed(),
            cube([[1.5, 2.5]; 3]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut plan = BrepPolyhedralBooleanPlan::try_new(&[&input], Tolerance::DEFAULT).unwrap();
    let region = plan.input(0).unwrap();
    let components = plan.components(&region).unwrap();
    assert_eq!(components.len(), 2);
    let intersection = plan
        .combine(
            BrepBooleanOperation::Intersection,
            &[&components[0], &components[1]],
        )
        .unwrap();
    assert!(plan.is_empty(&intersection).unwrap());
    let mut volumes = Vec::new();
    for region in components {
        let bodies = plan.export(&region).unwrap();
        assert_eq!(bodies.len(), 1);
        volumes.push(bodies[0].brep.signed_volume(Tolerance::DEFAULT).unwrap());
    }
    volumes.sort_by(f64::total_cmp);
    assert_eq!(volumes, [1., 56.]);
}

#[test]
fn sheet_work_limits_and_noncoplanar_faces_fail_before_output() {
    let target = cube([[0., 2.]; 3]);
    let full = x(-1., 3.);
    assert!(matches!(
        split_polyhedral_brep_with_surfaces(&target, &vec![&full; 128], Tolerance::DEFAULT),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    let warped = Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            p(1., -1., -1.),
            p(1., 3., -1.),
            p(1.5, 3., 3.),
            p(1., -1., 3.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(matches!(
        split_polyhedral_brep_with_surfaces(&target, &[&warped], Tolerance::DEFAULT),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
    ));
}
