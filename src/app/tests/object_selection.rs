use super::*;
use viboceros_document::SelectionMode;

#[test]
fn remove_from_group_prompt_picks_individual_members_and_supports_copy() {
    for copy in [false, true] {
        let mut app = test_app();
        let ids = [0., 1., 2.].map(|x| {
            app.document
                .add_geometry(Geometry::Point(point(x, 0., 0.)))
                .unwrap()
        });
        let group = app.document.add_group(None, [ids[0], ids[1]]).unwrap();
        enter(&mut app, "RemoveFromGroup");
        assert!(app.object_prompt.is_some());
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[2]),
            mode: SelectionMode::Replace,
        });
        assert_eq!(app.document.selected_object_count(), 0);
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[0]),
            mode: SelectionMode::Replace,
        });
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [ids[0]]
        );
        if copy {
            enter(&mut app, "Copy=Yes");
        }
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.selected_object_count(), 1);
        assert!(
            app.document
                .selected_objects()
                .next()
                .unwrap()
                .group_ids()
                .is_empty()
        );
        assert_eq!(
            app.document.group(group).unwrap().members().len(),
            if copy { 2 } else { 1 }
        );
        assert_eq!(app.document.objects().len(), if copy { 4 } else { 3 });
        enter(&mut app, "Undo");
        assert_eq!(app.document.group(group).unwrap().members().len(), 2);
        assert_eq!(app.document.objects().len(), 3);
    }
}

#[test]
fn point_cloud_command_first_filters_clouds_and_curves_and_preserves_pick_order() {
    let mut app = test_app();
    let ids = [1.0, 2.0, 3.0].map(|x| {
        app.document
            .add_geometry(Geometry::Point(point(x, 0.0, 0.0)))
            .unwrap()
    });
    let cloud_id = app
        .document
        .add_geometry(Geometry::PointCloud(
            viboceros_geometry::PointCloud3::try_new(vec![point(99.0, 0.0, 0.0)]).unwrap(),
        ))
        .unwrap();
    enter(&mut app, "PointCloud");
    assert!(app.object_prompt.is_some());
    for id in [cloud_id, ids[2], ids[0], ids[1]] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    assert!(!app.document.is_selected(cloud_id));
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(cloud_id).is_some());
    let object = app.document.objects().find(|o| o.id() != cloud_id).unwrap();
    let Geometry::PointCloud(cloud) = object.geometry() else {
        panic!()
    };
    assert_eq!(
        cloud.points(),
        [
            point(3.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0)
        ]
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 4);
}

#[test]
fn point_cloud_cancel_preserves_sources_and_rejects_unsupported_colors() {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::Point(point(1.0, 0.0, 0.0)))
        .unwrap();
    enter(&mut app, "PointCloud UsePointColors=Yes");
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(id).is_some());
    enter(&mut app, "PointCloud");
    assert!(app.object_prompt.is_some());
    app.cancel_interactive_command(false);
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(id).is_some());
}

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn fixture() -> (VibocerosApp, [viboceros_document::ObjectId; 3]) {
    let mut app = test_app();
    let mut ids = Vec::new();
    for x in [0., 10.] {
        ids.push(
            app.document
                .add_geometry(Geometry::Mesh(
                    TriangleMesh::try_new(
                        vec![point(x, 0., 0.), point(x + 4., 0., 0.), point(x, 3., 0.)],
                        vec![[0, 1, 2]],
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                ))
                .unwrap(),
        );
    }
    ids.push(
        app.document
            .add_geometry(Geometry::Point(point(0., 0., 0.)))
            .unwrap(),
    );
    app.document
        .add_group(Some("all".into()), ids.iter().copied())
        .unwrap();
    (app, ids.try_into().unwrap())
}

#[test]
fn command_first_picking_filters_groups_adds_clicks_and_clears_sources_on_commit() {
    let (mut app, ids) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "MeshToNURB");
    assert!(app.object_prompt.is_some());
    assert!(app.active_command.is_none());
    for id in [ids[2], ids[1], ids[0]] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    enter(&mut app, "TrimTriangularFaces No");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().len(), 5);
    for (o, x) in app.document.objects().skip(3).zip([10., 0.]) {
        let Geometry::Brep(b) = o.geometry() else {
            panic!()
        };
        assert_eq!(b.faces()[0].loops()[0].trims().len(), 4);
        assert_eq!(b.vertices()[0].point().x(), x);
        assert!(o.group_ids().is_empty());
    }
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(app.document.undo_label(), Some("MeshToNURB"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn cancelled_picks_and_accepted_options_do_not_edit_geometry_or_clear_redo() {
    let (mut app, ids) = fixture();
    enter(&mut app, "Point 30,0,0");
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "MeshToNURB TrimTriangularFaces=No");
    assert_eq!(app.document.selected_object_count(), 0);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    app.cancel_interactive_command(true);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.document.can_redo());
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "MeshToNURB");
    assert!(app.object_prompt.is_none());
    assert!(app.document.is_selected(ids[0]));
    let Geometry::Brep(b) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces()[0].loops()[0].trims().len(), 4);
}

#[test]
fn empty_finish_invalid_options_and_nonmesh_clicks_keep_the_prompt_usable() {
    let (mut app, ids) = fixture();
    enter(&mut app, "MeshToNURB");
    let prompt = app.object_prompt.clone();
    enter(&mut app, "");
    assert_eq!(app.object_prompt, prompt);
    for input in [
        "TrimTriangularFaces=No Unknown=Yes",
        "UseNgons=No UseNgons=Yes",
        "not-an-option",
    ] {
        enter(&mut app, input);
        assert_eq!(app.object_prompt, prompt);
        assert_eq!(app.command_input, input);
    }
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[2]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "SelAll");
    assert_eq!(app.document.selected_object_count(), 2);
    app.apply_selection_click(SelectionClick {
        object_id: None,
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 2);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Remove,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    enter(&mut app, "SelNone");
    assert_eq!(app.document.selected_object_count(), 0);
    app.apply_selection_window(SelectionWindow {
        object_ids: ids.to_vec(),
        mode: SelectionMode::Replace,
        crossing: true,
    });
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 5);
}

#[test]
fn transparent_interface_and_cplane_prompts_preserve_mesh_picks() {
    let (mut app, ids) = fixture();
    enter(&mut app, "MeshToNURB");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    let pending = app.object_prompt.clone();
    for input in [
        "SetDisplayMode Viewport=All Mode=Ghosted",
        "Snap",
        "Osnap",
        "Help UI",
        "CPlane World Front",
    ] {
        enter(&mut app, input);
        assert_eq!(app.object_prompt, pending, "{input}");
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [ids[0]]
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
