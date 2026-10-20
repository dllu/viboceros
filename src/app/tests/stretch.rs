use super::*;
use crate::deformation_test_support as support;
use viboceros_command::stretch::Target;

#[test]
fn stretch_point_prompts_match_native_selection_and_history() {
    workflows("stretch_command", 16);
}
#[test]
fn stretch_geometry_prompts_match_native_copies_groups_and_history() {
    workflows("stretch_geometry_command", 57);
}
#[test]
fn stretch_grip_prompts_match_native_display_and_history() {
    workflows("stretch_grips", 46);
}
fn workflows(kind: &str, expected_count: usize) {
    let mut count = 0;
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        if op["op"] != kind {
            continue;
        }
        let label = op["id"].as_str().unwrap().to_owned();
        let result = std::panic::catch_unwind(|| {
            let mut app = test_app();
            let (doc, sources) = support::setup(&op, &value);
            app.document = doc;
            sources.check(&app.document, &op, &value, "before");
            assert!(app.try_start_interactive_command("Stretch"), "{label}");
            if op["preselect"] == false {
                assert!(app.object_prompt.is_some());
                app.document
                    .select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)
                    .unwrap();
                assert!(app.try_continue_transform_source_prompt("Enter"));
            }
            let options = support::options(&op);
            for word in options.arguments().split_whitespace() {
                assert!(app.try_continue_stretch(word));
            }
            let (start, end) = support::axis(&op);
            assert!(app.accept_drafting_point(start), "{label}");
            let targets = support::targets(&op);
            let early = op["early_factor"] == true;
            if early {
                assert!(app.try_continue_stretch(&targets[0].argument()));
            }
            assert!(app.accept_drafting_point(end), "{label}");
            if !early {
                for target in targets {
                    match target {
                        Target::Point(p) => assert!(app.accept_drafting_point(p), "{label}"),
                        _ => assert!(app.try_continue_stretch(&target.argument()), "{label}"),
                    }
                }
            }
            if app.active_command.is_some() {
                assert!(app.try_continue_stretch(
                    if op["finish"] == "Cancel" || op["id"] == "identity_1" {
                        "Cancel"
                    } else {
                        "Enter"
                    }
                ));
            }
            assert!(app.active_command.is_none());
            assert!(app.stretch_session.is_none());
            let mut gap = sources.check(&app.document, &op, &value, "after");
            let undo = &value["undo"];
            let undo_objects = if undo.is_array() {
                undo.as_array().unwrap().len()
            } else {
                undo["objects"].as_array().map_or(0, Vec::len)
            };
            if !undo.is_null() && undo_objects > 0 {
                assert_eq!(app.document.undo_label(), Some("Stretch"));
                app.execute_command("Undo");
                gap = gap.max(sources.check(&app.document, &op, &value, "undo"));
                app.execute_command("Redo");
                gap = gap.max(sources.check(&app.document, &op, &value, "redo"));
            } else {
                assert!(app.document.undo_label().is_none());
            }
            eprintln!(
                "STRETCH_UI_AUDIT {}",
                serde_json::json!({"id":label,"kind":op["op"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(label);
        }
        count += 1;
    }
    assert_eq!(count, expected_count);
    assert!(failures.is_empty(), "Stretch UI differences: {failures:?}");
}

#[test]
fn stretch_typed_lengths_early_factors_invalid_axes_and_cancellation() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.document.clear_history().unwrap();
    let original = app.document.objects().next().unwrap().geometry().clone();
    assert!(app.try_start_interactive_command("Stretch"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.try_continue_stretch("Copy"));
    assert!(app.try_continue_stretch("Yes"));
    assert!(app.try_continue_stretch("2"));
    assert!(app.accept_drafting_point(point(0., 0., 10.)));
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.try_continue_stretch("Length"));
    assert!(!app.accept_drafting_point(point(0., 0., 15.)));
    assert!(app.try_continue_stretch("2cm"));
    assert_eq!(app.document.objects().count(), 3);
    assert!(app.try_continue_stretch("Cancel"));
    assert!(app.active_command.is_none());
    assert!(app.commands.stretch_options_default().copy);
    assert!(!app.commands.stretch_options_default().rigid);
    assert_eq!(app.commands.stretch_factor_default(), 2.);
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.objects().next().unwrap().geometry(), &original);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn stretch_hover_never_changes_geometry_and_clears_after_placements() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.document.clear_history().unwrap();
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.try_start_interactive_command("Stretch"));
    for p in [point(0., 0., 0.), point(0., 0., 10.)] {
        assert!(app.accept_drafting_point(p));
    }
    let p = point(0., 0., 20.);
    assert!(app.update_stretch_preview(Some(p)));
    assert!(!app.update_stretch_preview(Some(p)));
    assert_eq!(app.stretch_preview().unwrap().last_point, Some(p));
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert!(!app.document.can_undo());
    assert!(app.try_continue_stretch("Copy=Yes"));
    assert!(app.accept_drafting_point(p));
    assert_eq!(app.stretch_preview().unwrap().last_point, None);
    assert!(app.update_stretch_preview(Some(point(0., 0., 15.))));
    assert!(app.try_continue_stretch("Cancel"));
    assert!(app.stretch_preview().is_none());
    app.execute_command("Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
}

#[test]
fn stretch_copy_and_factor_defaults_survive_native_completion_and_escape() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/stretch_preferences.json"
    ))
    .unwrap();
    let r: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/stretch_preferences.json"
    ))
    .unwrap();
    let mut app = test_app();
    let mut factor = 1.;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let value = &row["value"];
        let (doc, sources) = support::setup(op, value);
        app.document = doc;
        assert_eq!(app.commands.stretch_factor_default(), factor);
        if op["id"].as_str().unwrap().starts_with("following_") {
            assert!(app.commands.stretch_options_default().copy);
            assert!(!app.commands.stretch_options_default().rigid);
            assert!(
                value["history"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("<{factor}>"))
            );
        }
        assert!(app.try_start_interactive_command("Stretch"));
        for word in support::options(op).arguments().split_whitespace() {
            assert!(app.try_continue_stretch(word));
        }
        let (a, b) = support::axis(op);
        for p in [a, b] {
            assert!(app.accept_drafting_point(p));
        }
        for target in support::targets(op) {
            assert!(app.try_continue_stretch(&target.argument()));
            factor = target.factor(a, b).unwrap();
        }
        if app.active_command.is_some() {
            assert!(app.try_continue_stretch(if op["finish"] == "Cancel" {
                "Cancel"
            } else {
                "Enter"
            }));
        }
        let gap = sources.check(&app.document, op, value, "after");
        app.execute_command("Undo");
        sources.check(&app.document, op, value, "undo");
        app.execute_command("Redo");
        sources.check(&app.document, op, value, "redo");
        assert_eq!(app.commands.stretch_factor_default(), factor);
        eprintln!(
            "STRETCH_PREFERENCES_AUDIT {}",
            serde_json::json!({"id":op["id"],"maximum_error":gap,"factor":factor})
        );
    }
}
