//! Qualified planar offset geometry and admission regressions.
use super::*;

#[test]
fn planar_surface_offsets_on_both_sides_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(
            &mut document,
            "OffsetSrf 2 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    assert_eq!(document.objects().count(), 3);
    let Geometry::Brep(offset) = &result_geometry(&document) else {
        panic!("two-sided offset is one disjoint B-rep")
    };
    assert_eq!(offset.faces().len(), 2);
    assert_eq!(offset.edges().len(), 8);
    assert_eq!(offset.vertices().len(), 8);
    assert!(offset.faces().iter().all(|face| !face.is_reversed()));
    let mut heights = offset
        .faces()
        .iter()
        .map(|face| {
            let surface = face.surface();
            surface
                .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
                .unwrap()
                .z()
        })
        .collect::<Vec<_>>();
    heights.sort_by(Real::total_cmp);
    assert_eq!(heights, [-2.0, 2.0]);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn solid_offset_closes_a_planar_surface_at_exact_volume() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(
            &mut document,
            "OffsetSrf 2 Solid=Yes BothSides=No DeleteInput=No",
        )
        .unwrap();
    assert_eq!(document.objects().count(), 2);
    let Geometry::Brep(solid) = &result_geometry(&document) else {
        panic!("solid offset is a B-rep")
    };
    assert_eq!(
        (
            solid.faces().len(),
            solid.edges().len(),
            solid.vertices().len()
        ),
        (6, 12, 8)
    );
    assert!((solid.signed_volume(document.tolerance()).unwrap() - 24.0).abs() < 1e-9);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 2 Solid=Yes BothSides=Yes DeleteInput=No",
        )
        .unwrap();
    assert_eq!(document.objects().count(), 2);
    let Geometry::Brep(solid) = &result_geometry(&document) else {
        panic!("two-sided solid offset is one B-rep")
    };
    assert_eq!(
        (
            solid.faces().len(),
            solid.edges().len(),
            solid.vertices().len()
        ),
        (6, 12, 8)
    );
    assert!((solid.signed_volume(document.tolerance()).unwrap() - 48.0).abs() < 1e-9);

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -2 Solid=Yes BothSides=No DeleteInput=No",
        )
        .unwrap();
    let Geometry::Brep(solid) = &result_geometry(&document) else {
        panic!("negative solid offset is a B-rep")
    };
    assert_eq!(
        (
            solid.faces().len(),
            solid.edges().len(),
            solid.vertices().len()
        ),
        (6, 12, 8)
    );
    assert!((solid.signed_volume(document.tolerance()).unwrap() - 24.0).abs() < 1e-9);
}

#[test]
fn trimmed_planar_brep_offset_follows_face_orientation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Rectangle 0,0,0 4,3,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(&mut document, "PlanarSrf DeleteInput=Yes")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let source = result_geometry(&document);
    let normal = planar_normal(&source, document.tolerance()).unwrap();
    let Geometry::Brep(original) = &source else {
        panic!("planar source is a B-rep")
    };
    let surface = original.faces()[0].surface();
    let source_point = surface
        .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
        .unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -2 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(offset) = &result_geometry(&document) else {
        panic!("trimmed offset remains a B-rep")
    };
    let surface = offset.faces()[0].surface();
    let offset_point = surface
        .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
        .unwrap();
    let displacement = source_point.vector_to(offset_point).unwrap();
    assert!((displacement.dot(normal).unwrap() + 2.0).abs() < 1e-10);
    assert_eq!(
        offset.faces()[0].loops().len(),
        original.faces()[0].loops().len()
    );
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 1 Solid=Yes BothSides=No DeleteInput=No",
        )
        .unwrap();
    let Geometry::Brep(solid) = &result_geometry(&document) else {
        panic!("trimmed planar solid offset is a B-rep")
    };
    assert!((solid.signed_volume(document.tolerance()).unwrap() - 12.0).abs() < 1e-9);
}

#[test]
fn two_sided_trimmed_face_keeps_both_trim_loops_in_separate_objects() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Rectangle 0,0,0 4,3,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(&mut document, "PlanarSrf DeleteInput=Yes")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let Geometry::Brep(source) = &result_geometry(&document) else {
        panic!("planar source is a B-rep")
    };
    let source_loop_count = source.faces()[0].loops().len();
    let source_edge_count = source.edges().len();
    registry
        .execute(
            &mut document,
            "OffsetSrf 2 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    assert_eq!(document.objects().count(), 3);
    let Geometry::Brep(offset) = &result_geometry(&document) else {
        panic!("two-sided trimmed offset is a B-rep")
    };
    assert_eq!(offset.faces().len(), 2);
    assert!(
        offset
            .faces()
            .iter()
            .all(|face| face.loops().len() == source_loop_count)
    );
    assert_eq!(offset.edges().len(), source_edge_count * 2);
}

#[test]
fn vertical_rectangle_offsets_to_six_face_solid() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,0,3 0,0,3")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -2 Solid=Yes BothSides=Yes DeleteInput=Yes",
        )
        .unwrap();
    let Geometry::Brep(solid) = &result_geometry(&document) else {
        panic!("vertical offset is a B-rep")
    };
    assert_eq!(
        (
            solid.faces().len(),
            solid.edges().len(),
            solid.vertices().len()
        ),
        (6, 12, 8)
    );
    assert!((solid.signed_volume(document.tolerance()).unwrap() - 48.0).abs() < 1e-9);
}

#[test]
fn unsupported_selection_does_not_create_partial_offsets() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
        .unwrap();
    registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(
            &mut document,
            "OffsetSrf 2 Solid=No BothSides=No DeleteInput=No"
        ),
        Err(CommandError::OffsetSurfaceRequiresPlanarFaces)
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

    let mut warped = Document::default();
    registry
        .execute(&mut warped, "SrfPt 0,0,0 4,0,0 4,3,1 0,3,0")
        .unwrap();
    registry.execute(&mut warped, "SelAll").unwrap();
    let before = warped.objects().cloned().collect::<Vec<_>>();
    let result = registry.execute(
        &mut warped,
        "OffsetSrf 2 Solid=Yes BothSides=No DeleteInput=No",
    );
    #[cfg(not(feature = "native-smlib"))]
    assert!(matches!(
        result,
        Err(CommandError::OffsetSurfaceRequiresPlanarFaces)
    ));
    #[cfg(feature = "native-smlib")]
    {
        result.unwrap();
        let Geometry::Brep(offset) = &result_geometry(&warped) else {
            panic!("native offset wall")
        };
        assert!(offset.is_solid());
        registry.execute(&mut warped, "Undo").unwrap();
    }
    assert_eq!(warped.objects().cloned().collect::<Vec<_>>(), before);
}
