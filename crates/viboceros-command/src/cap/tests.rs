use super::*;
mod orientation;

fn source(document: &mut Document) -> ObjectId {
    let solid = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry_with_attributes(
            Geometry::Brep(
                solid
                    .sub_brep(&[0, 1, 2, 3, 4], document.tolerance())
                    .unwrap(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name("Source")
                .with_object_color(ColorRgb::new(11, 22, 33)),
        )
        .unwrap()
}

#[test]
fn cap_is_atomic_in_place_with_attributes_groups_selection_and_history() {
    for preselect in [false, true] {
        let mut doc = Document::default();
        let id = source(&mut doc);
        let group = doc.add_group(Some("source".into()), [id]).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let registry = CommandRegistry::with_builtins();
        if preselect {
            registry.execute(&mut doc, "Cap").unwrap();
        } else {
            registry
                .execute_postselected(&mut doc, "Cap", Default::default())
                .unwrap();
        }
        let after = doc.object(id).unwrap().clone();
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(after.attributes(), before.attributes());
        assert_eq!(after.group_ids(), &[group]);
        let Geometry::Brep(brep) = after.geometry() else {
            panic!("expected brep")
        };
        assert!(brep.is_solid());
        assert_eq!(doc.is_selected(id), preselect);
        assert_eq!(doc.undo_label(), Some("Cap"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}

#[test]
fn unsupported_inputs_and_arguments_roll_back_every_object() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    assert!(matches!(
        registry.execute(&mut doc, "Cap"),
        Err(CommandError::NoObjectsSelected)
    ));
    let id = source(&mut doc);
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut doc, "Cap"),
        Err(CommandError::UnsupportedCapGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), history.as_deref());
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut doc, "Cap DeleteInput=Yes"),
        Err(CommandError::Usage("Cap"))
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn planar_sheet_noop_preserves_surface_representation_and_creates_no_history() {
    let mut doc = Document::default();
    let solid = Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 1.]; 3],
        doc.tolerance(),
    )
    .unwrap();
    let surface = solid.faces()[0].surface().clone();
    let id = doc
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    let history = doc.undo_label().map(str::to_owned);
    registry
        .execute_postselected(&mut doc, "Cap", Default::default())
        .unwrap();
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert!(!doc.is_selected(id));
    assert_eq!(doc.undo_label(), history.as_deref());
    let prompt = CapCommand::default()
        .object_selection_prompt(&[])
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Cap);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(prompt.filter.accepts_object(doc.object(id).unwrap()));
    assert!(
        CapCommand::default()
            .object_selection_prompt(&["Triangles=Yes"])
            .is_err()
    );
}

