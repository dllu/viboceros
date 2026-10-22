use super::*;

#[test]
fn cylinder_creates_exact_signed_and_both_sides_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cylinder 1,2,3 2.5 -4 Solid=No")
        .unwrap();
    let cylinder_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(cylinder_id));
    let surface = document
        .object(cylinder_id)
        .unwrap()
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 2);
    assert_eq!(surface.domain_u(), 0.0..=2.5 * std::f64::consts::TAU);
    assert_eq!(surface.domain_v(), 0.0..=4.0);
    assert_eq!(
        surface.control_point(0, 0).unwrap().point(),
        Point3::try_new(3.5, 2.0, 3.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Cylinder"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "_Cyl 1,2,3 4,7,3 4 Axis=0,1,0 BothSides=Yes Solid=No",
        )
        .unwrap();
    let both_sides = document
        .objects()
        .next()
        .unwrap()
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    assert_eq!(both_sides.domain_v(), 0.0..=8.0);
    assert_eq!(
        both_sides.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(4.0, -2.0, 3.0).unwrap()
    );
    assert_eq!(
        both_sides.evaluate(0.0, 8.0).unwrap(),
        Point3::try_new(4.0, 6.0, 3.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(&mut document, "Cylinder 1,2,3 2.5 -4 Solid=Yes")
        .unwrap();
    assert!(result.contains("closed extrusion cylinder"));
    let Geometry::Brep(solid) = document.objects().next().unwrap().geometry() else {
        panic!("Cylinder Solid=Yes must create a B-rep")
    };
    assert_eq!(solid.faces().len(), 3);
    assert!(solid.is_solid());
    let mesh = solid.tessellate(8, document.tolerance()).unwrap();
    assert!(mesh.topology().is_solid());
    let expected_volume = std::f64::consts::PI * 2.5 * 2.5 * 4.0;
    assert!((mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume < 0.01);

    for invalid in [
        "Cylinder 0,0,0 1 0",
        "Cylinder 0,0,0 0 2",
        "Cylinder 0,0,0 0,0,2 3 Axis=0,0,1",
        "Cylinder 0,0,0 1 2 Axis=0,0,0",
        "Cylinder 0,0,0 1 2 BothSides=Maybe",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Cylinder"));
}
