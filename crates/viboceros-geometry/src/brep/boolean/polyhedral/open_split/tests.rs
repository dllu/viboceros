use super::super::super::tests::cube;
use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane(lo: f64, hi: f64) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., lo, -1.), p(1., hi, -1.), p(1., hi, 3.), p(1., lo, 3.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn open_target_normal_selects_the_shared_solid_half_and_keeps_open_remainder() {
    let target = plane(-1., 3.);
    let cutter = cube([[0., 2.]; 3]);
    for (target, expected_x) in [(target.clone(), 0.5), (target.reversed(), 1.5)] {
        let before = (target.clone(), cutter.clone());
        let pieces = split_open_polyhedral_brep(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
        assert_eq!(pieces.len(), 2);
        let solid = pieces.iter().find(|p| p.brep.is_solid()).unwrap();
        let mass = solid
            .brep
            .volume_mass_properties(Tolerance::DEFAULT)
            .unwrap();
        assert!((mass.signed_volume().unwrap() - 4.).abs() < 1e-10);
        assert!((mass.centroid().unwrap().x() - expected_x).abs() < 1e-10);
        let open = pieces.iter().find(|p| !p.brep.is_solid()).unwrap();
        assert!(
            open.brep
                .faces()
                .iter()
                .flat_map(|f| f.loops())
                .flat_map(|l| l.trims())
                .any(|t| t.trim_type() == BrepTrimType::Boundary)
        );
        assert!((open.brep.area(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-9);
        assert_eq!((target, cutter.clone()), before);
    }
}
#[test]
fn open_target_finite_coverage_rejects_short_sheet_and_preserves_closed_contract() {
    let partial = plane(-1., 1.);
    let cube = cube([[0., 2.]; 3]);
    assert!(
        split_open_polyhedral_brep(&partial, &[&cube], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    let full = plane(-1., 3.);
    assert!(matches!(
        split_polyhedral_brep_with_surfaces(&full, &[&cube], Tolerance::DEFAULT),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
    ));
    assert!(split_open_polyhedral_brep(&cube, &[&full], Tolerance::DEFAULT).is_err());
}

#[test]
fn mixed_coplanar_stages_preserve_unrounded_remainders_and_cut_order() {
    let target = plane(-1., 3.);
    let overlap = Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., 0., 0.), p(1., 2., 0.), p(1., 2., 2.), p(1., 0., 2.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let perpendicular = Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            p(-1., 1., -1.),
            p(-1., 1., 3.),
            p(3., 1., 3.),
            p(3., 1., -1.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (cutters, expected) in [
        (vec![&overlap, &perpendicular], vec![12., 16., 16.]),
        (vec![&perpendicular, &overlap], vec![16., 16.]),
    ] {
        let pieces = split_open_polyhedral_brep(&target, &cutters, Tolerance::DEFAULT).unwrap();
        let mut areas = pieces
            .iter()
            .map(|p| p.brep.area(Tolerance::DEFAULT).unwrap())
            .collect::<Vec<_>>();
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas, expected);
        assert!(pieces.iter().all(|p| !p.brep.is_solid()));
        for piece in &pieces {
            for (face, [owner, index]) in piece.brep.faces.iter().zip(&piece.face_sources) {
                assert_eq!(
                    face.surface,
                    [&target]
                        .into_iter()
                        .chain(cutters.iter().copied())
                        .collect::<Vec<_>>()[*owner]
                        .faces[*index]
                        .surface
                );
            }
        }
    }
}

#[test]
fn coplanar_hole_boundary_crossing_is_ignored_and_future_faces_do_not_expand_target() {
    let target = plane(-1., 3.);
    let solid = cube([[0., 2.]; 3]);
    let straddle = Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            p(1., -0.5, -0.5),
            p(1., 0.5, -0.5),
            p(1., 0.5, 0.5),
            p(1., -0.5, 0.5),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let pieces =
        split_open_polyhedral_brep(&target, &[&solid, &straddle], Tolerance::DEFAULT).unwrap();
    assert_eq!(pieces.len(), 2);
    let mut areas = pieces
        .iter()
        .map(|p| p.brep.area(Tolerance::DEFAULT).unwrap())
        .collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas, [16., 24.]);
    let outside = Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., 4., 0.), p(1., 5., 0.), p(1., 5., 1.), p(1., 4., 1.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!(
        split_open_polyhedral_brep(&target, &[&outside], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}

fn holed_sheet() -> Brep {
    let outer = cube([[0., 1.], [-1., 3.], [-1., 3.]]);
    let tunnel = cube([[-1., 2.], [0.5, 1.5], [0.5, 1.5]]);
    let tube = outer
        .try_boolean_convex(
            &tunnel,
            BrepBooleanOperation::Difference,
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
    let tube = tube
        .try_merge_coplanar_polygon_faces_in_groups(&vec![0; tube.faces.len()], Tolerance::DEFAULT)
        .unwrap()
        .unwrap_or(tube);
    let face = tube
        .faces
        .iter()
        .position(|f| {
            f.loops.len() == 2
                && f.loops[0]
                    .trims
                    .iter()
                    .all(|t| tube.vertices[t.vertices[0]].point.x() == 1.)
        })
        .unwrap();
    tube.duplicate_faces(&[face], Tolerance::DEFAULT).unwrap()
}
#[test]
fn trimmed_sheet_boundary_api_keeps_holes_while_solid_partition_requires_full_caps() {
    let target = cube([[0., 2.]; 3]);
    let sheet = holed_sheet();
    let strict =
        split_polyhedral_brep_with_surfaces(&target, &[&sheet], Tolerance::DEFAULT).unwrap();
    assert_eq!(strict.len(), 1);
    assert!(strict[0].brep.is_solid());
    assert_eq!(strict[0].cut_sides, [None]);
    let open =
        split_polyhedral_brep_by_trimmed_sheets(&target, &[&sheet], Tolerance::DEFAULT).unwrap();
    assert_eq!(open.len(), 2);
    for p in open {
        assert!(!p.brep.is_solid());
        assert!((p.brep.area(Tolerance::DEFAULT).unwrap() - 15.).abs() < 1e-9);
    }
}
#[test]
fn original_trim_holes_and_lineage_reports_preserve_disconnected_remainders() {
    let target = holed_sheet();
    let cutter = cube([[0., 2.]; 3]);
    let pieces =
        split_open_polyhedral_brep_with_lineage(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
    let mut areas = pieces
        .iter()
        .map(|p| p.brep.area(Tolerance::DEFAULT).unwrap())
        .collect::<Vec<_>>();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas, [15., 24.]);
    assert!(pieces.iter().all(|p| p.branch_component_count == 1));
    let overlap = Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., 0., 0.), p(1., 2., 0.), p(1., 2., 2.), p(1., 0., 2.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let pieces =
        split_open_polyhedral_brep_with_lineage(&target, &[&overlap], Tolerance::DEFAULT).unwrap();
    assert_eq!(pieces.len(), 3);
    assert_eq!(
        pieces
            .iter()
            .filter(|p| p.branch_component_count == 2)
            .count(),
        2
    );
}
#[test]
fn compound_open_boundaries_repeat_untouched_components_in_each_branch() {
    let target = Brep::try_disjoint_union(
        vec![
            plane(-1., 3.),
            Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    p(1., 4., 4.),
                    p(1., 6., 4.),
                    p(1., 6., 6.),
                    p(1., 4., 6.),
                ])
                .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cutter = cube([[0., 2.]; 3]);
    let pieces =
        split_open_polyhedral_brep_with_lineage(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
    assert_eq!(pieces.len(), 4);
    assert!(pieces.iter().all(|p| p.branch_component_count == 2));
    assert_eq!(
        pieces
            .iter()
            .filter(|p| (p.brep.area(Tolerance::DEFAULT).unwrap() - 4.).abs() < 1e-9)
            .count(),
        2
    );
}
