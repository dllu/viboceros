//! Qualified sphere offset geometry and admission regressions.
use super::*;

#[test]
fn exact_sphere_offsets_match_rhino_shell_volumes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Sphere 1,2,3 2").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let source = document.objects().cloned().collect::<Vec<_>>();

    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::NurbsSurface(offset) = &result_geometry(&document) else {
        panic!("open spherical offset is a NURBS surface")
    };
    let (_, radius) = offset
        .canonical_sphere(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((radius - 2.5).abs() < 1e-10);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -0.5 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::NurbsSurface(offset) = &result_geometry(&document) else {
        panic!("inward spherical offset is a NURBS surface")
    };
    let (_, radius) = offset
        .canonical_sphere(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((radius - 1.5).abs() < 1e-10);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("one-sided sphere solid offset is a B-rep")
    };
    assert_eq!((shell.faces().len(), shell.edges().len()), (2, 2));
    let one_side_volume = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 2.0_f64.powi(3));
    assert!((shell.signed_volume(document.tolerance()).unwrap() - one_side_volume).abs() < 1e-7);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    let Geometry::Brep(open_pair) = &result_geometry(&document) else {
        panic!("two-sided sphere offset is a B-rep")
    };
    assert_eq!((open_pair.faces().len(), open_pair.edges().len()), (2, 2));
    let summed_volume = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) + 1.5_f64.powi(3));
    assert!((open_pair.signed_volume(document.tolerance()).unwrap() - summed_volume).abs() < 1e-7);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 Solid=Yes BothSides=Yes DeleteInput=Yes",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("spherical solid offset is a B-rep")
    };
    assert_eq!((shell.faces().len(), shell.edges().len()), (2, 2));
    let shell_volume = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 1.5_f64.powi(3));
    assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-7);
}

#[test]
fn collapsing_sphere_offset_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Sphere 1,2,3 2").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(
            &mut document,
            "OffsetSrf -2.5 DeleteInput=Yes Solid=No BothSides=No"
        ),
        Err(CommandError::OffsetSurfaceCollapsedSphere)
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn untrimmed_spherical_brep_offsets_as_exact_shell() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
    let source = Brep::try_surface_face(surface, document.tolerance()).unwrap();
    document.add_geometry(Geometry::Brep(source)).unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("spherical B-rep offset is an exact shell")
    };
    assert_eq!(shell.faces().len(), 2);
    let expected = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 2.0_f64.powi(3));
    assert!((shell.signed_volume(document.tolerance()).unwrap() - expected).abs() < 1e-7);
}

#[test]
fn reversed_spherical_face_offsets_inward_along_its_normal() {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
    let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
    source.reverse_orientation();
    let options = parse(
        &["0.5"],
        Options::default(),
        &viboceros_geometry::LengthUnitSystem::Millimeters,
    )
    .unwrap()
    .options;
    let Some(Geometry::Brep(offset)) =
        spherical_offset(&Geometry::Brep(source), options, tolerance).unwrap()
    else {
        panic!("reversed spherical face offsets as a B-rep")
    };
    let (_, radius) = offset.faces()[0]
        .surface()
        .canonical_sphere(tolerance)
        .unwrap()
        .unwrap();
    assert!((radius - 1.5).abs() < 1e-10);
    assert!(offset.faces()[0].is_reversed());
}
