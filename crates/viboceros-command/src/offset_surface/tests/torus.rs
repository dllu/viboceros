//! Qualified torus offset geometry and admission regressions.
use super::*;

#[test]
fn exact_torus_offsets_keep_source_domains_and_shell_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Torus 1,2,3 4 1").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let source = document.objects().cloned().collect::<Vec<_>>();
    let Geometry::NurbsSurface(original) = source[0].geometry() else {
        panic!("torus source is a NURBS surface")
    };
    let domains = (original.domain_u(), original.domain_v());

    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::NurbsSurface(offset) = &result_geometry(&document) else {
        panic!("open toroidal offset is a NURBS surface")
    };
    let (_, major, minor) = offset
        .canonical_torus(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((major - 4.0).abs() < 1e-10);
    assert!((minor - 1.25).abs() < 1e-10);
    assert_eq!((offset.domain_u(), offset.domain_v()), domains);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf -0.25 DeleteInput=Yes Solid=No BothSides=No",
        )
        .unwrap();
    let Geometry::NurbsSurface(offset) = &result_geometry(&document) else {
        panic!("inward toroidal offset is a NURBS surface")
    };
    let (_, _, minor) = offset
        .canonical_torus(document.tolerance())
        .unwrap()
        .unwrap();
    assert!((minor - 0.75).abs() < 1e-10);
    assert_eq!((offset.domain_u(), offset.domain_v()), domains);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 BothSides=Yes DeleteInput=Yes Solid=No",
        )
        .unwrap();
    let Geometry::Brep(open_pair) = &result_geometry(&document) else {
        panic!("two-sided toroidal offset is a B-rep")
    };
    assert_eq!(
        (
            open_pair.faces().len(),
            open_pair.edges().len(),
            open_pair.vertices().len()
        ),
        (2, 4, 2)
    );
    assert!(
        open_pair
            .faces()
            .iter()
            .all(|face| { (face.surface().domain_u(), face.surface().domain_v()) == domains })
    );
    let summed_volume =
        2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) + 0.75_f64.powi(2));
    assert!((open_pair.signed_volume(document.tolerance()).unwrap() - summed_volume).abs() < 1e-6);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes BothSides=No",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("toroidal solid offset is a B-rep")
    };
    assert_eq!(
        (
            shell.faces().len(),
            shell.edges().len(),
            shell.vertices().len()
        ),
        (2, 4, 2)
    );
    let shell_volume = 2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) - 1.0);
    assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-6);
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "OffsetSrf 0.25 Solid=Yes BothSides=Yes DeleteInput=Yes",
        )
        .unwrap();
    let Geometry::Brep(shell) = &result_geometry(&document) else {
        panic!("two-sided toroidal solid offset is a B-rep")
    };
    assert_eq!(
        (
            shell.faces().len(),
            shell.edges().len(),
            shell.vertices().len()
        ),
        (2, 4, 2)
    );
    let shell_volume =
        2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) - 0.75_f64.powi(2));
    assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-6);
}

#[test]
fn collapsing_or_self_intersecting_torus_offset_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Torus 1,2,3 4 1").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    for distance in ["-1.25", "3.5"] {
        assert!(matches!(
            registry.execute(
                &mut document,
                &format!("OffsetSrf {distance} DeleteInput=Yes Solid=No BothSides=No")
            ),
            Err(CommandError::OffsetSurfaceCollapsedTorus)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn reversed_toroidal_face_offsets_inward_along_its_normal() {
    let tolerance = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let surface = NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap();
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
        toroidal_offset(&Geometry::Brep(source), options, tolerance).unwrap()
    else {
        panic!("reversed toroidal face offsets as a B-rep")
    };
    let (_, major, minor) = offset.faces()[0]
        .surface()
        .canonical_torus(tolerance)
        .unwrap()
        .unwrap();
    assert!((major - 4.0).abs() < 1e-10);
    assert!((minor - 0.75).abs() < 1e-10);
    assert!(offset.faces()[0].is_reversed());
}
