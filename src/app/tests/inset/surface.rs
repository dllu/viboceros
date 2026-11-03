use super::*;

#[test]
fn inset_surface_native_preselection_previews_selection_and_history() {
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_ready")
        .into_iter()
        .chain(common::cases_for("inset_planar_curves_command"))
    {
        if op["join_outputs"] != true {
            continue;
        }
        let label = op["id"].as_str().unwrap();
        let mut app = test_app();
        let (doc, id) = native_surface::fixture(&native);
        app.document = doc;
        if native["before"][0]["selected"] == true {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        } else {
            for i in native["before"][0]["selected_faces"].as_array().unwrap() {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: id,
                        index: i.as_u64().unwrap() as usize,
                        kind: ComponentSelectionKind::BrepFace,
                    }],
                    preselection: true,
                    modifiers: Default::default(),
                });
            }
        }
        native_surface::assert_metadata(&app.document, id, &native["before"][0]);
        let before = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, "Inset");
        assert!(!app.picking_inset(), "{label}: {:?}", app.command_log);
        assert!(app.command_log.back().unwrap().contains("Distance=1 "));
        enter(
            &mut app,
            if op["crease_corners"] == true {
                "CreaseCorners=Yes"
            } else {
                "CreaseCorners=No"
            },
        );
        enter(&mut app, ".3");
        assert!(!app.inset_session.as_ref().unwrap().edges().is_empty());
        assert_eq!(
            app.document.object(id).unwrap().geometry_snapshot(),
            &before
        );
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert!(app.inset_session.is_none());
        assert_eq!(app.document.undo_label(), Some("Inset"));
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        native_surface::assert_metadata(&app.document, id, &native["after"][0]);
        let Geometry::Brep(b) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        native_surface::assert_geometry_with_epsilon(
            b,
            &native["after"][0]["brep"],
            app.document.tolerance(),
            native_surface::epsilon_for(&op),
            label,
        );
        let after = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, "SelNone");
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.object(id).unwrap().geometry_snapshot(),
            &before
        );
        native_surface::assert_metadata(&app.document, id, &native["undo"][0]);
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id).unwrap().geometry_snapshot(), &after);
        native_surface::assert_metadata(&app.document, id, &native["redo"][0]);
        covered += 1;
    }
    assert_eq!(covered, 30);
}

#[test]
fn inset_surface_command_first_cancel_stale_and_remembered_options() {
    let native = common::cases_for("inset_surface_sdk").remove(0).1;
    let mut app = test_app();
    let (doc, id) = native_surface::fixture(&native["before"]);
    app.document = doc;
    let pick = ComponentPick {
        object: id,
        index: 0,
        kind: ComponentSelectionKind::BrepFace,
    };
    let choose = |app: &mut VibocerosApp| {
        app.accept_component_window(ComponentWindow {
            picks: vec![pick],
            preselection: false,
            modifiers: Default::default(),
            crossing: false,
            inverted: false,
        })
    };
    enter(&mut app, "Inset");
    assert!(app.picking_inset());
    choose(&mut app);
    enter(&mut app, "");
    assert!(!app.picking_inset());
    enter(&mut app, ".3 CreaseCorners=Yes");
    enter(&mut app, "");
    assert_eq!(app.commands.inset_surface_options().distance, 0.3);
    assert!(app.commands.inset_surface_options().crease_corners);
    assert_eq!(app.commands.inset_options(false).distance, 0.1);
    assert_eq!(app.commands.inset_options(true).distance, 1.);
    enter(&mut app, "Undo");
    let before = app.document.object(id).unwrap().geometry_snapshot().clone();
    enter(&mut app, "Inset");
    choose(&mut app);
    enter(&mut app, "");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Distance=0.3 CreaseCorners=Yes")
    );
    enter(&mut app, ".7 CreaseCorners=No");
    enter(&mut app, "Cancel");
    assert!(app.commands.inset_surface_options().crease_corners);
    assert_eq!(app.commands.inset_surface_options().distance, 0.3);
    assert_eq!(app.document.redo_label(), Some("Inset"));
    enter(&mut app, "Inset");
    choose(&mut app);
    enter(&mut app, "");
    app.document
        .add_group(Some("Changed".into()), [id])
        .unwrap();
    enter(&mut app, "");
    assert!(app.inset_session.is_some());
    assert_eq!(
        app.document.object(id).unwrap().geometry_snapshot(),
        &before
    );
    enter(&mut app, "Cancel");
}

#[test]
fn inset_surface_invalid_default_can_be_repaired_and_still_rejects_stale_sources() {
    let native = common::cases_for("inset_surface_sdk")
        .into_iter()
        .find(|(op, _)| op["id"] == "hole_tight_ring")
        .unwrap()
        .1;
    for stale in [false, true] {
        let mut app = test_app();
        let (doc, id) = native_surface::fixture(&native["before"]);
        app.document = doc;
        enter(&mut app, "Inset");
        app.accept_component_click(ComponentClick {
            picks: vec![ComponentPick {
                object: id,
                index: 0,
                kind: ComponentSelectionKind::BrepFace,
            }],
            preselection: false,
            modifiers: Default::default(),
        });
        enter(&mut app, "");
        assert!(!app.picking_inset());
        assert!(app.inset_session.as_ref().unwrap().edges().is_empty());
        if stale {
            app.document
                .add_group(Some("Changed".into()), [id])
                .unwrap();
        }
        let before = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, ".3");
        if stale {
            assert!(app.inset_session.as_ref().unwrap().edges().is_empty());
            assert_eq!(
                app.document.object(id).unwrap().geometry_snapshot(),
                &before
            );
            enter(&mut app, "");
            assert!(app.inset_session.is_some());
            enter(&mut app, "Cancel");
        } else {
            assert!(!app.inset_session.as_ref().unwrap().edges().is_empty());
            enter(&mut app, "");
            let Geometry::Brep(result) = app.document.object(id).unwrap().geometry() else {
                panic!()
            };
            native_surface::assert_geometry(
                result,
                &native["output"]["geometry"],
                app.document.tolerance(),
                "repaired holed face",
            );
            assert_eq!(app.document.undo_label(), Some("Inset"));
        }
    }
}