#[test]
fn cap_mesh_fills_only_planar_openings_and_preserves_object_history() {
    let mut doc = Document::default();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(2., 2., 0.5),
            point(0., 2., 0.),
            point(1., 1., -2.),
            point(10., 0., 0.),
            point(12., 0., 0.),
            point(10., 2., 0.),
            point(10., 0., -2.),
        ],
        vec![
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
            [5, 6, 8],
            [6, 7, 8],
            [7, 5, 8],
        ],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    assert!(
        CapCommand::default()
            .object_selection_prompt(&[])
            .unwrap()
            .unwrap()
            .filter
            .accepts_object(&before)
    );
    registry.execute(&mut doc, "Cap").unwrap();
    let after = doc.object(id).unwrap().clone();
    let Geometry::Mesh(result) = after.geometry() else {
        panic!("expected mesh")
    };
    assert_eq!(result.face_count(), 8);
    assert_eq!(result.topology().boundary_edge_count(), 4);
    assert_eq!(after.attributes(), before.attributes());
    assert!(doc.is_selected(id));
    assert_eq!(doc.undo_label(), Some("Cap"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}

#[test]
fn cap_mesh_delete_input_no_retains_source_and_copies_attributes_and_groups() {
    let mut doc = Document::default();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 4.),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        doc.tolerance(),
    )
    .unwrap();
    let attrs = ObjectAttributes::on_layer(doc.current_layer_id())
        .with_name("Open mesh")
        .with_object_color(ColorRgb::new(17, 29, 43))
        .try_with_user_text("code", "attribute")
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(Geometry::Mesh(mesh), attrs.clone())
        .unwrap();
    doc.set_object_geometry_user_text([id], "code", Some("geometry"))
        .unwrap();
    let group = doc.add_group(Some("assembly".to_owned()), [id]).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "Cap DeleteInput=No Crease=No")
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.object(id), Some(&before));
    let copied = doc
        .objects()
        .find(|object| object.id() != id)
        .unwrap()
        .clone();
    assert_eq!(copied.attributes(), &attrs);
    assert!(copied.geometry_user_text().is_empty());
    assert!(
        matches!(copied.geometry(), Geometry::Mesh(mesh) if mesh.topology().is_solid() && mesh.vertices().len() == 4)
    );
    assert!(doc.is_selected(id));
    assert!(!doc.is_selected(copied.id()));
    assert_eq!(
        doc.group(group).unwrap().members().collect::<BTreeSet<_>>(),
        BTreeSet::from([id, copied.id()])
    );
    assert!(
        !registry
            .object_selection_prompt("Cap")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert!(
        !registry
            .object_selection_prompt("Cap")
            .unwrap()
            .unwrap()
            .options[1]
            .value
    );
    assert_eq!(doc.undo_label(), Some("Cap"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    assert!(doc.object(copied.id()).is_none());
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(copied.id()), Some(&copied));
}

#[test]
fn cap_mesh_crease_no_welds_boundary_and_remembers_choice() {
    let mut doc = Document::default();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 4.),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    assert!(
        registry
            .object_selection_prompt("Cap")
            .unwrap()
            .unwrap()
            .options[1]
            .value
    );
    registry.execute(&mut doc, "Cap Crease=No").unwrap();
    let after = doc.object(id).unwrap().clone();
    let Geometry::Mesh(welded) = after.geometry() else {
        panic!("expected mesh")
    };
    assert_eq!(welded.vertices().len(), 4);
    assert!(welded.topology().is_solid());
    assert!(
        welded
            .filtered_edge_polylines(
                viboceros_geometry::MeshEdgeFilter::Unwelded,
                doc.tolerance()
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        !registry
            .object_selection_prompt("Cap")
            .unwrap()
            .unwrap()
            .options[1]
            .value
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}

#[test]
fn cap_mixed_sources_copy_mesh_and_replace_brep_in_one_transaction() {
    let mut doc = Document::default();
    let brep_id = source(&mut doc);
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(10., 0., 0.).unwrap(),
            Point3::try_new(14., 0., 0.).unwrap(),
            Point3::try_new(10., 4., 0.).unwrap(),
            Point3::try_new(10., 0., 4.).unwrap(),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        doc.tolerance(),
    )
    .unwrap();
    let mesh_id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_objects_direct([brep_id, mesh_id], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Cap DeleteInput=No").unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.object(mesh_id), Some(&before[1]));
    assert!(
        matches!(doc.object(brep_id).unwrap().geometry(), Geometry::Brep(brep) if brep.is_solid())
    );
    assert_eq!(doc.undo_label(), Some("Cap"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 3);
    for invalid in [
        "Cap DeleteInput=Maybe",
        "Cap DeleteInput=No DeleteInput=Yes",
        "Cap Crease=Maybe",
        "Cap Crease=Yes Crease=No",
    ] {
        let unchanged = format!("{doc:?}");
        assert!(registry.execute(&mut doc, invalid).is_err());
        assert_eq!(format!("{doc:?}"), unchanged);
    }
}

#[test]
fn cap_mesh_triangulates_annular_openings_in_one_undo_step() {
    let mut doc = Document::default();
    let mut vertices = Vec::new();
    let outer = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
    let inner = [[1., 1.], [3., 1.], [3., 3.], [1., 3.]];
    for (ring, z) in [(outer, 0.), (outer, 2.), (inner, 0.), (inner, 2.)] {
        vertices.extend(ring.map(|[x, y]| Point3::try_new(x, y, z).unwrap()));
    }
    let mut faces = Vec::new();
    for side in 0..4_u32 {
        let next = (side + 1) % 4;
        faces.push(viboceros_geometry::MeshFace::Quad([
            side,
            next,
            next + 4,
            side + 4,
        ]));
        faces.push(viboceros_geometry::MeshFace::Quad([
            side + 8,
            side + 12,
            next + 12,
            next + 8,
        ]));
    }
    let wall = TriangleMesh::try_new_faces(vertices, faces, doc.tolerance()).unwrap();
    let id = doc.add_geometry(Geometry::Mesh(wall)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Cap").unwrap();
    let after = doc.object(id).unwrap().clone();
    let Geometry::Mesh(capped) = after.geometry() else {
        panic!("expected capped mesh")
    };
    assert!(capped.topology().is_solid());
    assert!((capped.signed_volume().unwrap() - 24.).abs() < 1e-12);
    assert_eq!(doc.undo_label(), Some("Cap"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}

#[test]
fn kink_boundary_subdivision_is_part_of_one_identity_preserving_history_step() {
    let mut doc = Document::default();
    let profile = NurbsCurve::try_clamped_uniform(
        1,
        [[0., 0., 0.], [4., 0., 0.], [0., 3., 0.], [0., 0., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
    )
    .unwrap();
    let solid = Brep::try_extruded_curve(
        &profile,
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 2., 5.).unwrap(),
        doc.tolerance(),
    )
    .unwrap();
    let wall = solid.sub_brep(&[0], doc.tolerance()).unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::Brep(wall),
            ObjectAttributes::on_layer(doc.current_layer_id()).with_name("kinked wall"),
        )
        .unwrap();
    let group = doc.add_group(Some("profile".into()), [id]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Cap").unwrap();
    let after = doc.object(id).unwrap().clone();
    assert_eq!(after.attributes(), before.attributes());
    assert_eq!(after.group_ids(), &[group]);
    assert!(doc.is_selected(id));
    assert_eq!(doc.objects().len(), 1);
    let Geometry::Brep(brep) = after.geometry() else {
        panic!("expected brep")
    };
    assert!(brep.is_solid());
    assert_eq!(brep.faces().len(), 3);
    assert_eq!(brep.edges().len(), 7);
    assert_eq!(brep.vertices().len(), 6);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id), Some(&before));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id), Some(&after));
}
