//! curves join command regressions.
use super::*;

#[test]
fn joins_selected_linear_chains_and_preserves_disconnected_inputs() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Line 1,0 2,0",
        "Line 4,0 3,0",
        "Line 3,0 2,0",
        "Line 10,0 11,0",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    for id in &ids {
        document.select_object(*id, SelectionMode::Add).unwrap();
    }

    assert_eq!(
        registry.execute(&mut document, "Join").unwrap(),
        "Joined 3 curve(s) into 1 curve(s); 1 curve(s) unchanged"
    );
    assert_eq!(document.objects().len(), 2);
    assert!(document.object(ids[3]).is_some());
    assert!(document.object(ids[1]).is_none());
    let Geometry::Polyline(joined) = document
        .objects()
        .find(|object| object.id() != ids[3] && object.id() != ids[1])
        .unwrap()
        .geometry()
    else {
        panic!("expected a joined polyline")
    };
    assert_eq!(
        joined.vertices(),
        &[
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.undo_label(), Some("Join"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().len(), 2);
}

#[test]
fn join_chooses_straight_continuations_and_rejects_unsupported_geometry_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Line -1,0 0,0", "Line 0,0 1,0", "Line 0,0 0,1"] {
        registry.execute(&mut document, command).unwrap();
    }
    registry.execute(&mut document, "SelAll").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Join").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert!(document.object(ids[2]).is_some());
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>(),
        ids
    );
    registry.execute(&mut document, "Point 5,5").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Join"),
        Err(CommandError::UnsupportedJoinGeometry)
    ));
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.undo_label(), Some("Point"));
}

