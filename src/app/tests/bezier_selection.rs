use super::super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::LineSegment;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn fixture() -> (VibocerosApp, [ObjectId; 4]) {
    let mut app = test_app();
    let mut ids = Vec::new();
    for x in [0., 10.] {
        ids.push(
            app.document
                .add_geometry(Geometry::Line(
                    LineSegment::try_new(
                        point(x, 0., 0.),
                        point(x + 4., 2., 0.),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                ))
                .unwrap(),
        );
    }
    ids.push(
        app.document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![point(20., 0., 0.), point(24., 0., 0.), point(20., 3., 0.)],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap(),
    );
    ids.push(
        app.document
            .add_geometry(Geometry::Point(point(30., 0., 0.)))
            .unwrap(),
    );
    app.document
        .add_group(Some("all".into()), ids.clone())
        .unwrap();
    (app, ids.try_into().unwrap())
}

fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}

#[test]
fn bezier_selection_filters_groups_and_yes_no_answers_convert_without_an_extra_enter() {
    for answer in ["Yes", "_No", "DeleteInput=Yes", "DeleteInput No"] {
        let (mut app, ids) = fixture();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "ConvertToBeziers");
        enter(&mut app, "");
        assert_eq!(
            app.object_prompt.as_ref().unwrap().phase,
            ObjectPromptPhase::Selecting
        );
        enter(&mut app, "Yes");
        assert_eq!(app.command_input, "Yes");
        pick(&mut app, ids[2]);
        pick(&mut app, ids[3]);
        assert_eq!(app.document.selected_object_count(), 0);
        pick(&mut app, ids[1]);
        pick(&mut app, ids[0]);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [ids[1], ids[0]]
        );
        enter(&mut app, "");
        let pending = app.object_prompt.clone();
        assert_eq!(pending.as_ref().unwrap().phase, ObjectPromptPhase::Options);
        assert!(pending.as_ref().unwrap().selection_filter().is_none());
        for invalid in [
            "Maybe",
            "Yes No",
            "DeleteInput=Yes DeleteInput=No",
            "MeshOptions",
        ] {
            enter(&mut app, invalid);
            assert_eq!(app.object_prompt, pending);
            assert_eq!(app.command_input, invalid);
        }
        pick(&mut app, ids[3]);
        enter(&mut app, "SelNone");
        assert_eq!(app.document.selected_object_count(), 2);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, answer);
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.selected_object_count(), 0);
        let deleting = answer.contains("Yes");
        assert_eq!(app.document.objects().len(), if deleting { 4 } else { 6 });
        let outputs = app
            .document
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        assert_eq!(
            outputs[0]
                .geometry()
                .curve_ref()
                .unwrap()
                .start_point()
                .unwrap(),
            point(10., 0., 0.)
        );
        assert_eq!(
            outputs[1]
                .geometry()
                .curve_ref()
                .unwrap()
                .start_point()
                .unwrap(),
            point(0., 0., 0.)
        );
        assert!(outputs.iter().all(|o| o.group_ids().is_empty()));
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn bezier_cancellation_preserves_memory_geometry_and_redo_but_only_keeps_preselection() {
    for postselected in [false, true] {
        for selecting in [false, true] {
            if !postselected && selecting {
                continue;
            }
            let (mut app, ids) = fixture();
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "ConvertToBeziers Yes");
            assert!(app.object_prompt.is_none());
            enter(&mut app, "Undo");
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            app.document
                .select_objects_direct(
                    [ids[if postselected { 3 } else { 0 }]],
                    SelectionMode::Replace,
                )
                .unwrap();
            enter(
                &mut app,
                if postselected {
                    "ConvertToBeziers No"
                } else {
                    "ConvertToBeziers"
                },
            );
            if postselected {
                assert_eq!(app.document.selected_object_count(), 0);
                pick(&mut app, ids[0]);
                if !selecting {
                    enter(&mut app, "");
                }
            }
            app.cancel_interactive_command(true);
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(
                app.document.selected_object_count(),
                usize::from(!postselected)
            );
            assert!(app.document.can_redo());
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "ConvertToBeziers");
            assert!(app.object_prompt.as_ref().unwrap().description.options[0].value);
            enter(&mut app, "");
            assert!(app.object_prompt.is_none());
            assert!(app.document.object(ids[0]).is_none());
        }
    }
}

#[test]
fn bezier_transparent_commands_preserve_pending_question_and_selall_filters_locked_objects() {
    let (mut app, ids) = fixture();
    app.document.set_objects_locked([ids[1]], true).unwrap();
    enter(&mut app, "ConvertToBeziers");
    enter(&mut app, "SelAll");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    enter(&mut app, "SelNone");
    assert_eq!(app.document.selected_object_count(), 0);
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    let pending = app.object_prompt.clone();
    for input in [
        "SetDisplayMode Viewport=All Mode=Ghosted",
        "CPlane World Front",
        "Osnap On",
        "SmartTrack On",
    ] {
        enter(&mut app, input);
        assert_eq!(app.object_prompt, pending);
    }
    enter(&mut app, "CPlane");
    assert!(app.plane_prompt.is_some());
    app.cancel_plane_prompt();
    assert_eq!(app.object_prompt, pending);
    enter(&mut app, "Point 50,0,0");
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(ids[0]).is_some());
}
