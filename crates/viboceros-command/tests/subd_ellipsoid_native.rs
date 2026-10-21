use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_ellipsoid.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn native_ellipsoid_inputs_caps_and_frames_match_geometry_sources_and_history() {
    let mut registry = CommandRegistry::with_builtins();
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true && !native::getter_only(r))
    {
        if row["fresh"] == true {
            registry = CommandRegistry::with_builtins();
        }
        let value = &row["value"];
        let mut scene = native::setup(value);
        let command = native::command(value, scene.source);
        let result = registry.execute_in_context(
            &mut scene.document,
            &command,
            CommandContext {
                construction_plane: native::plane(value),
            },
        );
        let declined = value["input"]["no_output"] == true;
        if declined {
            assert!(
                matches!(result, Err(viboceros_command::CommandError::Geometry(_))),
                "{command}: {result:?}"
            );
            assert_eq!(scene.document.undo_label(), None);
        } else {
            result.unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
            assert_eq!(scene.document.undo_label(), Some("SubDEllipsoid"));
        }
        let check = native::check(&scene, value, "after_script");
        let saved = scene.document.objects().cloned().collect::<Vec<_>>();
        if !declined {
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            native::check(&scene, value, "redo");
            assert_eq!(scene.document.objects().cloned().collect::<Vec<_>>(), saved);
        }
        println!(
            "SUBD_ELLIPSOID_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true,"declined":declined})
        );
    }
}
#[test]
fn invalid_scripts_leave_geometry_selection_redo_and_all_settings_unchanged() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let p = registry.subd_ellipsoid_preferences();
    let ellipse = registry.ellipse_options();
    let torus = registry.subd_torus_preferences();
    for args in [
        "AroundFaces=2 0,0,0 5,0,0 0,3,0 2",
        "VerticalFaces=1 0,0,0 5,0,0 0,3,0 2",
        "AroundFaces=100000 VerticalFaces=100000 0,0,0 5,0,0 0,3,0 2",
        "AroundFaces=3 CapFaceStyle=Quad 0,0,0 5,0,0 0,3,0 2",
        "CapFaceStyle=Unknown 0,0,0 5,0,0 0,3,0 2",
        "Vertical 0,0,0 5,0,0 0,3,0 2",
        "Deformable 0,0,0 5,0,0 0,3,0 2",
        "Radius 0,0,0 5,0,0 0,3,0 2",
        "0,0,0 5,0,0 0,3,0 Diameter 4",
        "0,0,0 5,0,0 0,3,0 nan",
        "0,0,0 5,0,0 0,3,0",
        "0,0,0 5,0,0 0,3,0 2 trailing",
        "0,0,0 AroundFaces=8 5,0,0 0,3,0 2",
        "1,2,3 6,2,3 0 3 1,5,3 2",
        "FromFoci -4,0,0 4,0,0 0,3,0 2",
    ] {
        let command = format!("SubDEllipsoid {args}");
        assert!(registry.execute(&mut doc, &command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before, "{command}");
        assert_eq!(registry.subd_ellipsoid_preferences(), p);
        assert_eq!(registry.ellipse_options(), ellipse);
        assert_eq!(registry.subd_torus_preferences(), torus);
    }
}
