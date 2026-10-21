use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_cylinder.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn subd_cylinders_match_native_caps_radius_placement_axes_and_history() {
    for row in native::rows().into_iter().filter(|r| {
        r["value"]["input"]["unsupported"] != true && r["value"]["input"]["cancel"] != true
    }) {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute_in_context(
                &mut scene.document,
                &native::command(value, scene.source),
                CommandContext {
                    construction_plane: native::plane(value),
                },
            )
            .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), Some("SubDCylinder"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_CYLINDER_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_cylinders_preserve_redo_and_command_preferences() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for command in [
        "SubDCylinder AroundFaces=2 0,0,0 3 5",
        "SubDCylinder VerticalFaces=1 0,0,0 3 5",
        "SubDCylinder AroundFaces=100000 VerticalFaces=100000 0,0,0 3 5",
        "SubDCylinder 0,0,0 0 5",
        "SubDCylinder 0,0,0 3 0",
        "SubDCylinder 0,0,0 3 5 extra",
        "SubDCylinder Tangent",
        "SubDCylinder FitPoints",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before);
    }
    assert_eq!(registry.subd_cylinder_preferences(), Default::default());
}

#[test]
fn remembered_sizes_diameter_and_direction_options_are_independent() {
    use viboceros_command::subd_cylinder::{Direction, Input, Preferences};
    let plane = CommandContext::default().construction_plane;
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "SubDCylinder 0,0,0 Diameter 6 -5")
        .unwrap();
    let p = registry.subd_cylinder_preferences();
    assert_eq!((p.radius, p.height, p.diameter), (3., -5., true));
    registry
        .execute(&mut doc, "SubDCylinder 2Point -3,0,0 3,0,0 4")
        .unwrap();
    assert!(registry.subd_cylinder_preferences().diameter);
    registry
        .execute(&mut doc, "SubDCylinder 1,2,3 Enter Enter")
        .unwrap();
    let p = registry.subd_cylinder_preferences();
    assert_eq!((p.radius, p.height, p.diameter), (3., 4., true));
    let mut input = Input::new(plane, Preferences::default());
    input.point(plane.origin()).unwrap();
    input.number(3.).unwrap();
    input.option("BothSides=Yes").unwrap();
    input.option("DirectionConstraint=None").unwrap();
    assert_eq!(input.preferences.direction, Direction::None);
    assert!(!input.preferences.both_sides);
    let mut area = Input::new(plane, p);
    area.point(plane.origin()).unwrap();
    area.option("Area").unwrap();
    assert!(area.enter().is_err());
}
