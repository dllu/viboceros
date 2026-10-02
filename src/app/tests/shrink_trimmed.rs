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
