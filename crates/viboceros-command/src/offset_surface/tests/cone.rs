//! Qualified cone offset geometry and admission regressions.
use super::*;

#[test]
fn exact_cone_offsets_match_rhino_open_face_geometry() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap(),
        ))
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let original = document.objects().cloned().collect::<Vec<_>>();
    for (distance, apex_x, apex_z) in [
        (0.25, 1.208012573584461, 2.861324950943693),
        (-0.25, 0.791987426415539, 3.138675049056307),
    ] {
        registry
            .execute(
                &mut document,
                &format!("OffsetSrf {distance} DeleteInput=Yes Solid=No BothSides=No"),
            )
            .unwrap();
        let Geometry::NurbsSurface(offset) = &result_geometry(&document) else {
            panic!("open cone offset is a NURBS surface")
        };
        let apex = offset.control_points()[0].point().to_array();
        assert!((apex[0] - apex_x).abs() < 1e-12);
        assert!((apex[2] - apex_z).abs() < 1e-12);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
    }
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    let Geometry::Brep(pair) = &result_geometry(&document) else {
        panic!("two-sided cone offset is a B-rep")
    };
    assert_eq!(
        (
            pair.faces().len(),
            pair.edges().len(),
            pair.vertices().len()
        ),
        (2, 6, 4)
    );
}

#[test]
fn reversed_cone_face_offsets_along_its_reversed_normal() {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let surface = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
    let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
    source.reverse_orientation();
    let options = parse(
        &["0.25"],
        Options::default(),
        &viboceros_geometry::LengthUnitSystem::Millimeters,
    )
    .unwrap()
    .options;
    let Some(Geometry::Brep(offset)) =
        conical_offset(&Geometry::Brep(source), options, tolerance).unwrap()
    else {
        panic!("reversed cone face offsets as a B-rep")
    };
    assert!(offset.faces()[0].is_reversed());
    let apex = offset.faces()[0].surface().control_points()[0].point();
    assert!((apex.x() - 0.791987426415539).abs() < 1e-12);
}

#[test]
fn exact_cone_solid_offsets_match_rhino_shell_topology_and_volume() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap(),
        ))
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    for (command, edges, volume) in [
        (
            "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes BothSides=No",
            7,
            6.252635399278535,
        ),
        (
            "OffsetSrf -0.25 Solid=Yes DeleteInput=Yes BothSides=No",
            7,
            5.074538139644014,
        ),
        (
            "OffsetSrf 0.25 Solid=Yes BothSides=Yes DeleteInput=Yes",
            8,
            11.327173538922558,
        ),
    ] {
        registry.execute(&mut document, command).unwrap();
        let Geometry::Brep(shell) = &result_geometry(&document) else {
            panic!("solid cone offset is a B-rep")
        };
        assert_eq!(
            (
                shell.faces().len(),
                shell.edges().len(),
                shell.vertices().len()
            ),
            (4, edges, 4)
        );
        assert!((shell.signed_volume(document.tolerance()).unwrap() - volume).abs() < 1e-6);
        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
    }
}

#[test]
fn negative_height_cone_solid_offset_closes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_cone(frame, 2.0, -3.0).unwrap(),
        ))
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("solid cone offset is a B-rep")
    };
    assert_eq!(
        (
            shell.faces().len(),
            shell.edges().len(),
            shell.vertices().len()
        ),
        (4, 7, 4)
    );
    assert!((shell.signed_volume(document.tolerance()).unwrap() - 6.252635399278535).abs() < 1e-6);
}
