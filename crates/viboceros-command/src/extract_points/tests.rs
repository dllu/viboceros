use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn extract_points_cloud_metadata_follows_first_selected_source_and_preserves_storage() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Layer New Input").unwrap();
    let input_layer = doc.current_layer_id();
    registry.execute(&mut doc, "Line 0,0,0 1,2,3").unwrap();
    let line = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "SetObjectName Guide").unwrap();
    registry.execute(&mut doc, "Layer New Output").unwrap();
    let output_layer = doc.current_layer_id();
    let ignored = doc
        .add_geometry(Geometry::Point(point(9., 8., 7.)))
        .unwrap();
    let snapshot = doc.object(line).unwrap().geometry_snapshot().clone();
    for first_contributes in [false, true] {
        let ids = if first_contributes {
            [line, ignored]
        } else {
            [ignored, line]
        };
        doc.clear_selection();
        doc.select_object(ids[0], SelectionMode::Replace).unwrap();
        doc.select_object(ids[1], SelectionMode::Add).unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(&mut doc, "ExtractPt Output=PointCloud")
            .unwrap();
        let output = doc.selected_objects().next().unwrap();
        assert_eq!(
            output.attributes().layer_id(),
            if first_contributes {
                input_layer
            } else {
                output_layer
            }
        );
        assert_eq!(
            output.attributes().name(),
            if first_contributes {
                Some("Guide")
            } else {
                None
            }
        );
        let Geometry::PointCloud(cloud) = output.geometry() else {
            panic!()
        };
        assert_eq!(cloud.points(), [point(0., 0., 0.), point(1., 2., 3.)]);
        assert!(
            doc.object(line)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&snapshot)
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        assert!(
            doc.object(line)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&snapshot)
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(
            doc.object(line)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&snapshot)
        );
        registry.execute(&mut doc, "Undo").unwrap();
    }
}

#[test]
fn extract_points_cumulative_limit_rejects_a_late_source_without_consuming_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 1,0,0").unwrap();
    let cloud = PointCloud3::try_new(vec![point(2., 3., 4.); MAX_EXTRACTED_POINTS - 1]).unwrap();
    doc.add_geometry(Geometry::PointCloud(cloud)).unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    doc.clear_history().unwrap();
    doc.add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    doc.undo().unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    for output in ["Points", "PointCloud"] {
        assert!(matches!(
            registry.execute(&mut doc, &format!("ExtractPt Output={output}")),
            Err(CommandError::TooManyExtractedPoints {
                maximum: MAX_EXTRACTED_POINTS
            })
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(doc.redo_label(), redo.as_deref());
        assert!(!doc.can_undo());
    }
}

#[test]
fn extract_points_prompt_uses_the_typed_parser_and_filters_non_sources() {
    let registry = CommandRegistry::with_builtins();
    let mut prompt = registry
        .object_selection_prompt("ExtractPt _OutputLayer _Current")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::ExtractPoints);
    prompt.update_options("Output=PointCloud").unwrap();
    assert_eq!(
        prompt.command_line(),
        "ExtractPt OutputLayer=Current Output=PointCloud SubDLocation=Surface"
    );
    let mut doc = Document::default();
    let point = doc
        .add_geometry(Geometry::Point(point(1., 2., 3.)))
        .unwrap();
    let empty = doc.add_geometry(Geometry::EmptyMesh).unwrap();
    registry.execute(&mut doc, "Line 0,0 1,0").unwrap();
    let line = doc.objects().last().unwrap().id();
    assert!(!prompt.filter.accepts_object(doc.object(point).unwrap()));
    assert!(!prompt.filter.accepts_object(doc.object(empty).unwrap()));
    assert!(prompt.filter.accepts_object(doc.object(line).unwrap()));
    for input in [
        "ExtractPt Output=Bad",
        "ExtractPt Output=Points Output=PointCloud",
        "ExtractPt OutputLayer",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.object_selection_prompt(input).is_err());
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn component_validation_is_atomic_and_whole_owner_is_overridden() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let mesh = viboceros_geometry::TriangleMesh::try_from_face_records(
        vec![point(0., 0., 0.), point(2., 0., 0.), point(0., 2., 0.)],
        vec![viboceros_geometry::MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    doc.add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for suffix in [
        format!("Component={id},Face,3"),
        format!("Component={id},Vertex,99"),
        format!("Component={id},Edge,99"),
        format!("Component={id},Bogus,0"),
        "Component=not-a-uuid,Vertex,0".into(),
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("ExtractPt Component={id},Face,0 {suffix}")
                )
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        assert!(doc.can_redo());
    }
    registry
        .execute(
            &mut doc,
            &format!("ExtractPt Component={id},Vertex,0 Component={id},Vertex,0"),
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.selected_object_count(), 2);
    assert_eq!(
        doc.objects().last().unwrap().geometry(),
        &Geometry::Point(point(0., 0., 0.))
    );
}
