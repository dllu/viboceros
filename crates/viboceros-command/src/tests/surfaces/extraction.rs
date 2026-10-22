//! surfaces extraction command regressions.
use super::*;

#[test]
fn qualified_surface_face_commands_only_change_the_target_source() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "SrfPt 0,0,0 1,0,0 1,1,0 0,1,0")
        .unwrap();
    let first = document.objects().next().unwrap().id();
    registry
        .execute(&mut document, "SrfPt 3,0,0 4,0,0 4,1,0 3,1,0")
        .unwrap();
    let second = document
        .objects()
        .find(|object| object.id() != first)
        .unwrap()
        .id();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let original_count = document.objects().len();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        format!("ExtractSrf Face=1 Object={second}"),
        format!("DupFaceBorder Face=1 Object={second}"),
        format!("ExtractSrf Face=0 Object={second} Faces=All"),
    ] {
        assert!(
            registry.execute(&mut document, &command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), original_count);
        assert_eq!(document.undo_label(), history.as_deref());
    }
    document
        .select_objects_direct([first], SelectionMode::Replace)
        .unwrap();
    for command in [
        format!("ExtractSrf Face=0 Object={second}"),
        format!("DupFaceBorder Face=0 Object={second}"),
    ] {
        assert!(matches!(
            registry.execute(&mut document, &command),
            Err(CommandError::Usage(_))
        ));
        assert_eq!(document.objects().len(), original_count);
    }
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("ExtractSrf Face=0 Object={second} Copy=No")
            )
            .unwrap(),
        "Extracted 1 surface(s) from 1 object(s); source faces removed"
    );
    assert!(document.object(first).is_some());
    assert!(document.object(second).is_none());
    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("DupFaceBorder Face=0 Object={second}")
            )
            .unwrap(),
        "Duplicated 1 curve object(s) in 1 face border(s) from 1 object(s)"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(document.object(first).is_some());
    assert!(document.object(second).is_some());
}

