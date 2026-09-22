use super::*;
use viboceros_document::SelectionMode;

#[test]
fn volume_warning_answers_and_escape_are_separate_from_cancelling_selection() {
    use crate::app::object_selection::ObjectPromptPhase;
    use viboceros_geometry::{Tolerance, TriangleMesh};
    for post in [false, true] {
        for answer in ["Yes", "No", "", "Escape", "replace"] {
            let mut app = test_app();
            let m = TriangleMesh::try_new(
                vec![
                    point(0., 0., 0.),
                    point(3., 0., 0.),
                    point(0., 4., 0.),
                    point(0., 0., 5.),
                ],
                vec![[0, 1, 3], [0, 3, 2], [1, 2, 3]],
                Tolerance::DEFAULT,
            )
            .unwrap();
            let id = app.document.add_geometry(Geometry::Mesh(m)).unwrap();
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            let undo_before = app.document.undo_label().map(str::to_owned);
            let redo_before = app.document.redo_label().map(str::to_owned);
            if !post {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            enter(&mut app, "VolumeCentroid");
            if post {
                assert_eq!(
                    app.object_prompt.as_ref().unwrap().phase,
                    ObjectPromptPhase::Selecting
                );
                assert!(!app.answer_object_prompt_escape());
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
                enter(&mut app, "");
            }
            assert_eq!(
                app.object_prompt.as_ref().unwrap().phase,
                ObjectPromptPhase::Options
            );
            assert!(
                app.object_prompt
                    .as_ref()
                    .unwrap()
                    .hint()
                    .contains("jointly enclose")
            );
            assert!(app.viewport_object_filter().is_none());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            match answer {
                "Escape" => assert!(app.answer_object_prompt_escape()),
                "replace" => app.cancel_interactive_command(false),
                _ => enter(&mut app, answer),
            }
            assert!(app.object_prompt.is_none(), "{answer}");
            assert_eq!(app.document.selected_object_count(), usize::from(!post));
            if matches!(answer, "No" | "replace") {
                assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(app.document.undo_label(), undo_before.as_deref());
                assert_eq!(app.document.redo_label(), redo_before.as_deref());
            } else {
                assert!(
                    matches!(app.document.objects().last().unwrap().geometry(),Geometry::Point(p) if p.to_array()==[0.375,0.5,1.875])
                );
                assert_eq!(app.document.undo_label(), Some("VolumeCentroid"));
                enter(&mut app, "Undo");
                assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
                enter(&mut app, "Redo");
                assert_eq!(app.document.objects().len(), before.len() + 1);
            }
        }
    }
}

#[test]
fn volume_centroid_object_prompt_finishes_or_cancels_with_filtered_picks() {
    for cancel in [false, true] {
        let mut app = test_app();
        enter(&mut app, "Box 0,0,0 4,6,0 8");
        let solid = app.document.objects().last().unwrap().id();
        enter(&mut app, "Line 10,0 11,0");
        let line = app.document.objects().last().unwrap().id();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document.clear_selection();
        enter(&mut app, "VolumeCentroid");
        assert!(app.object_prompt.is_some());
        for id in [line, solid] {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        assert!(!app.document.is_selected(line));
        assert!(app.document.is_selected(solid));
        if cancel {
            app.cancel_interactive_command(false);
        } else {
            enter(&mut app, "");
            assert!(
                matches!(app.document.objects().last().unwrap().geometry(),Geometry::Point(p) if p.distance_to(point(2.,3.,4.)).unwrap()<1e-12)
            );
            assert_eq!(app.document.selected_object_count(), 0);
            enter(&mut app, "Undo");
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn area_centroid_uses_filtered_object_prompt_and_creates_one_undoable_point() {
    for cancel in [false, true] {
        let mut app = test_app();
        enter(&mut app, "Circle 0,0 2");
        let curve = app.document.objects().last().unwrap().id();
        enter(&mut app, "Line 5,0 6,0");
        let line = app.document.objects().last().unwrap().id();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document.clear_selection();
        enter(&mut app, "AreaCentroid");
        assert!(app.object_prompt.is_some());
        for id in [line, curve] {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        assert!(!app.document.is_selected(line));
        assert!(app.document.is_selected(curve));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        if cancel {
            app.cancel_interactive_command(false);
        } else {
            enter(&mut app, "");
            assert_eq!(app.document.objects().len(), before.len() + 1);
            assert!(
                matches!(app.document.objects().last().unwrap().geometry(),Geometry::Point(p) if *p==point(0.,0.,0.))
            );
            assert_eq!(app.document.selected_object_count(), 0);
            assert_eq!(app.document.undo_label(), Some("AreaCentroid"));
            enter(&mut app, "Undo");
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn merge_all_edges_filters_picks_waits_for_enter_and_cancels_without_edits() {
    use viboceros_geometry::{Brep, Tolerance};
    for cancel in [false, true] {
        let mut app = test_app();
        let cube = Brep::try_box(
            viboceros_command::CommandContext::default().construction_plane,
            [[0., 2.]; 3],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cut = cube.edges()[0].curve().parameter_at(0.5).unwrap();
        let split = cube
            .try_split_edges_at_parameters(&[(0, vec![cut])], Tolerance::DEFAULT)
            .unwrap();
        let id = app.document.add_geometry(Geometry::Brep(split)).unwrap();
        let peer = app
            .document
            .add_geometry(Geometry::Point(point(9., 0., 0.)))
            .unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document.clear_selection();
        enter(&mut app, "MergeAllEdges");
        for picked in [peer, id] {
            app.apply_selection_click(SelectionClick {
                object_id: Some(picked),
                mode: SelectionMode::Replace,
            });
            assert!(app.object_prompt.is_some());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        }
        assert!(!app.document.is_selected(peer));
        assert!(app.document.is_selected(id));
        if cancel {
            app.cancel_interactive_command(false);
        } else {
            enter(&mut app, "");
            assert!(
                matches!(app.document.object(id).unwrap().geometry(), Geometry::Brep(b) if b.edges().len() == 12)
            );
            assert_eq!(app.document.selected_object_count(), 0);
            assert_eq!(app.document.undo_label(), Some("MergeAllEdges"));
            enter(&mut app, "Undo");
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn surface_join_picks_wait_for_enter_and_undo_restores_the_sources() {
    use viboceros_geometry::{Brep, Frame3, Tolerance, Vector3};
    for command in ["Join", "JoinCopy"] {
        let mut app = test_app();
        let frame = Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cube =
            Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
        let ids = [0, 2].map(|i| {
            app.document
                .add_geometry(Geometry::Brep(
                    cube.sub_brep(&[i], Tolerance::DEFAULT).unwrap(),
                ))
                .unwrap()
        });
        let peer = app
            .document
            .add_geometry(Geometry::Point(point(9., 0., 0.)))
            .unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document.clear_selection();
        enter(&mut app, command);
        for id in [peer, ids[0], ids[1]] {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            assert!(app.object_prompt.is_some());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        }
        assert!(!app.document.is_selected(peer));
        assert_eq!(app.document.selected_object_count(), 2);
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        let output = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()) && o.id() != peer)
            .unwrap();
        assert!(matches!(output.geometry(), Geometry::Brep(b) if b.faces().len() == 2));
        assert!(!app.document.is_selected(output.id()));
        assert_eq!(
            app.document.selected_object_count(),
            if command == "JoinCopy" { 2 } else { 0 }
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn individual_join_picks_finish_on_closure_without_consuming_the_next_click() {
    for command in ["Join", "JoinCopy"] {
        let mut app = test_app();
        for input in [
            "Line 0,0,0 1,0,0",
            "Line 1,0,0 1,2,0",
            "Line 1,2,0 0,0,0",
            "Point 8,0,0",
        ] {
            enter(&mut app, input);
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.document.clear_selection();
        enter(&mut app, command);
        for (index, &id) in ids[..3].iter().enumerate() {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            if index < 2 {
                assert!(app.object_prompt.is_some());
                assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            }
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.undo_label(), Some(command));
        assert_eq!(
            app.document.selected_object_count(),
            if command == "JoinCopy" { 3 } else { 0 }
        );
        let joined = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap();
        assert!(joined.geometry().curve_ref().unwrap().is_closed().unwrap());
        assert!(!app.document.is_selected(joined.id()));
        // The next input belongs to ordinary selection, not the completed Join.
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[3]),
            mode: SelectionMode::Replace,
        });
        assert_eq!(app.document.selected_object_count(), 1);
        assert!(app.document.is_selected(ids[3]));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn mesh_join_and_copy_command_first_filter_points_and_retain_pick_order() {
    for command in ["Join", "JoinCopy"] {
        let mut app = test_app();
        for input in [
            "MeshPlane 0,0,0 2,2,0 XCount=1 YCount=1",
            "MeshPlane 10,0,0 12,2,0 XCount=1 YCount=1",
            "Point 20,0,0",
        ] {
            enter(&mut app, input);
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        assert_eq!(ids.len(), 3);
        app.document.clear_selection();
        enter(&mut app, command);
        assert!(app.object_prompt.is_some());
        assert_eq!(
            app.viewport_object_filter(),
            Some(viboceros_command::ObjectSelectionFilter::Join)
        );
        for id in [ids[2], ids[1], ids[0]] {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        assert_eq!(app.document.selected_object_count(), 2);
        assert!(!app.document.is_selected(ids[2]));
        enter(&mut app, "JoinDisjointMeshes=Yes");
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        assert_eq!(
            app.document.objects().len(),
            if command == "Join" { 2 } else { 4 }
        );
        assert_eq!(
            app.document.selected_object_count(),
            if command == "Join" { 0 } else { 2 }
        );
        let output = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap();
        let Geometry::Mesh(mesh) = output.geometry() else {
            panic!("mesh output")
        };
        assert_eq!(mesh.vertices()[0].x(), 10.);
        assert_eq!(mesh.faces().len(), 2);
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
            ids
        );
    }
}

#[test]
fn prompt_batch_selection_filters_and_coalesces_before_applying_modes() {
    let mut app = test_app();
    let ids = [0., 1., 2., 3., 4.].map(|x| {
        app.document
            .add_geometry(Geometry::Point(point(x, 0., 0.)))
            .unwrap()
    });
    app.document
        .add_group(None, ids[..4].iter().copied())
        .unwrap();
    app.document.set_objects_locked([ids[2]], true).unwrap();
    app.document
        .set_objects_visibility([ids[3]], false)
        .unwrap();
    let missing = app
        .document
        .add_geometry(Geometry::Point(point(99., 0., 0.)))
        .unwrap();
    app.document.delete_objects([missing]).unwrap();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let groups = app.document.groups().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "RemoveFromGroup");
    app.select_prompt_objects(
        [ids[1], ids[0], ids[1], ids[2], ids[3], ids[4], missing],
        SelectionMode::Replace,
    );
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..2]
    );
    app.select_prompt_objects([ids[0], ids[0]], SelectionMode::Remove);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1]]
    );
    app.select_prompt_objects([ids[1], ids[0], ids[0]], SelectionMode::Toggle);
    // Batch toggle adds the entire batch unless every member is selected.
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    app.select_prompt_objects([ids[1], ids[0], ids[0]], SelectionMode::Toggle);
    assert_eq!(app.document.selected_object_count(), 0);
    app.select_prompt_objects([ids[0]], SelectionMode::Replace);
    app.select_prompt_objects([ids[1]], SelectionMode::Replace);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..2]
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.groups().cloned().collect::<Vec<_>>(), groups);
    assert_eq!(app.document.undo_label(), history.as_deref());
}

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
