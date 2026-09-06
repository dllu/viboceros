use super::*;

fn triangle() -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new(
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(4., 0., 0.).unwrap(),
                Point3::try_new(0., 3., 0.).unwrap(),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}

fn selected_triangle() -> (Document, ObjectId) {
    let mut d = Document::default();
    let id = d.add_geometry(triangle()).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (d, id)
}

#[test]
fn options_survive_undo_and_partial_updates_but_errors_do_not_accept_them() {
    let command = MeshToNurbCommand::default();
    let (mut d, _) = selected_triangle();
    command
        .run(&mut d, &["TrimTriangularFaces=No", "UseNgons=No"])
        .unwrap();
    d.undo().unwrap();
    assert_eq!(
        command.options.get(),
        Options {
            trim_triangular_faces: false,
            use_ngons: false
        }
    );
    command.run(&mut d, &["UseNgons=Yes"]).unwrap();
    assert_eq!(
        command.options.get(),
        Options {
            trim_triangular_faces: false,
            use_ngons: true
        }
    );
    for arguments in [
        vec!["TrimTriangularFaces=Yes", "Other=No"],
        vec!["UseNgons=No", "UseNgons=Yes"],
        vec!["DeleteInput=Yes"],
    ] {
        assert!(command.run(&mut d, &arguments).is_err());
        assert_eq!(
            command.options.get(),
            Options {
                trim_triangular_faces: false,
                use_ngons: true
            }
        );
    }
    d.select_objects_direct([], SelectionMode::Replace).unwrap();
    assert!(command.run(&mut d, &["TrimTriangularFaces=Yes"]).is_err());
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    d.select_objects_direct([point], SelectionMode::Replace)
        .unwrap();
    assert!(command.run(&mut d, &["TrimTriangularFaces=Yes"]).is_err());
    assert!(!command.options.get().trim_triangular_faces);
    let (mut next, _) = selected_triangle();
    command.run(&mut next, &[]).unwrap();
    let Geometry::Brep(output) = next.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(output.faces()[0].loops()[0].trims().len(), 4);
    assert!(
        MeshToNurbCommand::default()
            .options
            .get()
            .trim_triangular_faces
    );
}

#[test]
fn outputs_follow_source_document_order_independently_of_selection_actions() {
    let (mut d, first) = selected_triangle();
    let second = d.add_geometry(triangle()).unwrap();
    d.set_object_names([
        (first, Some("first".into())),
        (second, Some("second".into())),
    ])
    .unwrap();
    d.select_objects_direct([second], SelectionMode::Replace)
        .unwrap();
    d.select_objects_direct([first], SelectionMode::Add)
        .unwrap();
    let r = CommandRegistry::with_builtins();
    let before = d.objects().cloned().collect::<Vec<_>>();
    r.execute(&mut d, "MeshToNURB").unwrap();
    assert_eq!(
        d.objects()
            .skip(2)
            .map(|o| o.attributes().name())
            .collect::<Vec<_>>(),
        [Some("first"), Some("second")]
    );
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), [second, first]);
    let after = d.objects().cloned().collect::<Vec<_>>();
    for _ in 0..3 {
        r.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        r.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn aggregate_control_budget_fails_before_copying_any_output_or_accepting_choices() {
    let command = MeshToNurbCommand::default();
    let (mut d, first) = selected_triangle();
    let Geometry::Mesh(mesh) = triangle() else {
        panic!()
    };
    // This mesh by itself fits; the earlier triangle takes the batch over budget.
    let huge = TriangleMesh::try_new(
        mesh.vertices().to_vec(),
        vec![[0, 1, 2]; MAX_OUTPUT_CONTROLS / 4],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let second = d.add_geometry(Geometry::Mesh(huge)).unwrap();
    d.select_objects_direct([second], SelectionMode::Add)
        .unwrap();
    let group = d
        .add_group(Some("sources".into()), [first, second])
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(matches!(
        command.run(&mut d, &["TrimTriangularFaces=No"]),
        Err(CommandError::TooManyMeshNurbsControls { .. })
    ));
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(d.undo_label(), history.as_deref());
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), [first, second]);
    assert_eq!(d.group(group).unwrap().members().count(), 2);
    assert_eq!(command.options.get(), Options::default());
}

#[test]
fn mesh_to_nurb_splits_disjoint_pieces_and_preserves_derived_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(14.0, 0.0, 0.0).unwrap(),
            Point3::try_new(14.0, 3.0, 1.0).unwrap(),
            Point3::try_new(10.0, 3.0, 0.0).unwrap(),
        ],
        vec![
            viboceros_geometry::MeshFace::Triangle([0, 1, 2]),
            viboceros_geometry::MeshFace::Quad([3, 4, 5, 6]),
        ],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Faceted shell")
        .with_object_color(ColorRgb::new(23, 61, 149));
    let source_id = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Mesh source".to_owned()), [source_id])
        .unwrap();
    document
        .select_objects_direct([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "MeshToNURB").unwrap(),
        "Created 2 NURBS B-rep(s) from 2 mesh face(s); source mesh(es) retained"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(document.is_selected(source_id));
    let outputs = document
        .objects()
        .filter(|object| object.id() != source_id)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|object| !document.is_selected(object.id()))
    );
    assert!(
        outputs
            .iter()
            .all(|object| object.attributes() == &attributes)
    );
    let Geometry::Brep(triangle) = outputs[0].geometry() else {
        panic!("MeshToNURB must create B-reps")
    };
    assert_eq!(triangle.faces().len(), 1);
    assert_eq!(triangle.faces()[0].loops()[0].trims().len(), 3);
    let Geometry::Brep(quad) = outputs[1].geometry() else {
        panic!("MeshToNURB must create B-reps")
    };
    assert_eq!(quad.faces().len(), 1);
    assert_eq!(quad.faces()[0].loops()[0].trims().len(), 4);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source_id]
    );
    assert_eq!(document.undo_label(), Some("MeshToNURB"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
}

#[test]
fn mesh_to_nurb_options_and_mixed_selection_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert_eq!(
        parse(&[], Options::default()).unwrap(),
        Options {
            trim_triangular_faces: true,
            use_ngons: true,
        }
    );
    assert!(matches!(
        registry.execute(&mut document, "MeshToNURB"),
        Err(CommandError::NoObjectsSelected)
    ));
    for command in [
        "MeshToNURB TrimTriangularFaces=Maybe",
        "MeshToNURB TrimTriangularFaces=Yes TrimTriangularFaces=No",
        "MeshToNURB UseNgons=Maybe",
        "MeshToNURB Unknown=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0);
    }

    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let mesh_id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([mesh_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "MeshToNURB TrimTriangularFaces No UseNgons No",
        )
        .unwrap();
    let Geometry::Brep(untrimmed) = document.objects().last().unwrap().geometry() else {
        panic!("MeshToNURB must create a B-rep")
    };
    let trims = untrimmed.faces()[0].loops()[0].trims();
    assert_eq!(trims.len(), 4);
    assert_eq!(
        trims[3].trim_type(),
        viboceros_geometry::BrepTrimType::Singular
    );
    registry.execute(&mut document, "Undo").unwrap();

    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(10.0, 10.0, 10.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([mesh_id, point_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "MeshToNURB").unwrap();
    assert_eq!(document.objects().len(), before.len() + 1);
    assert!(document.is_selected(mesh_id));
    assert!(document.is_selected(point_id));
    let Geometry::Brep(output) = document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(output.faces()[0].loops()[0].trims().len(), 4);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}
