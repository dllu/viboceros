use super::*;
use viboceros_oracle::use_extrusions::snapshot;
#[path = "../../../crates/viboceros-command/tests/support/use_extrusions.rs"]
mod creators;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn use_extrusions_prompt_matches_native_options_cancel_and_redo() {
    let native = creators::read("use_extrusions");
    for row in native["results"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let (initial, answer) = id.split_once('_').unwrap();
        let mut app = test_app();
        enter(&mut app, &format!("UseExtrusions {initial}"));
        enter(&mut app, "Point 1,2,3");
        enter(&mut app, "SelAll");
        app.document.clear_history().unwrap();
        enter(&mut app, "Point 10,20,30");
        enter(&mut app, "Undo");
        assert_eq!(
            snapshot(&app.document, &app.commands),
            row["value"]["before"],
            "{id}"
        );
        enter(&mut app, "_UseExtrusions");
        assert!(app.use_extrusions_prompt);
        assert!(!app.command_line_idle());
        enter(&mut app, "invalid");
        assert!(app.use_extrusions_prompt);
        if answer == "Cancel" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if answer == "Enter" { "" } else { answer });
        }
        assert!(!app.use_extrusions_prompt);
        assert!(app.command_line_idle());
        assert_eq!(
            snapshot(&app.document, &app.commands),
            row["value"]["after"],
            "{id}"
        );
        enter(&mut app, "Redo");
        assert_eq!(
            snapshot(&app.document, &app.commands),
            row["value"]["redo"],
            "{id}"
        );
    }
    let mut app = test_app();
    enter(&mut app, "UseExtrusions");
    enter(&mut app, "_Cancel");
    assert!(!app.use_extrusions_prompt);
    assert!(app.commands.use_extrusions());
    enter(&mut app, "UseExtrusions");
    enter(&mut app, "Point 3,4,5");
    assert!(!app.use_extrusions_prompt);
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn use_extrusions_prompt_changes_all_supported_creators() {
    for family in [
        "use_extrusions_creators",
        "use_extrusions_creators_limits",
        "use_extrusions_creators_domains",
    ] {
        replay_creators(family);
    }
}
fn replay_creators(family: &str) {
    let native = creators::read(family);
    for row in native["results"].as_array().unwrap() {
        let v = &row["value"];
        let label = row["id"].as_str().unwrap();
        let (mut doc, source, context) = creators::setup(v);
        doc.clear_history().unwrap();
        let mut app = test_app();
        app.document = doc;
        // Geometry input is already complete; use the app command executor so
        // every creator receives the same shared policy as interactive getters.
        enter(&mut app, "UseExtrusions");
        enter(&mut app, label.split_once('_').unwrap().0);
        if !cfg!(feature = "native-smlib") && v["input"]["command"] == "ExtrudeSrf" {
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            assert!(!app.try_execute_command_with_plane(
                v["input"]["application"].as_str().unwrap(),
                Some(context.construction_plane)
            ));
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(app.document.undo_label(), None);
            continue;
        }
        assert!(
            app.try_execute_command_with_plane(
                v["input"]["application"].as_str().unwrap(),
                Some(context.construction_plane)
            ),
            "{label}: {:?}",
            app.command_log
        );
        creators::check(&app.document, source, v, label);
    }
}
