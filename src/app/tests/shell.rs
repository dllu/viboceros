use super::brep_comparison;
use super::plane_surface_native_support as native;
use super::*;
#[path = "shell/mouse.rs"]
mod mouse;
#[path = "../../../crates/viboceros-command/tests/support/shell.rs"]
#[allow(dead_code)]
mod native_inputs;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 10.], [0., 8.], [0., 6.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
fn pick(app: &mut VibocerosApp, id: ObjectId, index: usize, modifiers: egui::Modifiers) {
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: id,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index,
        }],
        preselection: false,
        modifiers,
    });
}
#[test]
fn shell_face_picker_options_units_and_cancel_preserve_geometry() {
    let mut app = test_app();
    let s = source(&mut app);
    let original = app.document.object(s).unwrap().clone();
    enter(&mut app, "Shell DeleteInput=No");
    assert!(app.shell_prompt.is_some());
    assert!(!app.command_line_idle());
    pick(&mut app, s, 0, Default::default());
    pick(&mut app, s, 1, Default::default());
    pick(
        &mut app,
        s,
        1,
        egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    enter(&mut app, "Thickness");
    assert!(app.shell_prompt.unwrap().awaiting_thickness);
    enter(&mut app, "1cm");
    let options = app.shell_prompt.unwrap().options;
    assert_eq!(options.thickness, 10.);
    enter(&mut app, "gibberish");
    assert!(app.shell_prompt.is_some());
    enter(&mut app, "DeleteInput=Yes Thickness=-1");
    assert_eq!(app.shell_prompt.unwrap().options, options);
    enter(&mut app, "Cancel");
    assert!(app.shell_prompt.is_none());
    assert_eq!(app.document.object(s), Some(&original));
}
#[test]
fn shell_empty_and_stale_faces_keep_the_picker() {
    let mut app = test_app();
    let s = source(&mut app);
    enter(&mut app, "Shell");
    enter(&mut app, "");
    assert!(app.shell_prompt.is_none());
    enter(&mut app, "Shell");
    pick(&mut app, s, 0, Default::default());
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[test]
fn shell_initial_thickness_uses_document_units() {
    let mut app = test_app();
    app.document
        .set_units(viboceros_geometry::LengthUnitSystem::Meters, false)
        .unwrap();
    enter(&mut app, "Shell Thickness=1cm DeleteInput=No");
    let options = app.shell_prompt.unwrap().options;
    assert_eq!(options.thickness, 0.01);
    assert!(!options.delete_input);
    enter(&mut app, "Thickness=2mm");
    assert_eq!(app.shell_prompt.unwrap().options.thickness, 0.002);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().len(), 0);
}
#[test]
fn shell_remembers_options_on_cancel_without_adding_model_history() {
    let mut app = test_app();
    enter(&mut app, "Shell Thickness=2 DeleteInput=No");
    enter(&mut app, "Thickness");
    enter(&mut app, "");
    assert!(!app.shell_prompt.unwrap().awaiting_thickness);
    assert!(!app.document.can_undo());
    enter(&mut app, "Thickness");
    app.cancel_current_prompt_or_selection();
    assert!(!app.shell_prompt.unwrap().awaiting_thickness);
    enter(&mut app, "Cancel");
    assert!(app.shell_prompt.is_none());
    enter(&mut app, "Shell");
    let options = app.shell_prompt.unwrap().options;
    assert_eq!(options.thickness, 2.);
    assert!(!options.delete_input);
    assert!(!app.document.can_undo());
    assert_eq!(
        test_app().commands.shell_options(),
        viboceros_command::shell::ShellOptions::default()
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn shell_portable_failure_retains_face_picks() {
    let mut app = test_app();
    let s = source(&mut app);
    enter(&mut app, "Shell");
    pick(&mut app, s, 0, Default::default());
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn shell_component_preselection_and_thickness_failure_repair_commit_one_history_step() {
    let mut app = test_app();
    let s = source(&mut app);
    let original = app.document.object(s).unwrap().clone();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: s,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: Default::default(),
    });
    enter(&mut app, "Shell Thickness=100");
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.shell_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Thickness=1");
    enter(&mut app, "");
    assert!(app.shell_prompt.is_none());
    assert_eq!(app.document.selected_objects().count(), 0);
    assert!(
        matches!(app.document.object(s).unwrap().geometry(), Geometry::Brep(b) if b.is_solid())
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(s), Some(&original));
    enter(&mut app, "Redo");
    assert_ne!(app.document.object(s), Some(&original));
}

#[test]
fn shell_getters_match_native_geometry_metadata_identity_history_and_archives() {
    let mut count = 0;
    for (id, value) in native_inputs::rows() {
        if native_inputs::diagnostic(&value) {
            continue;
        }
        let mut app = test_app();
        app.document = native_inputs::setup(&value);
        if !native_inputs::supported(&app.document, &value) {
            continue;
        }
        println!("SHELL_UI_CASE {id}");
        let state = native_inputs::State::capture(&app.document);
        native_inputs::check(&app.document, &value["before"], false);
        app.commands
            .remember_shell_options(native_inputs::initial(&value))
            .unwrap();
        enter(&mut app, "Shell");
        let actions = value["prescribed_case"]["actions"].as_array().unwrap();
        let prompts = value["input_trace"][0]["prompts"].as_array().unwrap();
        for (i, action) in actions.iter().enumerate() {
            if action != "Cancel" {
                assert_eq!(
                    app.shell_prompt.unwrap().awaiting_thickness,
                    prompts[i].as_str().unwrap().starts_with("Thickness"),
                    "{id} input {i}"
                );
            }
            if let Some(xyz) = action.get("click") {
                let (object, index) = native_inputs::picked_face(&app.document, xyz);
                pick(&mut app, object, index, Default::default());
            } else {
                let text = if action == "Enter" {
                    String::new()
                } else if let Some(n) = action.as_f64() {
                    n.to_string()
                } else {
                    action
                        .get("number")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| action.as_str())
                        .unwrap()
                        .to_owned()
                };
                if action == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, &text);
                }
            }
        }
        assert!(app.shell_prompt.is_none(), "{id}: {:?}", app.command_log);
        let mut record = state.verify(&mut app.document, &value);
        record["id"] = id.into();
        println!("SHELL_UI {record}");
        count += 1;
    }
    println!("SHELL_UI_COUNT {count}");
}
