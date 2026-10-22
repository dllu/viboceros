//! extraction border command regressions.
use super::*;

#[test]
fn duplicate_edges_reject_collapsed_surface_without_history_changes() {
    let registry = CommandRegistry::with_builtins();
    let mut collapsed_document = Document::default();
    let origin = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    let collapsed = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![origin; 4],
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let collapsed_id = collapsed_document
        .add_geometry(Geometry::NurbsSurface(collapsed))
        .unwrap();
    collapsed_document
        .select_objects_direct([collapsed_id], SelectionMode::Replace)
        .unwrap();
    let history = collapsed_document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut collapsed_document, "DupEdge Edges=All"),
        Err(CommandError::NoDuplicateEdges)
    ));
    assert_eq!(collapsed_document.objects().len(), 1);
    assert_eq!(collapsed_document.undo_label(), history.as_deref());
}

#[test]
fn duplicate_border_extracts_exact_surface_boundaries_and_respects_output_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,2 4,0,2 4,3,2 0,3,2")
        .unwrap();
    let source = document.objects().next().unwrap().id();
    let input_layer = document.object(source).unwrap().attributes().layer_id();
    registry
        .execute(&mut document, "Layer New Borders")
        .unwrap();
    let border_layer = document.current_layer_id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    let message = registry.execute(&mut document, "DupBorder").unwrap();
    assert!(message.contains("1 border curve(s) in 1 border(s)"));
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1);
    assert_eq!(document.groups().len(), 0);
    assert!(!document.is_selected(source));
    assert_eq!(outputs[0].attributes().layer_id(), border_layer);
    assert!(document.is_selected(outputs[0].id()));
    let Geometry::Polyline(curve) = outputs[0].geometry() else {
        panic!("rectangular border must be one native polyline")
    };
    assert_eq!(curve.domain(), 0.0..=14.0);
    assert_eq!(
        curve
            .vertices()
            .iter()
            .map(|p| p.to_array())
            .collect::<Vec<_>>(),
        vec![
            [0., 3., 2.],
            [0., 0., 2.],
            [4., 0., 2.],
            [4., 3., 2.],
            [0., 3., 2.]
        ]
    );
    assert_eq!(document.undo_label(), Some("DupBorder"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.groups().len(), 0);
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "DuplicateBorder OutputLayer _Input")
        .unwrap();
    assert!(
        document
            .objects()
            .skip(1)
            .all(|object| object.attributes().layer_id() == input_layer)
    );
}

