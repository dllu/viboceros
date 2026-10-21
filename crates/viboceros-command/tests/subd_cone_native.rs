use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_cone.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn cone_modes_caps_angles_and_axes_match_native_geometry_sources_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let command = native::command(value, scene.source);
        registry
            .execute_in_context(
                &mut scene.document,
                &command,
                CommandContext {
                    construction_plane: native::plane(value),
                },
            )
            .unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), Some("SubDCone"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_CONE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_cone_inputs_preserve_the_model_redo_and_independent_preferences() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let prefs = registry.subd_cone_preferences();
    assert_eq!(prefs.height, 10.);
    for cmd in [
        "SubDCone 0,0,0 -3 5",
        "SubDCone 0,0,0 0 5",
        "SubDCone 0,0,0 3 0",
        "SubDCone VerticalFaces=1 0,0,0 3 5",
        "SubDCone AroundFaces=2 0,0,0 3 5",
        "SubDCone AroundFaces=100000 VerticalFaces=100000 0,0,0 3 5",
        "SubDCone 0,0,0 3 BothSides=Yes 5",
        "SubDCone 0,0,0 Orientation 0,0,1 3 5",
        "SubDCone AroundCurve 0,0,0 3 5",
        "SubDCone 0,0,0 3 OrBaseAngle=0",
        "SubDCone 0,0,0 3 OrBaseAngle=90",
        "SubDCone 0,0,0 3 OrApexAngle=0",
        "SubDCone 0,0,0 3 OrApexAngle=180",
        "SubDCone 0,0,0 3 5 trailing",
    ] {
        assert!(registry.execute(&mut doc, cmd).is_err(), "{cmd}");
        assert_eq!(format!("{doc:?}"), before, "{cmd}");
        assert_eq!(registry.subd_cone_preferences(), prefs);
    }
    registry
        .execute(
            &mut doc,
            "SubDCone AroundFaces=7 VerticalFaces=3 0,0,0 Diameter 6 -5",
        )
        .unwrap();
    let cone = registry.subd_cone_preferences();
    assert_eq!((cone.around_faces, cone.vertical_faces), (7, 3));
    assert_eq!(cone.height, -5.);
    assert!(cone.diameter);
    assert_eq!(registry.subd_cylinder_preferences(), Default::default());
    assert_eq!(registry.subd_sphere_preferences(), Default::default());
}
