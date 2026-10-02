use super::*;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;

fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn cube(app: &mut VibocerosApp, x: f64) -> ComponentPick {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[x, x + 2.], [0., 3.], [0., 5.]],
        app.document.tolerance(),
    )
    .unwrap();
    ComponentPick {
        object: app.document.add_geometry(Geometry::Brep(brep)).unwrap(),
        kind: ComponentSelectionKind::BrepEdge,
        index: 0,
    }
}
fn objects(app: &VibocerosApp) -> Vec<viboceros_document::Object> {
    app.document.objects().cloned().collect()
}
fn click(app: &mut VibocerosApp, picks: Vec<ComponentPick>, preselection: bool) {
    assert!(app.handle_viewport_action(ViewportOutput {
        component_click: Some(ComponentClick {
            picks,
            preselection
        }),
        ..Default::default()
    }));
}
fn window(app: &mut VibocerosApp, picks: Vec<ComponentPick>, preselection: bool) {
    assert!(app.handle_viewport_action(ViewportOutput {
        component_window: Some(ComponentWindow {
            picks,
            preselection,
            crossing: true,
            inverted: false
        }),
        ..Default::default()
    }));
}
#[test]
fn preselection_finishes_immediately_preserves_metadata_and_is_one_external_undo() {
    let mut app = test_app();
    let first = cube(&mut app, 0.);
    let second = cube(&mut app, 10.);
    let group = app.document.add_group(None, [first.object]).unwrap();
    app.document.clear_history().unwrap();
    let before = objects(&app);
    window(
        &mut app,
        vec![
            second,
            first,
            ComponentPick { index: 1, ..first },
            ComponentPick { index: 2, ..first },
            ComponentPick { index: 3, ..first },
        ],
        true,
    );
    submit(&mut app, "_UnjoinEdge");
    assert!(app.unjoin_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    let after = objects(&app);
    let child = after
        .iter()
        .find(|o| o.id() != first.object && o.id() != second.object)
        .unwrap();
    assert_eq!(child.group_ids(), [group]);
    assert_eq!(
        child.attributes(),
        app.document.object(first.object).unwrap().attributes()
    );
    submit(&mut app, "_Cancel");
    assert_eq!(objects(&app), after);
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), before);
    assert!(!app.document.can_undo());
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    submit(&mut app, "Redo");
    assert_eq!(objects(&app), after);
}
#[test]
fn postselection_collects_clicks_and_rectangles_until_enter_and_escape_discards_the_batch() {
    for cancel in [false, true] {
        let mut app = test_app();
        let first = cube(&mut app, 0.);
        let second = cube(&mut app, 10.);
        app.document.clear_history().unwrap();
        let before = objects(&app);
        submit(&mut app, "UnjoinEdge");
        assert!(!app.command_line_idle());
        assert!(app.viewport_object_filter().is_none());
        click(&mut app, vec![second], false);
        window(
            &mut app,
            (0..4)
                .map(|index| ComponentPick { index, ..first })
                .collect(),
            false,
        );
        click(&mut app, vec![second], false); // Repeated postselection keeps the edge.
        assert_eq!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()[0],
            second
        );
        assert_eq!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .len(),
            5
        );
        assert_eq!(objects(&app), before);
        assert!(!app.document.can_undo());
        if cancel {
            app.cancel_current_prompt_or_selection();
        } else {
            submit(&mut app, "");
        }
        assert!(app.unjoin_prompt.is_none());
        assert!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap()
                .is_empty()
        );
        if cancel {
            assert_eq!(objects(&app), before);
            assert!(!app.document.can_undo());
        } else {
            let after = objects(&app);
            assert_eq!(after.len(), 3);
            submit(&mut app, "Undo");
            assert_eq!(objects(&app), before);
            assert!(!app.document.can_undo());
            submit(&mut app, "Redo");
            assert_eq!(objects(&app), after);
        }
    }
}
#[test]
fn stale_sources_and_tolerance_changes_reject_the_entire_batch_without_dropping_other_picks() {
    for change in ["geometry", "hidden", "tolerance"] {
        let mut app = test_app();
        let first = cube(&mut app, 0.);
        let second = cube(&mut app, 10.);
        submit(&mut app, "UnjoinEdge");
        window(&mut app, vec![first, second], false);
        match change {
            "geometry" => {
                app.document
                    .replace_object_geometries([(
                        first.object,
                        Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()),
                    )])
                    .unwrap();
            }
            "hidden" => {
                app.document
                    .set_objects_visibility([first.object], false)
                    .unwrap();
            }
            _ => {
                app.document.set_tolerance(
                    viboceros_geometry::Tolerance::try_new(0.01, 1e-8, 1e-4).unwrap(),
                );
            }
        }
        app.document.clear_history().unwrap();
        let before = objects(&app);
        let _ = app.component_selection.highlights(&app.document);
        submit(&mut app, "");
        assert_eq!(objects(&app), before);
        assert!(!app.document.can_undo());
        assert!(app.command_log.back().unwrap().contains("changed"));
        assert!(app.unjoin_prompt.is_none());
    }
}
#[test]
fn ambiguity_is_readonly_filters_seams_and_faces_and_checks_candidate_identity() {
    let mut app = test_app();
    let first = cube(&mut app, 0.);
    let second = cube(&mut app, 10.);
    let tube = app
        .document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_tube(
                viboceros_command::CommandContext::default().construction_plane,
                [2., 5.],
                8.,
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    submit(&mut app, "UnjoinEdge");
    let before = objects(&app);
    window(
        &mut app,
        vec![
            ComponentPick {
                object: tube,
                index: 1,
                ..first
            },
            ComponentPick {
                kind: ComponentSelectionKind::BrepFace,
                ..first
            },
        ],
        false,
    );
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    click(&mut app, vec![first, second], false);
    submit(&mut app, "99");
    assert_eq!(objects(&app), before);
    submit(&mut app, "2");
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [second]
    );
    click(&mut app, vec![first, second], false);
    app.document
        .set_objects_locked([first.object], true)
        .unwrap();
    submit(&mut app, "1");
    assert!(!app.component_selection.has_choices());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [second]
    );
    submit(&mut app, "Cancel");
    assert!(app.unjoin_prompt.is_none());
}
#[test]
fn empty_wrong_kind_and_cancelled_selection_preserve_redo() {
    let mut app = test_app();
    let first = cube(&mut app, 0.);
    submit(&mut app, "Point 1,2,3");
    submit(&mut app, "Undo");
    let before = objects(&app);
    window(
        &mut app,
        vec![ComponentPick {
            kind: ComponentSelectionKind::BrepFace,
            ..first
        }],
        true,
    );
    submit(&mut app, "UnjoinEdge");
    assert!(app.unjoin_prompt.is_some());
    submit(&mut app, "");
    assert_eq!(objects(&app), before);
    assert!(app.document.can_redo());
    submit(&mut app, "UnjoinEdge");
    click(&mut app, vec![first], false);
    submit(&mut app, "Cancel");
    assert_eq!(objects(&app), before);
    assert!(app.document.can_redo());
}
#[test]
fn view_and_cplane_commands_preserve_selection_and_new_geometry_commands_cancel_it() {
    let mut app = test_app();
    let first = cube(&mut app, 0.);
    let before = objects(&app);
    submit(&mut app, "UnjoinEdge");
    click(&mut app, vec![first], false);
    submit(&mut app, "SetView World Top");
    assert!(app.unjoin_prompt.is_some());
    submit(&mut app, "CPlane");
    submit(&mut app, "w0,0,7");
    assert!(app.unjoin_prompt.is_some());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [first]
    );
    assert_eq!(objects(&app), before);
    submit(&mut app, "Point 10,20,30");
    assert!(app.unjoin_prompt.is_none());
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), before);
}
