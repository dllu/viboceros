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
fn use_ngons_merges_planar_face_region_after_component_split() {
    let square = TriangleMesh::try_new(
        vec![
            Point3::try_new(10., 0., 0.).unwrap(),
            Point3::try_new(11., 0., 0.).unwrap(),
            Point3::try_new(10., 1., 0.).unwrap(),
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
            Point3::try_new(2., 2., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5], [3, 5, 6]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![viboceros_geometry::MeshNgon::from_parts(
        vec![3, 4, 5, 6],
        vec![1, 2],
    )])
    .unwrap();
    for (choice, expected_faces) in [("UseNgons=Yes", 1), ("UseNgons=No", 2)] {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Mesh(square.clone()))
            .unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        MeshToNurbCommand::default()
            .run(&mut document, &[choice])
            .unwrap();
        assert_eq!(document.objects().len(), 3);
        let Geometry::Brep(first) = document.objects().nth(1).unwrap().geometry() else {
            panic!("MeshToNURB must create a B-rep")
        };
        assert_eq!(first.faces().len(), 1);
        let Geometry::Brep(output) = document.objects().last().unwrap().geometry() else {
            panic!("MeshToNURB must create a B-rep")
        };
        assert_eq!(output.faces().len(), expected_faces);
        if expected_faces == 1 {
            assert_eq!(output.faces()[0].loops()[0].trims().len(), 4);
            assert_eq!(output.edges().len(), 4);
        }
    }
}

#[test]
fn use_ngons_handles_concave_boundaries_and_nonplanar_fallback() {
    let concave = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(3., 0., 0.).unwrap(),
            Point3::try_new(3., 3., 0.).unwrap(),
            Point3::try_new(1.5, 1.5, 0.).unwrap(),
            Point3::try_new(0., 3., 0.).unwrap(),
        ],
        vec![[0, 1, 3], [1, 2, 3], [0, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![viboceros_geometry::MeshNgon::from_parts(
        vec![0, 1, 2, 3, 4],
        vec![0, 1, 2],
    )])
    .unwrap();
    let merged = Brep::try_from_mesh_with_ngons(&concave, true, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(merged.faces().len(), 1);
    assert_eq!(merged.edges().len(), 5);
    assert_eq!(merged.faces()[0].loops()[0].trims().len(), 5);

    let mut vertices = concave.vertices().to_vec();
    vertices[3] = Point3::try_new(1.5, 1.5, 1.).unwrap();
    let warped = TriangleMesh::try_new(
        vertices,
        vec![[0, 1, 3], [1, 2, 3], [0, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(concave.ngons().to_vec())
    .unwrap();
    let expanded = Brep::try_from_mesh_with_ngons(&warped, true, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(expanded.faces().len(), 3);
}

#[test]
fn planar_ngon_cap_keeps_closed_brep_topology() {
    let pyramid = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
            Point3::try_new(2., 2., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
            Point3::try_new(1., 1., 2.).unwrap(),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![viboceros_geometry::MeshNgon::from_parts(
        vec![0, 3, 2, 1],
        vec![0, 1],
    )])
    .unwrap();
    let merged = Brep::try_from_mesh_with_ngons(&pyramid, true, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(merged.faces().len(), 5);
    assert!(merged.is_solid());
    assert!((merged.signed_volume(Tolerance::DEFAULT).unwrap() - 8. / 3.).abs() < 1e-8);
}

#[test]
fn nonplanar_ngon_splits_into_connected_planar_regions() {
    let folded = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 1.).unwrap(),
            Point3::try_new(2., 1., 1.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
        ],
        vec![[0, 1, 4], [0, 4, 5], [1, 2, 3], [1, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![viboceros_geometry::MeshNgon::from_parts(
        vec![0, 1, 2, 3, 4, 5],
        vec![0, 1, 2, 3],
    )])
    .unwrap();
    for trim_triangles in [false, true] {
        let merged =
            Brep::try_from_mesh_with_ngons(&folded, trim_triangles, true, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(merged.faces().len(), 2);
        assert_eq!(merged.edges().len(), 7);
        assert_eq!(merged.faces()[0].loops()[0].trims().len(), 4);
        assert_eq!(merged.faces()[1].loops()[0].trims().len(), 4);
        let expanded =
            Brep::try_from_mesh_with_ngons(&folded, trim_triangles, false, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(expanded.faces().len(), 4);
    }
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
