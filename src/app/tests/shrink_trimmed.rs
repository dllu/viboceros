use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, NurbsSurface};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn patch(app: &mut VibocerosApp, compound: bool) -> viboceros_document::ObjectId {
    let surface = |offset| {
        NurbsSurface::try_bilinear([
            point(offset, 0., 0.),
            point(offset + 10., 0., 0.),
            point(offset, 10., 0.),
            point(offset + 10., 10., 0.),
        ])
        .unwrap()
    };
    let a = Brep::try_rectangular_surface_face(
        surface(0.),
        0.2..=0.8,
        0.1..=0.9,
        app.document.tolerance(),
    )
    .unwrap();
    let brep = if compound {
        let b = Brep::try_rectangular_surface_face(
            surface(20.),
            0.2..=0.8,
            0.1..=0.9,
            app.document.tolerance(),
        )
        .unwrap();
        Brep::try_combine(vec![a, b], app.document.tolerance()).unwrap()
    } else {
        a
    };
    app.document.add_geometry(Geometry::Brep(brep)).unwrap()
}

#[test]
fn shrink_commands_route_filtered_whole_brep_picks_cancel_and_history() {
    for command in ["ShrinkTrimmedSrf", "ShrinkTrimmedSrfToEdge"] {
        for pre in [false, true] {
            for compound in [false, true] {
                let mut app = test_app();
                let id = patch(&mut app, compound);
                let peer = app
                    .document
                    .add_geometry(Geometry::Point(point(30., 30., 0.)))
                    .unwrap();
                let before = app.document.objects().cloned().collect::<Vec<_>>();
                app.document.clear_history().unwrap();
                if pre {
                    app.document
                        .select_objects_direct([id], SelectionMode::Replace)
                        .unwrap();
                }
                enter(&mut app, command);
                if !pre {
                    assert!(app.object_prompt.is_some());
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(peer),
                        mode: SelectionMode::Replace,
                    });
                    assert!(!app.document.is_selected(peer));
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                    app.cancel_current_prompt_or_selection();
                    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
                    assert!(!app.document.can_undo());
                    enter(&mut app, command);
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                    enter(&mut app, "");
                }
                assert!(app.object_prompt.is_none());
                assert_eq!(app.document.is_selected(id), pre);
                let Geometry::Brep(result) = app.document.object(id).unwrap().geometry() else {
                    panic!()
                };
                assert!(
                    result
                        .faces()
                        .iter()
                        .all(|face| face.surface().domain_u() == (0.2..=0.8))
                );
                enter(&mut app, "Undo");
                assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
                assert!(!app.document.can_undo());
                enter(&mut app, "Redo");
                assert!(app.document.object(peer).is_some());
            }
        }
    }
}

fn face_click(app: &mut VibocerosApp, object: viboceros_document::ObjectId, index: usize) {
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index,
        }],
        preselection: true,
        modifiers: egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    });
}

#[test]
fn shrink_faces_route_preselection_modifier_picks_toggle_cancel_and_history() {
    for pre in [false, true] {
        let mut app = test_app();
        let id = patch(&mut app, true);
        let before = app.document.object(id).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        if pre {
            face_click(&mut app, id, 1);
        }
        enter(&mut app, "ShrinkTrimmedSrf");
        if !pre {
            assert!(app.component_preselection_available());
            face_click(&mut app, id, 0);
            face_click(&mut app, id, 0);
            face_click(&mut app, id, 1);
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none());
        assert!(
            app.component_selection
                .valid_picks(&app.document)
                .is_empty()
        );
        let Geometry::Brep(result) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        let Geometry::Brep(source) = &before else {
            panic!()
        };
        assert_eq!(result.faces()[0], source.faces()[0]);
        assert_eq!(result.faces()[1].surface().domain_u(), 0.2..=0.8);
        assert!(!app.document.is_selected(id));
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert!(
            app.component_selection
                .valid_picks(&app.document)
                .is_empty()
        );
    }
    for command in ["ShrinkTrimmedSrf", "ShrinkTrimmedSrfToEdge"] {
        let mut app = test_app();
        let id = patch(&mut app, true);
        let before = app.document.object(id).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        if command.ends_with("ToEdge") {
            face_click(&mut app, id, 0);
        }
        enter(&mut app, command);
        assert!(app.object_prompt.is_some());
        if command.ends_with("ToEdge") {
            assert!(!app.component_preselection_available());
            assert!(
                app.component_selection
                    .valid_picks(&app.document)
                    .is_empty()
            );
        } else {
            face_click(&mut app, id, 0);
        }
        app.cancel_current_prompt_or_selection();
        assert!(
            app.component_selection
                .valid_picks(&app.document)
                .is_empty()
        );
        assert_eq!(app.document.object(id).unwrap().geometry(), &before);
        assert!(!app.document.can_undo());
    }
}

#[test]
fn shrink_face_prompt_rejects_replaced_sources_and_selnone_clears_faces() {
    let mut app = test_app();
    let id = patch(&mut app, true);
    enter(&mut app, "ShrinkTrimmedSrf");
    face_click(&mut app, id, 1);
    enter(&mut app, "SelNone");
    assert!(
        app.component_selection
            .valid_picks(&app.document)
            .is_empty()
    );
    face_click(&mut app, id, 0);
    let Geometry::Brep(source) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    app.document
        .replace_object_geometries([(id, Geometry::Brep(source.reversed()))])
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert!(!app.document.can_undo());
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("source changed"))
    );
}

#[test]
fn shrink_face_window_and_whole_object_picks_share_one_undo_step() {
    let mut app = test_app();
    let id = patch(&mut app, true);
    let whole = patch(&mut app, false);
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "ShrinkTrimmedSrf");
    app.apply_selection_click(SelectionClick {
        object_id: Some(whole),
        mode: SelectionMode::Add,
    });
    app.accept_component_window(crate::viewport::ComponentWindow {
        picks: vec![crate::viewport::ComponentPick {
            object: id,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 1,
        }],
        preselection: true,
        modifiers: egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
        crossing: true,
        inverted: false,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let Geometry::Brep(result) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(result.faces()[0].surface().domain_u(), 0.0..=1.0);
    assert_eq!(result.faces()[1].surface().domain_u(), 0.2..=0.8);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
}
