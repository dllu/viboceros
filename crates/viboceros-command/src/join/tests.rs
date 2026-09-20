use super::*;
use viboceros_geometry::MeshFace;

fn quad(x: f64) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new_faces(
            [[x, 0., 0.], [x + 2., 0., 0.], [x + 2., 2., 0.], [x, 2., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap(),
    )
}

#[test]
fn mesh_join_preserves_seed_attributes_groups_and_exact_history_without_selecting_peers() {
    for postselected in [false, true] {
        let mut document = Document::default();
        let layer = document.add_layer("Seed", ColorRgb::BLACK).unwrap();
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name("seed")
            .with_object_color(ColorRgb::new(11, 22, 33));
        let first = document
            .add_geometry_with_attributes(quad(0.), attrs.clone())
            .unwrap();
        let second = document.add_geometry(quad(2.)).unwrap();
        let peer = document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        let group = document
            .add_group(Some("Shared".into()), [first, second, peer])
            .unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let before_members = document.group(group).unwrap().members().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        if postselected {
            registry
                .execute_postselected(
                    &mut document,
                    "Join JoinDisjointMeshes=Yes",
                    Default::default(),
                )
                .unwrap();
        } else {
            registry
                .execute(&mut document, "Join JoinDisjointMeshes=Yes")
                .unwrap();
        }
        assert!(document.object(first).is_none() && document.object(second).is_none());
        assert!(!document.is_selected(peer));
        let result = document.objects().find(|o| o.id() != peer).unwrap();
        assert_eq!(result.attributes(), &attrs);
        assert_eq!(result.group_ids(), [group]);
        assert_eq!(document.is_selected(result.id()), !postselected);
        let Geometry::Mesh(mesh) = result.geometry() else {
            panic!("mesh output")
        };
        assert_eq!(mesh.vertices().len(), 8);
        assert_eq!(
            mesh.faces(),
            [MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])]
        );
        let after = document.objects().cloned().collect::<Vec<_>>();
        let after_members = document.group(group).unwrap().members().collect::<Vec<_>>();
        for _ in 0..2 {
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(
                document.group(group).unwrap().members().collect::<Vec<_>>(),
                before_members
            );
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
            assert_eq!(
                document.group(group).unwrap().members().collect::<Vec<_>>(),
                after_members
            );
        }
    }
}

#[test]
fn disconnected_no_results_have_fresh_ids_even_when_geometry_is_unchanged() {
    let mut document = Document::default();
    let ids = [
        document.add_geometry(quad(0.)).unwrap(),
        document.add_geometry(quad(10.)).unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Join JoinDisjointMeshes=No")
        .unwrap();
    assert!(ids.into_iter().all(|id| document.object(id).is_none()));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_object_count(), 2);
}

#[test]
fn singleton_is_noop_and_preserves_redo() {
    let mut document = Document::default();
    let id = document.add_geometry(quad(0.)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Point 10,0,0").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let redo = document.redo_label().map(str::to_owned);
    registry.execute(&mut document, "Join").unwrap();
    assert_eq!(document.redo_label(), redo.as_deref());
    assert!(document.is_selected(id));
    registry
        .execute_postselected(&mut document, "Join", Default::default())
        .unwrap();
    assert!(!document.is_selected(id));
    assert_eq!(document.redo_label(), redo.as_deref());
}

#[test]
fn invalid_options_mixed_families_and_degenerate_alignment_leave_document_untouched() {
    let registry = CommandRegistry::with_builtins();
    for second in [
        Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
        quad(1e8),
    ] {
        let mut document = Document::default();
        let ids = [
            document.add_geometry(quad(0.)).unwrap(),
            document.add_geometry(second).unwrap(),
        ];
        registry.execute(&mut document, "Point 20,0,0").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let redo = document.redo_label().map(str::to_owned);
        for command in [
            "Join",
            "Join Invalid=Yes",
            "Join JoinDisjointMeshes=Maybe",
            "Join JoinDisjointMeshes=Yes JoinDisjointMeshes=No",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(document.selected_object_count(), 2);
            assert_eq!(document.redo_label(), redo.as_deref());
        }
    }
}

#[test]
fn join_prompt_has_typed_filter_and_registry_scoped_option_memory() {
    let command = JoinCommand::default();
    let prompt = command.object_selection_prompt(&[]).unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Join);
    assert!(prompt.options[0].value);
    command
        .accept_object_selection_options(&["_JoinDisjointMeshes", "No"])
        .unwrap();
    assert!(
        !command
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        command
            .accept_object_selection_options(&["JoinDisjointMeshes=Yes", "extra"])
            .is_err()
    );
    assert!(
        !command
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        JoinCommand::default()
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}

#[test]
fn preselection_uses_table_order_while_command_first_uses_pick_order() {
    for postselected in [false, true] {
        let mut document = Document::default();
        let first = document
            .add_geometry_with_attributes(
                quad(0.),
                ObjectAttributes::on_layer(document.current_layer_id()).with_name("first"),
            )
            .unwrap();
        let last = document
            .add_geometry_with_attributes(
                quad(10.),
                ObjectAttributes::on_layer(document.current_layer_id()).with_name("last"),
            )
            .unwrap();
        for id in [last, first] {
            document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        let registry = CommandRegistry::with_builtins();
        if postselected {
            registry
                .execute_postselected(&mut document, "Join", Default::default())
                .unwrap();
        } else {
            registry.execute(&mut document, "Join").unwrap();
        }
        let output = document.objects().next().unwrap();
        assert_eq!(
            output.attributes().name(),
            Some(if postselected { "last" } else { "first" })
        );
        let Geometry::Mesh(mesh) = output.geometry() else {
            panic!("mesh")
        };
        assert_eq!(mesh.vertices()[0].x(), if postselected { 10. } else { 0. });
    }
}
