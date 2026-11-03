use super::super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::SubD;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    let subd = SubD::try_from_polygons(
        vec![
            point(0., 0., 0.),
            point(2., 0., 1.),
            point(4., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 2.),
            point(4., 3., 0.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
    .try_with_vertex_tags(&[(0, 3), (2, 3), (3, 3), (5, 3)])
    .unwrap();
    let id = app.document.add_geometry(Geometry::SubD(subd)).unwrap();
    (app, id)
}
fn phase(app: &VibocerosApp) -> ObjectPromptPhase {
    app.object_prompt.as_ref().unwrap().phase
}
fn select(app: &mut VibocerosApp, id: ObjectId) {
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
}

#[test]
fn subd_menu_choices_return_to_their_menu_and_conversion_is_one_undoable_edit() {
    let (mut app, id) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ToNURBS");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(phase(&app), ObjectPromptPhase::Options);
    enter(&mut app, "DeleteInputObjects=Yes");
    enter(&mut app, "SubDOptions");
    assert_eq!(phase(&app), ObjectPromptPhase::Menu(0));
    enter(&mut app, "Faces");
    assert_eq!(
        phase(&app),
        ObjectPromptPhase::MenuChoice { menu: 0, choice: 0 }
    );
    enter(&mut app, "Unpacked");
    assert_eq!(phase(&app), ObjectPromptPhase::Menu(0));
    enter(&mut app, "ExtraordinaryVertex");
    assert_eq!(
        phase(&app),
        ObjectPromptPhase::MenuChoice { menu: 0, choice: 1 }
    );
    enter(&mut app, "G2");
    assert_eq!(phase(&app), ObjectPromptPhase::Menu(0));
    let prompt = app.object_prompt.as_ref().unwrap().description.clone();
    enter(&mut app, "Faces=Packed ExtraordinaryVertex=invalid");
    assert_eq!(app.object_prompt.as_ref().unwrap().description, prompt);
    enter(&mut app, "DeleteInputObjects=No");
    assert_eq!(app.object_prompt.as_ref().unwrap().description, prompt);
    enter(&mut app, "");
    assert_eq!(phase(&app), ObjectPromptPhase::Options);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let Geometry::Brep(b) = app.document.object(id).unwrap().geometry() else {
        panic!("converted SubD")
    };
    assert_eq!(b.faces().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn escaping_subd_choice_keeps_preselection_or_clears_prompted_picks_without_accepting_choices() {
    for preselected in [false, true] {
        let (mut app, id) = fixture();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if preselected {
            select(&mut app, id);
        }
        let history = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "ToNURBS");
        if !preselected {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        enter(&mut app, "DeleteInputObjects=Yes");
        enter(&mut app, "SubDOptions");
        enter(&mut app, "Faces=Unpacked ExtraordinaryVertex=G2");
        enter(&mut app, "Faces");
        assert_eq!(
            phase(&app),
            ObjectPromptPhase::MenuChoice { menu: 0, choice: 0 }
        );
        app.cancel_interactive_command(true);
        assert_eq!(app.document.is_selected(id), preselected);
        assert_eq!(app.document.undo_label(), history.as_deref());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        select(&mut app, id);
        enter(&mut app, "ToNURBS");
        let prompt = &app.object_prompt.as_ref().unwrap().description;
        assert_eq!(
            prompt.command_line(),
            "ToNURBS DeleteInputObjects=No SubDOptions Faces=Packed ExtraordinaryVertex=G1x"
        );
        enter(&mut app, "SubDOptions");
        enter(&mut app, "Faces");
        enter(&mut app, "");
        assert_eq!(phase(&app), ObjectPromptPhase::Menu(0));
        enter(&mut app, "");
        enter(&mut app, "");
        assert_eq!(app.document.objects().len(), 2);
        let Geometry::Brep(b) = app.document.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(b.faces().len(), 1);
    }
}

#[test]
fn mesh_and_subd_submenus_keep_choices_scoped_and_independent() {
    let (mut app, id) = fixture();
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
    app.document
        .select_objects_direct([id, mesh], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ToNURBS");
    enter(&mut app, "MeshOptions");
    enter(&mut app, "TrimTriangularFaces=No");
    enter(&mut app, "");
    enter(&mut app, "SubDOptions");
    assert_eq!(phase(&app), ObjectPromptPhase::Menu(1));
    enter(&mut app, "TrimTriangularFaces=Yes");
    assert_eq!(app.command_input, "TrimTriangularFaces=Yes");
    enter(&mut app, "Faces=Unpacked ExtraordinaryVertex=G1xx");
    enter(&mut app, "");
    enter(&mut app, "");
    let outputs = app.document.objects().skip(2).collect::<Vec<_>>();
    let Geometry::Brep(subd) = outputs[0].geometry() else {
        panic!()
    };
    assert_eq!(subd.faces().len(), 2);
    let Geometry::Brep(mesh) = outputs[1].geometry() else {
        panic!()
    };
    assert_eq!(mesh.faces()[0].loops()[0].trims().len(), 4);
}
