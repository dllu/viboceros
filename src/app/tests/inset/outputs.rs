use super::*;

#[test]
fn inset_curve_output_options_before_picking_preserve_original_face_picks() {
    let native = common::cases_for("inset_surface_ready")
        .into_iter()
        .find(|(op, _)| op["id"] == "rectangle_ring_curves")
        .unwrap()
        .1;
    let mut app = test_app();
    let (doc, id) = native_surface::fixture(&native);
    app.document = doc;
    let before = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "Inset Split=No");
    assert!(app.picking_inset());
    app.accept_component_window(ComponentWindow {
        picks: vec![ComponentPick {
            object: id,
            index: 0,
            kind: ComponentSelectionKind::BrepFace,
        }],
        preselection: false,
        modifiers: Default::default(),
        crossing: false,
        inverted: false,
    });
    enter(&mut app, "");
    assert!(app.command_log.back().unwrap().contains("Distance=1"));
    assert!(app.command_log.back().unwrap().contains("Split=No"));
    assert!(!app.command_log.back().unwrap().contains("JoinOutputs"));
    enter(&mut app, ".3");
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.object(id).unwrap().geometry_snapshot(),
        &before
    );
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn inset_output_previews_native_identity_components_attributes_and_history() {
    let cases = common::cases_for("inset_surface_ready")
        .into_iter()
        .chain(common::cases_for("inset_output_modes"))
        .chain(common::cases_for("inset_cusp_command"))
        .filter(|(op, _)| {
            matches!(
                op["id"].as_str().unwrap(),
                "rectangle_ring_curves"
                    | "rectangle_crease_pieces"
                    | "box_pair_crease_curves"
                    | "box_pair_crease_pieces"
                    | "ellipse_True_True_curves"
                    | "ellipse_False_False_pieces"
                    | "stadium_False_True_curves"
                    | "holes_True_False_pieces"
                    | "ellipse_plain_3.0_False_curves"
                    | "ellipse_tilted_3.0_True_pieces"
                    | "bezier_plain_2.8_True_pieces"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 11);
    for (op, native) in cases {
        let label = op["id"].as_str().unwrap();
        let mut app = test_app();
        let (doc, id) = native_surface::fixture(&native);
        app.document = doc;
        if native["before"][0]["selected"] == true {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        } else {
            for f in native["before"][0]["selected_faces"].as_array().unwrap() {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: id,
                        index: f.as_u64().unwrap() as usize,
                        kind: ComponentSelectionKind::BrepFace,
                    }],
                    preselection: true,
                    modifiers: Default::default(),
                });
            }
        }
        let before = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, "Inset");
        enter(
            &mut app,
            &format!(
                "CreaseCorners={} Loose={} Split={} JoinOutputs=No",
                if op["crease_corners"] == true {
                    "Yes"
                } else {
                    "No"
                },
                if op["loose"] == true { "On" } else { "Off" },
                if op["split"] == true { "Yes" } else { "No" }
            ),
        );
        enter(
            &mut app,
            &op["distance"].as_f64().unwrap_or(0.3).to_string(),
        );
        assert!(!app.inset_session.as_ref().unwrap().edges().is_empty());
        assert_eq!(
            app.document.object(id).unwrap().geometry_snapshot(),
            &before
        );
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert!(app.inset_session.is_none());
        let picks = app
            .component_selection
            .checked_picks(&app.document)
            .unwrap();
        if op["split"] == false {
            assert_eq!(
                picks.iter().map(|p| p.index).collect::<Vec<_>>(),
                native["after"][0]["selected_faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize)
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(picks.is_empty());
        }
        let epsilon = native_surface::epsilon_for(&op);
        output_support::assert_document(&app.document, id, &native["after"], epsilon, label);
        enter(&mut app, "SelNone");
        output_support::assert_document(&app.document, id, &native["after_script"], epsilon, label);
        enter(&mut app, "Undo");
        output_support::assert_document(&app.document, id, &native["undo"], epsilon, label);
        enter(&mut app, "Redo");
        output_support::assert_document(&app.document, id, &native["redo"], epsilon, label);
    }
}

#[test]
fn inset_output_native_preferences_cancel_and_hidden_join_option_replay() {
    let native = common::cases_for("inset_output_preferences").remove(0).1;
    let mut app = test_app();
    for (i, step) in native["steps"].as_array().unwrap().iter().enumerate() {
        let input = &step["input"];
        let expected = &step["value"];
        let (doc, id) = native_surface::fixture(expected);
        app.document = doc;
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let defaults = app.commands.inset_surface_options();
        let first = expected["history"]
            .as_str()
            .unwrap()
            .lines()
            .nth(1)
            .unwrap();
        assert!(first.contains(if defaults.split {
            "Split=Yes"
        } else {
            "Split=No"
        }));
        if defaults.split {
            assert!(first.contains(if defaults.join_outputs {
                "JoinOutputs=Yes"
            } else {
                "JoinOutputs=No"
            }));
        }
        enter(&mut app, "Inset");
        assert!(
            app.command_log
                .back()
                .unwrap()
                .starts_with(&defaults.surface_command())
        );
        if !defaults.split {
            assert!(!app.command_log.back().unwrap().contains("JoinOutputs"));
        }
        if let Some(split) = input["split"].as_bool() {
            enter(&mut app, if split { "Split=Yes" } else { "Split=No" });
        }
        if let Some(join) = input["join_outputs"].as_bool() {
            enter(
                &mut app,
                if join {
                    "JoinOutputs=Yes"
                } else {
                    "JoinOutputs=No"
                },
            );
        }
        let canceled = input["cancel"] == true;
        if canceled {
            enter(&mut app, "Cancel");
            assert_eq!(app.commands.inset_surface_options(), defaults);
        } else {
            enter(&mut app, ".3");
            enter(&mut app, "");
        }
        assert!(app.inset_session.is_none());
        let label = format!("output preference {i}");
        output_support::assert_document(&app.document, id, &expected["after"], 2e-6, &label);
        if !canceled {
            enter(&mut app, "SelNone");
            enter(&mut app, "Undo");
            output_support::assert_document(&app.document, id, &expected["undo"], 2e-6, &label);
            enter(&mut app, "Redo");
            output_support::assert_document(&app.document, id, &expected["redo"], 2e-6, &label);
        } else {
            assert!(!app.document.can_undo());
        }
    }
}