#[test]
fn explodes_selected_polylines_into_attribute_preserving_lines() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Construction")
        .unwrap();
    let construction = document.current_layer_id();
    registry
        .execute(&mut document, "Rectangle 0,0 4,3")
        .unwrap();
    registry.execute(&mut document, "SelLast").unwrap();
    registry
        .execute(&mut document, "SetObjectName Boundary")
        .unwrap();
    registry.execute(&mut document, "Group Outline").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry.execute(&mut document, "Point 9,9").unwrap();
    let point_id = document.objects().nth(1).unwrap().id();
    registry.execute(&mut document, "SelAll").unwrap();

    assert_eq!(
        registry.execute(&mut document, "X").unwrap(),
        "Exploded 1 polyline(s) into 4 line(s); 1 object(s) unchanged"
    );
    assert_eq!(document.objects().len(), 5);
    assert!(document.object(point_id).is_some());
    assert_eq!(
        document
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Line(_)))
            .count(),
        4
    );
    assert!(
        document
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Line(_)))
            .all(|object| {
                object.attributes().layer_id() == construction
                    && object.attributes().name() == Some("Boundary")
            })
    );
    let lines = document
        .objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Line(line) => Some((line.start(), line.end())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lines,
        vec![
            (
                Point3::try_new(0.0, 3.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(4.0, 3.0, 0.0).unwrap(),
                Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(4.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 3.0, 0.0).unwrap(),
            ),
            (
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            ),
        ]
    );
    assert_eq!(
        document.group_by_name("Outline").unwrap().members().count(),
        4
    );
    assert_eq!(document.selected_object_count(), 5);
    assert_eq!(document.undo_label(), Some("Explode"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Polyline(_)))
            .count(),
        1
    );
    assert_eq!(
        document.group_by_name("Outline").unwrap().members().count(),
        1
    );
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Explode"),
        Err(CommandError::NoExplodableObjects)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn explodes_polysurfaces_and_unwelded_mesh_parts_with_rhino_topology_and_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Shells").unwrap();
    let layer = document.current_layer_id();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let box_brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let box_id = document
        .add_geometry_with_attributes(
            Geometry::Brep(box_brep.clone()),
            ObjectAttributes::on_layer(layer)
                .with_name("Named shell")
                .with_object_color(ColorRgb::new(17, 83, 149))
                .try_with_wire_density(3)
                .unwrap(),
        )
        .unwrap();
    let box_group = document
        .add_group(Some("Shell group".to_owned()), [box_id])
        .unwrap();
    document
        .select_object(box_id, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Explode").unwrap(),
        "Exploded 1 polysurface(s) into 6 surface(s); 0 object(s) unchanged"
    );
    assert!(document.object(box_id).is_none());
    assert_eq!(document.objects().len(), 6);
    assert_eq!(document.selected_object_count(), 6);
    assert_eq!(document.group(box_group).unwrap().members().count(), 6);
    for (object, source_face) in document.objects().zip(box_brep.faces().iter().rev()) {
        assert_eq!(object.attributes().layer_id(), layer);
        assert_eq!(object.attributes().name(), Some("Named shell"));
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(17, 83, 149)
        );
        assert_eq!(
            object.attributes().color_source(),
            ObjectColorSource::Object
        );
        assert_eq!(object.attributes().wire_density(), 3);
        assert!(document.is_selected(object.id()));
        let Geometry::Brep(face) = object.geometry() else {
            panic!("exploded polysurface part must remain an exact B-rep face")
        };
        assert_eq!(face.faces().len(), 1);
        assert_eq!(face.faces()[0].surface(), source_face.surface());
        assert_eq!(face.faces()[0].is_reversed(), source_face.is_reversed());
        assert!(
            face.faces()[0].loops()[0]
                .trims()
                .iter()
                .all(|trim| trim.trim_type() == viboceros_geometry::BrepTrimType::Boundary)
        );
    }
    assert_eq!(document.undo_label(), Some("Explode"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.object(box_id).is_some());
    assert_eq!(
        document
            .group(box_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![box_id]
    );

    let mut document = Document::default();
    let unwelded = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 2.0).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let unwelded_id = document
        .add_geometry_with_attributes(
            Geometry::Mesh(unwelded.clone()),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("Fold"),
        )
        .unwrap();
    let mesh_group = document
        .add_group(Some("Mesh group".to_owned()), [unwelded_id])
        .unwrap();
    let welded = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(12.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 2.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 2.0).unwrap(),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let welded_id = document.add_geometry(Geometry::Mesh(welded)).unwrap();
    document
        .select_object(unwelded_id, SelectionMode::Replace)
        .unwrap();
    document
        .select_object(welded_id, SelectionMode::Add)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Explode").unwrap(),
        "Exploded 1 mesh(es) into 2 part(s); 1 object(s) unchanged"
    );
    assert!(document.object(unwelded_id).is_none());
    assert!(document.object(welded_id).is_some());
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.selected_object_count(), 3);
    let mesh_outputs = document
        .objects()
        .filter(|object| object.id() != welded_id)
        .collect::<Vec<_>>();
    assert_eq!(mesh_outputs.len(), 2);
    assert_eq!(
        document
            .group(mesh_group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        mesh_outputs
            .iter()
            .map(|object| object.id())
            .collect::<BTreeSet<_>>()
    );
    for object in &mesh_outputs {
        assert_eq!(object.attributes().name(), Some("Fold"));
        assert!(document.is_selected(object.id()));
    }
    let Geometry::Mesh(first) = mesh_outputs[0].geometry() else {
        panic!("first exploded mesh part must be a mesh")
    };
    let Geometry::Mesh(second) = mesh_outputs[1].geometry() else {
        panic!("second exploded mesh part must be a mesh")
    };
    assert_eq!(first.vertices(), &unwelded.vertices()[3..]);
    assert_eq!(second.vertices(), &unwelded.vertices()[..3]);
    registry.execute(&mut document, "Undo").unwrap();
    assert!(document.object(unwelded_id).is_some());
    assert!(document.object(welded_id).is_some());
    assert_eq!(
        document
            .group(mesh_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![unwelded_id]
    );
    document
        .select_objects_direct([welded_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Explode"),
        Err(CommandError::NoExplodableObjects)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn three_dm_keeps_polyline_and_nurbs_representations_distinct() {
    let points = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 1.0, 0.0).unwrap(),
    ];
    let polyline = Polyline3::try_new(points.clone(), Tolerance::DEFAULT).unwrap();
    assert_eq!(
        document_geometry_from_3dm(ThreeDmGeometry::Polyline(polyline.clone())),
        Geometry::Polyline(polyline)
    );

    let nonuniform = NurbsCurve::try_new(1, points, vec![0.0, 0.0, 0.25, 1.0, 1.0]).unwrap();
    assert_eq!(
        document_geometry_from_3dm(ThreeDmGeometry::NurbsCurve(nonuniform.clone())),
        Geometry::NurbsCurve(nonuniform)
    );
}