#[test]
fn extracts_brep_faces_in_order_while_preserving_remainder_identity_and_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
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
    let source = document
        .add_geometry_with_attributes(
            Geometry::Brep(box_brep.clone()),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Panel shell")
                .with_object_color(ColorRgb::new(17, 83, 149))
                .try_with_wire_density(3)
                .unwrap(),
        )
        .unwrap();
    let group = document
        .add_group(Some("Shell group".to_owned()), [source])
        .unwrap();
    registry.execute(&mut document, "Layer New Output").unwrap();
    let output_layer = document.current_layer_id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractSrf Faces=2,0 Copy=No OutputLayer=Current",
            )
            .unwrap(),
        "Extracted 2 surface(s) from 1 object(s); source faces removed"
    );
    assert_eq!(document.objects().len(), 3);
    assert!(!document.is_selected(source));
    let source_object = document.object(source).unwrap();
    assert_eq!(source_object.attributes().layer_id(), input_layer);
    assert_eq!(source_object.attributes().name(), Some("Panel shell"));
    let Geometry::Brep(remainder) = source_object.geometry() else {
        panic!("ExtractSrf remainder must retain exact B-rep geometry")
    };
    assert_eq!(
        remainder,
        &box_brep
            .sub_brep(&[1, 3, 4, 5], Tolerance::DEFAULT)
            .unwrap()
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );

    let outputs = document
        .objects()
        .filter(|object| object.id() != source)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for object in &outputs {
        assert!(document.is_selected(object.id()));
        assert_eq!(object.attributes().layer_id(), output_layer);
        assert_eq!(object.attributes().name(), Some("Panel shell"));
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(17, 83, 149)
        );
        assert_eq!(
            object.attributes().color_source(),
            ObjectColorSource::Object
        );
        assert_eq!(object.attributes().wire_density(), 3);
        assert!(
            document
                .group(group)
                .unwrap()
                .members()
                .all(|member| member != object.id())
        );
    }
    let output_faces = outputs
        .iter()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) => &brep.faces()[0],
            _ => panic!("ExtractSrf output must be an exact one-face B-rep"),
        })
        .collect::<Vec<_>>();
    assert_eq!(output_faces[0].surface(), box_brep.faces()[2].surface());
    assert_eq!(output_faces[1].surface(), box_brep.faces()[0].surface());
    assert!(output_faces.iter().all(|face| {
        face.loops()
            .iter()
            .flat_map(|face_loop| face_loop.trims())
            .all(|trim| trim.trim_type() == viboceros_geometry::BrepTrimType::Boundary)
    }));
    assert_eq!(document.undo_label(), Some("ExtractSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Brep(box_brep.clone())
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractSurface FaceIndices=5,1 Copy=Yes OutputLayer=Input",
            )
            .unwrap(),
        "Extracted 2 surface(s) from 1 object(s); source faces copied"
    );
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Brep(box_brep.clone())
    );
    assert!(!document.is_selected(source));
    let copies = document
        .selected_objects()
        .map(|object| {
            assert_eq!(object.attributes().layer_id(), input_layer);
            assert!(
                document
                    .group(group)
                    .unwrap()
                    .members()
                    .all(|member| member != object.id())
            );
            match object.geometry() {
                Geometry::Brep(brep) => brep.faces()[0].surface(),
                _ => panic!("ExtractSrf copy must be an exact one-face B-rep"),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        copies,
        vec![box_brep.faces()[5].surface(), box_brep.faces()[1].surface()]
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtractSrf Faces=All Copy=No")
            .unwrap(),
        "Extracted 6 surface(s) from 1 object(s); source faces removed"
    );
    assert!(document.object(source).is_none());
    assert_eq!(document.group(group).unwrap().members().len(), 0);
    assert_eq!(document.selected_object_count(), 6);
    // Native ExtractSrf emits faces in descending source index order.
    for (object, source_face) in document
        .selected_objects()
        .zip(box_brep.faces().iter().rev())
    {
        let Geometry::Brep(face) = object.geometry() else {
            panic!("ExtractSrf output must remain an exact one-face B-rep")
        };
        assert_eq!(face.faces()[0].surface(), source_face.surface());
    }
}

#[test]
fn extract_surface_point_mode_handles_single_surfaces_and_failures_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 2.0).unwrap(),
        Point3::try_new(4.0, 0.0, 2.0).unwrap(),
        Point3::try_new(4.0, 3.0, 2.0).unwrap(),
        Point3::try_new(0.0, 3.0, 2.0).unwrap(),
    ])
    .unwrap();
    let source = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface.clone()),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("Sheet"),
        )
        .unwrap();
    let group = document
        .add_group(Some("Sheet group".to_owned()), [source])
        .unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "ExtractSrf 2,1,5").unwrap(),
        "Extracted 1 surface(s) from 1 object(s); source faces removed"
    );
    assert!(document.object(source).is_none());
    assert_eq!(document.group(group).unwrap().members().len(), 0);
    assert_eq!(document.selected_object_count(), 1);
    let output = document.selected_objects().next().unwrap();
    assert_eq!(output.attributes().name(), Some("Sheet"));
    assert_eq!(output.geometry(), &Geometry::NurbsSurface(surface));

    registry.execute(&mut document, "Undo").unwrap();
    let history = document.undo_label().map(str::to_owned);
    let object_count = document.objects().len();
    for invalid in [
        "ExtractSrf Faces=1",
        "ExtractSrf Faces=0,0",
        "ExtractSrf Faces=0 1,1,2",
        "ExtractSrf Faces=0 Copy=Maybe",
        "ExtractSrf Faces=0 OutputLayer=Other",
        "ExtractSrf Faces=",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    registry.execute(&mut document, "Point 10,10").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let history = document.undo_label().map(str::to_owned);
    let object_count = document.objects().len();
    assert!(matches!(
        registry.execute(&mut document, "ExtractSrf Faces=0"),
        Err(CommandError::UnsupportedExtractSurfaceGeometry)
    ));
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn point_cloud_extraction_selection_and_explode_match_rhino() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New First").unwrap();
    let first_layer = document.current_layer_id();
    registry.execute(&mut document, "Line 0,0 2,0").unwrap();
    registry.execute(&mut document, "SelLast").unwrap();
    registry
        .execute(&mut document, "SetObjectName Guide")
        .unwrap();
    registry.execute(&mut document, "Layer New Second").unwrap();
    let second_layer = document.current_layer_id();
    registry
        .execute(&mut document, "Polyline 10,0 12,0 10,2")
        .unwrap();
    let source_points = document
        .objects()
        .flat_map(|object| object.geometry().extract_point_locations().unwrap())
        .collect::<Vec<_>>();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let reversed_source_points = [source_ids[1], source_ids[0]]
        .into_iter()
        .flat_map(|id| {
            document
                .object(id)
                .unwrap()
                .geometry()
                .extract_point_locations()
                .unwrap()
        })
        .collect::<Vec<_>>();
    document
        .select_object(source_ids[1], SelectionMode::Replace)
        .unwrap();
    document
        .select_object(source_ids[0], SelectionMode::Add)
        .unwrap();
    registry
        .execute(&mut document, "ExtractPt Output=PointCloud")
        .unwrap();
    let reversed_cloud = document.selected_objects().next().unwrap();
    assert_eq!(reversed_cloud.attributes().layer_id(), second_layer);
    assert_eq!(reversed_cloud.attributes().name(), None);
    let Geometry::PointCloud(reversed_cloud) = reversed_cloud.geometry() else {
        panic!("expected a reverse-selection point cloud")
    };
    assert_eq!(reversed_cloud.points(), reversed_source_points);
    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "SelNone").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractPt Output=PointCloud")
            .unwrap(),
        "Extracted 5 point(s) from 2 of 2 selected object(s)"
    );
    let input_cloud_id = document.selected_object_ids().next().unwrap();
    let input_cloud = document.object(input_cloud_id).unwrap();
    assert_eq!(input_cloud.attributes().layer_id(), first_layer);
    assert_eq!(input_cloud.attributes().name(), Some("Guide"));
    let Geometry::PointCloud(cloud) = input_cloud.geometry() else {
        panic!("expected a point cloud")
    };
    assert_eq!(cloud.points(), source_points);
    assert_eq!(document.selected_object_count(), 1);

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry
        .execute(
            &mut document,
            "ExtractPt OutputLayer=Current Output=PointCloud",
        )
        .unwrap();
    let current_cloud_id = document.selected_object_ids().next().unwrap();
    let current_cloud = document.object(current_cloud_id).unwrap();
    assert_eq!(current_cloud.attributes().layer_id(), second_layer);
    assert_eq!(current_cloud.attributes().name(), None);
    let Geometry::PointCloud(current_cloud_geometry) = current_cloud.geometry() else {
        panic!("expected a current-layer point cloud")
    };
    let expected_exploded_points = current_cloud_geometry
        .points()
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();

    registry.execute(&mut document, "SelNone").unwrap();
    assert_eq!(
        registry.execute(&mut document, "SelPt").unwrap(),
        "Selected 0 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "SelPtCloud").unwrap(),
        "Selected 1 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "Explode").unwrap(),
        "Exploded 1 point cloud(s) into 5 point(s); 0 object(s) unchanged"
    );
    assert_eq!(document.selected_object_count(), 5);
    assert_eq!(document.objects().len(), 7);
    assert!(document.objects().skip(2).all(|object| {
        matches!(object.geometry(), Geometry::Point(_))
            && object.attributes().layer_id() == second_layer
            && object.attributes().name().is_none()
    }));
    assert_eq!(
        document
            .objects()
            .skip(2)
            .filter_map(|object| match object.geometry() {
                Geometry::Point(point) => Some(*point),
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected_exploded_points
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert!(matches!(
        document.object(current_cloud_id).unwrap().geometry(),
        Geometry::PointCloud(_)
    ));
}