#[test]
fn duplicate_border_preserves_brep_holes_and_welds_mesh_borders() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let normal = UnitVector3::try_new(0.0, 0.0, 1.0, document.tolerance()).unwrap();
    let center = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    let outer = Circle3::try_new(center, 5.0, normal, document.tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let inner = Circle3::try_new(center, 2.0, normal, document.tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let face = Brep::try_planar_face_with_holes(
        &outer,
        std::slice::from_ref(&inner),
        document.tolerance(),
    )
    .unwrap();
    let face_id = document.add_geometry(Geometry::Brep(face)).unwrap();
    document
        .select_objects_direct([face_id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "DupBorder").unwrap();

    let exact_borders = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(exact_borders.len(), 2);
    assert_eq!(document.groups().len(), 0);
    let mut radii = exact_borders
        .iter()
        .map(|object| {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("a B-rep edge must remain an exact NURBS curve")
            };
            assert!(curve.is_rational());
            assert!(curve.is_closed().unwrap());
            curve.evaluate(*curve.domain().start()).unwrap().x().abs()
        })
        .collect::<Vec<_>>();
    radii.sort_by(Real::total_cmp);
    assert_eq!(radii, [2.0, 5.0]);

    let mut mesh_document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
            Point3::try_new(3.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(3.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        mesh_document.tolerance(),
    )
    .unwrap();
    let mesh_id = mesh_document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    mesh_document
        .select_objects_direct([mesh_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut mesh_document, "DupBorder OutputLayer=Current")
        .unwrap();
    assert_eq!(mesh_document.objects().len(), 2);
    let Geometry::Polyline(border) = mesh_document.objects().last().unwrap().geometry() else {
        panic!("a mesh border must be one welded polyline")
    };
    assert!(border.is_closed());
    assert_eq!(border.segment_count(), 4);
    assert_eq!(border.length().unwrap(), 10.0);
}

#[test]
fn duplicate_border_rejects_unsupported_or_borderless_inputs_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Torus 0,0,0 4 1").unwrap();
    let torus = document.objects().next().unwrap().id();
    document
        .select_objects_direct([torus], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupBorder"),
        Err(CommandError::NoDuplicateBorders)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());

    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().last().unwrap().id();
    document
        .select_objects_direct([torus, line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupBorder"),
        Err(CommandError::UnsupportedDuplicateBorderGeometry)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 0);
    assert_eq!(document.undo_label(), history.as_deref());
    for invalid in [
        "DupBorder OutputLayer=Other",
        "DupBorder OutputLayer",
        "DupBorder OutputLayer=Input extra",
        "DupBorder OutputLayer=Input OutputLayer=Current",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn duplicate_edge_preserves_order_and_uses_fresh_output_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let brep = Brep::try_box(
        frame,
        [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
        document.tolerance(),
    )
    .unwrap();
    let expected = [brep.edges()[2].curve(), brep.edges()[0].curve()].map(Clone::clone);
    let source = document
        .add_geometry_with_attributes(
            Geometry::Brep(brep),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Colored shell")
                .with_object_color(ColorRgb::new(17, 83, 149)),
        )
        .unwrap();
    let source_group = document
        .add_group(Some("Source group".to_owned()), [source])
        .unwrap();
    registry.execute(&mut document, "Layer New Output").unwrap();
    let output_layer = document.current_layer_id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DupEdge Edges=2,0 OutputLayer=Current")
            .unwrap(),
        "Duplicated 2 edge curve(s) from 1 object(s)"
    );
    assert!(!document.is_selected(source));
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for (output, expected) in outputs.iter().zip(expected) {
        assert!(document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert!(
            document
                .group(source_group)
                .unwrap()
                .members()
                .all(|member| member != output.id())
        );
        assert!(matches!(output.geometry(), Geometry::NurbsCurve(curve) if curve == &expected));
    }
    assert_eq!(document.undo_label(), Some("DupEdge"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn duplicate_edge_point_mode_includes_surface_seams_and_mesh_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut surface_document = Document::default();
    registry
        .execute(&mut surface_document, "Layer New Cylinders")
        .unwrap();
    let input_layer = surface_document.current_layer_id();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        surface_document.tolerance(),
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 2.0, -1.0, 4.0).unwrap();
    let expected_edges = cylinder.natural_edge_curves().unwrap();
    let expected_seam = expected_edges[1].clone();
    let cylinder_id = surface_document
        .add_geometry(Geometry::NurbsSurface(cylinder))
        .unwrap();
    registry
        .execute(&mut surface_document, "Layer Current Default")
        .unwrap();
    surface_document
        .select_objects_direct([cylinder_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut surface_document,
                "DuplicateEdge 2.1,0,1.5 OutputLayer=Input",
            )
            .unwrap(),
        "Duplicated 1 edge curve(s) from 1 object(s)"
    );
    let seam = surface_document.objects().last().unwrap();
    assert_eq!(seam.attributes().layer_id(), input_layer);
    assert!(matches!(seam.geometry(), Geometry::NurbsCurve(curve) if curve == &expected_seam));
    registry.execute(&mut surface_document, "Undo").unwrap();
    surface_document
        .select_objects_direct([cylinder_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut surface_document, "DupEdge Edges=All")
        .unwrap();
    let all_edges = surface_document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(all_edges.len(), 3);
    for (output, expected) in all_edges.iter().zip(expected_edges) {
        assert!(matches!(output.geometry(), Geometry::NurbsCurve(curve) if curve == &expected));
    }

    let mut mesh_document = Document::default();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
        mesh_document.tolerance(),
    )
    .unwrap();
    let expected_left = mesh.wireframe_lines(mesh_document.tolerance()).unwrap()[1];
    let mesh_id = mesh_document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    mesh_document
        .select_objects_direct([mesh_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut mesh_document, "DupEdge -0.1,2,0")
        .unwrap();
    let output = mesh_document.objects().last().unwrap();
    assert!(matches!(output.geometry(), Geometry::Line(line) if line == &expected_left));
    assert!(mesh_document.is_selected(output.id()));
    assert!(!mesh_document.is_selected(mesh_id));
}

#[test]
fn duplicate_edge_rejects_bad_selection_and_arguments_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "DupEdge Edges=All"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().next().unwrap().id();
    document
        .select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupEdge Edges=All"),
        Err(CommandError::UnsupportedDuplicateEdgeGeometry)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());

    registry
        .execute(&mut document, "SrfPt 0,0 4,0 4,3 0,3")
        .unwrap();
    let surface = document.objects().last().unwrap().id();
    document
        .select_objects_direct([surface], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupEdge Edges=4"),
        Err(CommandError::DuplicateEdgeIndexOutOfRange {
            edge: 4,
            edge_count: 4
        })
    ));
    for invalid in [
        "DupEdge",
        "DupEdge Edges=",
        "DupEdge Edges=0,0",
        "DupEdge Edges=0 1,2,3",
        "DupEdge 1,2,3 Edges=0",
        "DupEdge OutputLayer=Other Edges=0",
        "DupEdge Edges=0 OutputLayer=Input OutputLayer=Current",
        "DupEdge EdgeIndices=0 Edges=1",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}
