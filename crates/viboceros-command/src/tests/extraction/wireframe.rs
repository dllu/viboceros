//! extraction wireframe command regressions.
use super::*;

#[test]
fn extract_wireframe_matches_surface_density_and_preserves_exact_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Wires").unwrap();
    let input_layer = document.current_layer_id();
    let surface = rational_multi_span_surface();
    let source = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface.clone()),
            ObjectAttributes::on_layer(input_layer)
                .try_with_wire_density(2)
                .unwrap(),
        )
        .unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractWireframe OutputLayer=Input GroupOutput=Yes",
            )
            .unwrap(),
        "Extracted 10 wireframe curve(s) from 1 object(s); grouped output"
    );
    assert!(!document.is_selected(source));
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 10);
    assert!(outputs.iter().all(|object| {
        object.attributes().layer_id() == input_layer
            && document.is_selected(object.id())
            && matches!(object.geometry(), Geometry::NurbsCurve(_))
    }));
    assert_eq!(document.groups().len(), 1);
    assert_eq!(document.groups().next().unwrap().members().len(), 10);

    for object in &outputs[4..] {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            unreachable!()
        };
        let midpoint = curve
            .evaluate((*curve.domain().start() + *curve.domain().end()) * 0.5)
            .unwrap();
        assert!(
            surface
                .closest_parameters(midpoint, document.tolerance())
                .is_ok()
        );
    }
    assert_eq!(document.undo_label(), Some("ExtractWireframe"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.groups().len(), 0);
}

#[test]
fn extract_wireframe_deduplicates_brep_and_mesh_topology_and_groups_the_batch() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Solids").unwrap();
    let solid_layer = document.current_layer_id();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let solid = document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                frame,
                [[0.0, 2.0], [0.0, 3.0], [0.0, 5.0]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    registry.execute(&mut document, "Layer New Meshes").unwrap();
    let mesh_layer = document.current_layer_id();
    let mesh = document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(12.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(12.0, 2.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(12.0, 2.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 2.0, 0.0).unwrap(),
                ],
                vec![[0, 1, 2], [3, 4, 5]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([solid, mesh], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractWireframe GroupOutput _Yes OutputLayer _TargetObject",
            )
            .unwrap(),
        "Extracted 29 wireframe curve(s) from 2 object(s); grouped output"
    );
    let outputs = document.objects().skip(2).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 29);
    assert!(outputs[..24].iter().all(|object| {
        object.attributes().layer_id() == solid_layer
            && matches!(object.geometry(), Geometry::NurbsCurve(_))
    }));
    assert!(outputs[24..].iter().all(|object| {
        object.attributes().layer_id() == mesh_layer
            && matches!(object.geometry(), Geometry::Line(_))
    }));
    assert_eq!(document.groups().len(), 1);
    assert_eq!(document.groups().next().unwrap().members().len(), 29);
    assert_eq!(document.selected_object_count(), 29);
}

#[test]
fn extract_wireframe_rejects_bad_inputs_and_options_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "ExtractWireframe"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ExtractWireframe"),
        Err(CommandError::UnsupportedExtractWireframeGeometry)
    ));
    for invalid in [
        "ExtractWireframe OutputLayer",
        "ExtractWireframe OutputLayer=Other",
        "ExtractWireframe OutputLayer=Input OutputLayer=Current",
        "ExtractWireframe GroupOutput=Maybe",
        "ExtractWireframe GroupOutput=Yes GroupOutput=No",
        "ExtractWireframe Other=Yes",
        "ExtractWireframe stray",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn polysurface_selection_is_distinct_from_surface_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0 2,0 2,2 0,2")
        .unwrap();
    registry
        .execute(&mut document, "Box 4,0,0 6,2,0 3")
        .unwrap();
    let planar_boundary = Polyline3::try_new(
        vec![
            Point3::try_new(8.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 2.0, 0.0).unwrap(),
            Point3::try_new(8.0, 2.0, 0.0).unwrap(),
            Point3::try_new(8.0, 0.0, 0.0).unwrap(),
        ],
        document.tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    document
        .add_geometry(Geometry::Brep(
            Brep::try_planar_face(&planar_boundary, document.tolerance()).unwrap(),
        ))
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    assert_eq!(
        registry.execute(&mut document, "SelSrf").unwrap(),
        "Selected 2 object(s)"
    );
    assert!(
        document
            .selected_objects()
            .all(|object| match object.geometry() {
                Geometry::NurbsSurface(_) => true,
                Geometry::Brep(brep) => brep.faces().len() == 1,
                _ => false,
            })
    );
    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelPolysurface").unwrap(),
        "Selected 1 object(s)"
    );
    assert!(
        document.selected_objects().all(
            |object| matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() > 1)
        )
    );
    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelOpenPolysrf").unwrap(),
        "Selected 0 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelClosedPolysrf").unwrap(),
        "Selected 1 object(s)"
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
