//! Qualified cylinder offset geometry and admission regressions.
use super::*;

#[test]
fn exact_cylinder_offsets_match_rhino_wall_and_tube_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cylinder 1,2,3 2 3 Solid=No")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let source = document.objects().cloned().collect::<Vec<_>>();

    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let output = result_geometry(&document);
    let offset = output.nurbs_surface_representation().unwrap().unwrap();
    let (_, radius, height) = offset
        .canonical_cylinder(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((radius - 2.5).abs() < 1e-10);
    assert!((height - 3.0).abs() < 1e-10);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -0.5 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let output = result_geometry(&document);
    let offset = output.nurbs_surface_representation().unwrap().unwrap();
    let (_, radius, _) = offset
        .canonical_cylinder(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((radius - 1.5).abs() < 1e-10);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    let Geometry::Brep(open_pair) = &result_geometry(&document) else {
        panic!("two-sided cylindrical offset is a B-rep")
    };
    assert_eq!(
        (
            open_pair.faces().len(),
            open_pair.edges().len(),
            open_pair.vertices().len()
        ),
        (2, 6, 4)
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(tube) = &result_geometry(&document) else {
        panic!("cylindrical solid offset is a tube")
    };
    assert!(
        (tube.signed_volume(document.tolerance()).unwrap() - 6.75 * std::f64::consts::PI).abs()
            < 1e-7
    );
    assert_eq!(
        (
            tube.faces().len(),
            tube.edges().len(),
            tube.vertices().len()
        ),
        (4, 8, 4)
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 Solid=Yes BothSides=Yes DeleteInput=Yes",
        )
        .unwrap();
    let Geometry::Brep(tube) = &result_geometry(&document) else {
        panic!("two-sided cylindrical solid offset is a tube")
    };
    assert!(
        (tube.signed_volume(document.tolerance()).unwrap() - 12.0 * std::f64::consts::PI).abs()
            < 1e-7
    );
    assert_eq!(
        (
            tube.faces().len(),
            tube.edges().len(),
            tube.vertices().len()
        ),
        (4, 8, 4)
    );
}

#[test]
fn collapsing_cylinder_offset_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cylinder 1,2,3 2 3 Solid=No")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(
            &mut document,
            "OffsetSrf -2.5 DeleteInput=Yes Solid=No BothSides=No"
        ),
        Err(CommandError::OffsetSurfaceCollapsedCylinder)
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn reversed_cylindrical_brep_offsets_inward() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let surface = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 3.0).unwrap();
    let mut source = Brep::try_surface_face(surface, document.tolerance()).unwrap();
    source.reverse_orientation();
    document.add_geometry(Geometry::Brep(source)).unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(offset) = &result_geometry(&document) else {
        panic!("cylindrical B-rep offset remains a B-rep")
    };
    let (_, radius, height) = offset.faces()[0]
        .surface()
        .canonical_cylinder(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((radius - 1.5).abs() < 1e-10);
    assert!((height - 3.0).abs() < 1e-10);
    assert!(offset.faces()[0].is_reversed());
}
