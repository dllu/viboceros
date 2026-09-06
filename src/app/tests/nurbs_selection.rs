use super::super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::LineSegment;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn fixture() -> (VibocerosApp, [ObjectId; 3]) {
    let mut app = test_app();
    let curve = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(0., 0., 0.), point(4., 2., 0.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let mesh = app
        .document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![point(10., 0., 0.), point(14., 0., 0.), point(10., 3., 0.)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    let dot = app
        .document
        .add_geometry(Geometry::Point(point(20., 0., 0.)))
        .unwrap();
    let ids = [curve, mesh, dot];
    app.document.add_group(Some("all".into()), ids).unwrap();
    (app, ids)
}

fn pick(app: &mut VibocerosApp, id: ObjectId) {
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
}

fn phase(app: &VibocerosApp) -> ObjectPromptPhase {
    app.object_prompt.as_ref().unwrap().phase
}

#[test]
fn command_first_selection_confirmation_and_mesh_submenu_are_distinct_nonmutating_phases() {
    let (mut app, ids) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ToNURBS");
    assert_eq!(phase(&app), ObjectPromptPhase::Selecting);
    enter(&mut app, "DeleteInputObjects=Yes");
    assert_eq!(app.command_input, "DeleteInputObjects=Yes");
    pick(&mut app, ids[2]);
    assert_eq!(app.document.selected_object_count(), 0);
    pick(&mut app, ids[1]);
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    assert_eq!(phase(&app), ObjectPromptPhase::Options);
    assert!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .selection_filter()
            .is_none()
    );
    pick(&mut app, ids[2]);
    enter(&mut app, "SelNone");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    enter(&mut app, "DeleteInput Yes");
    enter(&mut app, "MeshOptions");
    assert_eq!(phase(&app), ObjectPromptPhase::Menu(0));
    enter(&mut app, "TrimTriangularFaces No");
    enter(&mut app, "DeleteInputObjects=No");
    assert_eq!(app.command_input, "DeleteInputObjects=No");
    enter(&mut app, "");
    assert_eq!(phase(&app), ObjectPromptPhase::Options);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        [ids[2], ids[1], ids[0]]
    );
    let Geometry::Brep(brep) = app.document.object(ids[1]).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(brep.faces()[0].loops()[0].trims().len(), 4);
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn cancelling_preselected_options_keeps_selection_redo_and_remembered_choices() {
    let (mut app, ids) = fixture();
    app.document
        .select_objects_direct([ids[0], ids[1]], SelectionMode::Replace)
        .unwrap();
    app.commands
        .execute(
            &mut app.document,
            "ToNURBS DeleteInputObjects=Yes MeshOptions TrimTriangularFaces=Yes",
        )
        .unwrap();
    app.commands.execute(&mut app.document, "Undo").unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ToNURBS");
    assert_eq!(phase(&app), ObjectPromptPhase::Options);
    assert!(!app.object_prompt.as_ref().unwrap().postselected);
    enter(&mut app, "DeleteInputObjects=No");
    enter(&mut app, "MeshOptions");
    enter(&mut app, "TrimTriangularFaces=No");
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.document.can_redo());
    enter(&mut app, "ToNURBS");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .description
            .command_line(),
        "ToNURBS DeleteInputObjects=Yes MeshOptions TrimTriangularFaces=Yes"
    );
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
}

#[test]
fn cancelling_postselection_at_each_phase_clears_picks_without_accepting_choices() {
    for target in [
        ObjectPromptPhase::Selecting,
        ObjectPromptPhase::Options,
        ObjectPromptPhase::Menu(0),
    ] {
        let (mut app, ids) = fixture();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document
            .select_objects_direct([ids[2]], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ToNURBS");
        assert_eq!(app.document.selected_object_count(), 0);
        pick(&mut app, ids[1]);
        if target != ObjectPromptPhase::Selecting {
            enter(&mut app, "");
            enter(&mut app, "DeleteInputObjects=Yes");
        }
        if matches!(target, ObjectPromptPhase::Menu(_)) {
            enter(&mut app, "MeshOptions");
            enter(&mut app, "TrimTriangularFaces=No");
        }
        assert_eq!(phase(&app), target);
        app.cancel_interactive_command(true);
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        app.document
            .select_objects_direct([ids[1]], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ToNURBS");
        assert_eq!(
            app.object_prompt
                .as_ref()
                .unwrap()
                .description
                .command_line(),
            "ToNURBS DeleteInputObjects=No MeshOptions TrimTriangularFaces=Yes"
        );
    }
}

#[test]
fn noops_skip_confirmation_and_mesh_options_are_unavailable_for_curves() {
    let (mut app, ids) = fixture();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ToNURBS");
    let pending = app.object_prompt.clone();
    assert!(pending.as_ref().unwrap().description.menus.is_empty());
    for invalid in [
        "MeshOptions",
        "TrimTriangularFaces=No",
        "DeleteInput=Yes DeleteInputObjects=No",
    ] {
        enter(&mut app, invalid);
        assert_eq!(app.object_prompt, pending);
        assert_eq!(app.command_input, invalid);
    }
    enter(&mut app, "DeleteInputObjects=Yes");
    enter(&mut app, "");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ToNURBS DeleteInputObjects=No");
    assert!(app.object_prompt.is_none());
    assert!(app.document.is_selected(ids[0]));
    app.document.clear_selection();
    enter(&mut app, "ToNURBS DeleteInputObjects=No");
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(
        app.commands
            .object_selection_prompt("ToNURBS")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}

#[test]
fn transparent_controls_and_nested_cplane_preserve_confirmation_and_submenu_state() {
    let (mut app, ids) = fixture();
    enter(&mut app, "ToNURBS");
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    enter(&mut app, "MeshOptions");
    let pending = app.object_prompt.clone();
    for input in [
        "SetDisplayMode Viewport=All Mode=Ghosted",
        "Snap",
        "Osnap",
        "CPlane World Front",
    ] {
        enter(&mut app, input);
        assert_eq!(app.object_prompt, pending);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [ids[1]]
        );
    }
    enter(&mut app, "CPlane");
    assert!(app.plane_prompt.is_some());
    app.cancel_plane_prompt();
    assert_eq!(app.object_prompt, pending);
    enter(&mut app, "Point 30,0,0");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().len(), 4);
}
