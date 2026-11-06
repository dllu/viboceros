use viboceros_command::{CommandRegistry, plane_surface::Preferences};
use viboceros_document::{Document, Geometry};

#[test]
fn deformable_settings_are_per_registry_and_plain_output_stays_bilinear() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(
            &mut doc,
            "Plane Deformable UDegree=5 UPointCount=6 VDegree=2 VPointCount=4 0,0 8,5",
        )
        .unwrap();
    assert_eq!(
        registry.plane_preferences(),
        Preferences {
            degrees: [5, 2],
            counts: [6, 4]
        }
    );
    registry.execute(&mut doc, "Plane 0,0 8,5").unwrap();
    let Geometry::NurbsSurface(s) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!([s.degree_u(), s.degree_v()], [1, 1]);
    registry
        .execute(&mut doc, "Plane Deformable 0,0 8,5")
        .unwrap();
    let Geometry::NurbsSurface(s) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!([s.degree_u(), s.degree_v()], [5, 2]);
    assert_eq!(
        CommandRegistry::with_builtins().plane_preferences(),
        Preferences::default()
    );
}

#[test]
fn invalid_or_incomplete_plane_scripts_preserve_document_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Point 4,5,6").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for script in [
        "Plane 0,0 0,5",
        "Plane 3Point 0,0 0,0 0,1",
        "Plane Rounded 0,0 8,5",
        "Plane 0,0 8,5 9,9",
        "Plane Deformable UDegree=1 0,0 8,5",
        "Plane Deformable VDegree=12 0,0 8,5",
        "Plane Deformable UPointCount=3 0,0 8,5",
        "Plane Deformable UPointCount=1000000 0,0 8,5",
        "Plane Deformable UDegree",
        "Plane UDegree=3 0,0 8,5",
        "Plane AroundCurve Curve=bad 0,0 8 5",
    ] {
        assert!(registry.execute(&mut doc, script).is_err(), "{script}");
        assert_eq!(format!("{doc:?}"), before, "{script}");
        assert_eq!(registry.plane_preferences(), Preferences::default());
    }
}
