//! curves queries command regressions.
use super::*;

#[test]
fn divides_selected_curves_by_count_or_length_on_current_layer_and_preserves_source() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Construction")
        .unwrap();
    let source_layer = document.current_layer_id();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let source = document.objects().next().unwrap().id();
    let output_layer = document.current_layer_id();

    assert_eq!(
        registry.execute(&mut document, "Div 5").unwrap(),
        "Divided 1 curve(s), adding 4 point(s)"
    );
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.selected_object_count(), 4);
    let mut points = document
        .selected_objects()
        .map(|object| {
            assert_eq!(object.attributes().layer_id(), output_layer);
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected a division point")
            };
            *point
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    assert_eq!(
        document.object(source).unwrap().attributes().layer_id(),
        source_layer
    );
    assert_eq!(
        points,
        [2.0, 4.0, 6.0, 8.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
    );
    assert_eq!(document.undo_label(), Some("Divide"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);

    registry.execute(&mut document, "SelAll").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "Divide Length 3 MarkEnds")
            .unwrap(),
        "Divided 1 curve(s), adding 4 point(s)"
    );
    let mut points = document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a division point"),
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    assert_eq!(
        points,
        [0.0, 3.0, 6.0, 9.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
    );
}

#[test]
fn divides_closed_curves_once_at_the_seam_and_rolls_back_errors() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 2").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Divide 4").unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.selected_object_count(), 4);
    let points = document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a division point"),
        })
        .collect::<Vec<_>>();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("expected source circle")
    };
    for expected in circle.quadrants().unwrap() {
        assert!(
            points
                .iter()
                .any(|actual| actual.is_near(expected, document.tolerance()))
        );
    }

    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let object_count = document.objects().len();
    assert!(matches!(
        registry.execute(&mut document, "Divide 3"),
        Err(CommandError::UnsupportedDivideGeometry)
    ));
    assert_eq!(document.objects().len(), object_count);
    assert!(registry.execute(&mut document, "Divide 0").is_err());
    assert!(registry.execute(&mut document, "Divide Length -1").is_err());
    assert_eq!(document.objects().len(), object_count);
}

#[test]
fn marks_curve_starts_and_ends_with_attribute_preserving_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Markers")
        .unwrap();
    let source_layer = document.current_layer_id();
    registry.execute(&mut document, "Line 1,2,3 7,5,3").unwrap();
    registry.execute(&mut document, "Circle 10,2,3 4").unwrap();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let expected_starts = source_ids
        .iter()
        .map(|id| {
            geometry_curve_ref(document.object(*id).unwrap().geometry())
                .unwrap()
                .start_point()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let expected_ends = source_ids
        .iter()
        .map(|id| {
            geometry_curve_ref(document.object(*id).unwrap().geometry())
                .unwrap()
                .end_point()
                .unwrap()
        })
        .collect::<Vec<_>>();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();

    assert_eq!(
        registry.execute(&mut document, "CrvStart").unwrap(),
        "Added 2 point(s) at curve starts"
    );
    let start_points = document
        .selected_objects()
        .map(|object| {
            assert_eq!(object.attributes().layer_id(), source_layer);
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected a start marker")
            };
            *point
        })
        .collect::<Vec<_>>();
    assert_eq!(start_points.len(), 2);
    assert!(
        expected_starts
            .iter()
            .all(|expected| start_points.contains(expected))
    );
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.undo_label(), Some("CrvStart"));

    registry.execute(&mut document, "Undo").unwrap();
    for id in &source_ids {
        document.select_object(*id, SelectionMode::Add).unwrap();
    }
    assert_eq!(
        registry.execute(&mut document, "CrvEnd").unwrap(),
        "Added 2 point(s) at curve ends"
    );
    let end_points = document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected an end marker"),
        })
        .collect::<Vec<_>>();
    assert!(
        expected_ends
            .iter()
            .all(|expected| end_points.contains(expected))
    );
    assert_eq!(expected_starts[1], expected_ends[1]);

    registry.execute(&mut document, "Point 99,99").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let object_count = document.objects().len();
    assert!(matches!(
        registry.execute(&mut document, "CrvStart"),
        Err(CommandError::UnsupportedCurveEndpointGeometry)
    ));
    assert_eq!(document.objects().len(), object_count);
}

#[test]
fn extracts_defining_points_in_rhino_order_with_layers_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    registry.execute(&mut document, "Line 0,0,0 2,3,4").unwrap();
    registry.execute(&mut document, "Circle 10,0,0 2").unwrap();
    registry
        .execute(&mut document, "Polyline 0,0 2,0 2,2 0,0")
        .unwrap();
    let periodic = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    document
        .add_geometry(Geometry::NurbsCurve(periodic))
        .unwrap();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        Point3::try_new(2.0, 0.0, 2.0).unwrap(),
        Point3::try_new(2.0, 3.0, 4.0).unwrap(),
        Point3::try_new(0.0, 3.0, 3.0).unwrap(),
    ])
    .unwrap();
    document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(99.0, 99.0, 99.0).unwrap(),
            Point3::try_new(0.0, 0.0, 5.0).unwrap(),
            Point3::try_new(2.0, 0.0, 5.0).unwrap(),
            Point3::try_new(0.0, 2.0, 5.0).unwrap(),
        ],
        vec![[1, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    registry.execute(&mut document, "Point 8,8,8").unwrap();

    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let expected_points = document
        .objects()
        .map(|object| object.geometry().extract_point_locations().unwrap())
        .collect::<Vec<_>>()
        .concat();
    assert_eq!(expected_points.len(), 24);
    registry.execute(&mut document, "Layer New Output").unwrap();
    let output_layer = document.current_layer_id();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Sources").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtractPt OutputLayer=Input Output Points",)
            .unwrap(),
        "Extracted 24 point(s) from 6 of 7 selected object(s)"
    );
    assert_eq!(document.undo_label(), Some("ExtractPt"));
    assert_eq!(document.selected_object_count(), 24);
    let extracted = document
        .objects()
        .skip(source_ids.len())
        .map(|object| {
            assert_eq!(object.attributes().layer_id(), input_layer);
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected an extracted point")
            };
            *point
        })
        .collect::<Vec<_>>();
    assert_eq!(extracted, expected_points);
    assert_eq!(
        document
            .group_by_name("Sources")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        source_ids.iter().copied().collect()
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), source_ids.len());
    document
        .select_object(source_ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtractPt OutputLayer Current")
            .unwrap(),
        "Extracted 24 point(s) from 6 of 7 selected object(s)"
    );
    for object in document.objects().skip(source_ids.len()) {
        assert_eq!(object.attributes().layer_id(), output_layer);
    }

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source_ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractPt Output=PointCloud OutputLayer=Input",
            )
            .unwrap(),
        "Extracted 24 point(s) from 6 of 7 selected object(s)"
    );
    assert_eq!(document.selected_object_count(), 1);
    let cloud_object = document.selected_objects().next().unwrap();
    assert_eq!(cloud_object.attributes().layer_id(), input_layer);
    let Geometry::PointCloud(cloud) = cloud_object.geometry() else {
        panic!("expected an extracted point cloud")
    };
    assert_eq!(cloud.points(), expected_points);

    let mut point_only = Document::default();
    registry.execute(&mut point_only, "Point 1,2,3").unwrap();
    registry.execute(&mut point_only, "SelAll").unwrap();
    let history = point_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut point_only, "ExtractPt"),
        Err(CommandError::NoExtractablePoints)
    ));
    assert_eq!(point_only.undo_label(), history.as_deref());
    assert_eq!(point_only.objects().len(), 1);
    assert!(matches!(
        registry.execute(&mut point_only, "ExtractPt Output=PointCloud"),
        Err(CommandError::NoExtractablePoints)
    ));
}

#[test]
fn extracts_curve_and_surface_control_polygons_with_rhino_layers_and_seams() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Inputs").unwrap();
    let input_layer = document.current_layer_id();
    registry.execute(&mut document, "Line 0,0,0 2,3,4").unwrap();
    registry.execute(&mut document, "Circle 10,0,0 2").unwrap();
    let periodic_points = [
        Point3::try_new(20.0, 0.0, 0.0).unwrap(),
        Point3::try_new(22.0, 0.0, 0.0).unwrap(),
        Point3::try_new(21.0, 2.0, 0.0).unwrap(),
    ];
    let periodic = NurbsCurve::try_new(
        2,
        vec![
            periodic_points[0],
            periodic_points[1],
            periodic_points[2],
            periodic_points[0],
            periodic_points[1],
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    document
        .add_geometry(Geometry::NurbsCurve(periodic))
        .unwrap();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(30.0, 0.0, 0.0).unwrap(),
        Point3::try_new(33.0, 0.0, 0.0).unwrap(),
        Point3::try_new(33.0, 2.0, 1.0).unwrap(),
        Point3::try_new(30.0, 2.0, 0.0).unwrap(),
    ])
    .unwrap();
    document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();

    registry
        .execute(&mut document, "Layer New Results")
        .unwrap();
    let result_layer = document.current_layer_id();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtractControlPolygon OutputLayer=Input")
            .unwrap(),
        "Extracted 4 control polygon(s) from 4 object(s)"
    );
    assert!(source_ids.iter().all(|id| !document.is_selected(*id)));
    let outputs = document
        .objects()
        .skip(source_ids.len())
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    assert!(outputs.iter().all(|object| {
        object.attributes().layer_id() == input_layer && document.is_selected(object.id())
    }));
    let Geometry::Polyline(line_polygon) = outputs[0].geometry() else {
        panic!("line control polygon must remain a polyline object")
    };
    assert_eq!(line_polygon.vertices().len(), 2);
    let Geometry::Polyline(circle_polygon) = outputs[1].geometry() else {
        panic!("circle control polygon must be a polyline object")
    };
    assert!(circle_polygon.is_closed());
    assert_eq!(circle_polygon.vertices().len(), 9);
    let Geometry::Polyline(periodic_polygon) = outputs[2].geometry() else {
        panic!("periodic control polygon must be a polyline object")
    };
    assert_eq!(
        periodic_polygon.vertices(),
        &[
            periodic_points[1],
            periodic_points[2],
            periodic_points[0],
            periodic_points[1],
        ]
    );
    let Geometry::Mesh(surface_polygon) = outputs[3].geometry() else {
        panic!("surface control polygon must be a mesh object")
    };
    assert_eq!(
        surface_polygon.faces(),
        &[viboceros_geometry::MeshFace::Quad([0, 1, 3, 2])]
    );
    assert_eq!(surface_polygon.topology().edge_count(), 4);
    assert_eq!(document.undo_label(), Some("ExtractControlPolygon"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), source_ids.len());
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "ExtractControlPolygon")
            .unwrap(),
        "Extracted 4 control polygon(s) from 4 object(s)"
    );
    assert!(
        document
            .objects()
            .skip(source_ids.len())
            .all(|object| object.attributes().layer_id() == result_layer)
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "ExtractControlPolygon OutputLayer _TargetObject",
        )
        .unwrap();
    assert!(
        document
            .objects()
            .skip(source_ids.len())
            .all(|object| object.attributes().layer_id() == input_layer)
    );
}

#[test]
fn extract_control_polygon_rejects_bad_inputs_and_options_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 2,0").unwrap();
    registry.execute(&mut document, "Point 5,5").unwrap();
    let point = document.objects().last().unwrap().id();
    document
        .select_objects_direct([point], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    let object_count = document.objects().len();
    assert!(matches!(
        registry.execute(&mut document, "ExtractControlPolygon"),
        Err(CommandError::UnsupportedExtractControlPolygonGeometry)
    ));
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.undo_label(), history.as_deref());
    for invalid in [
        "ExtractControlPolygon OutputLayer",
        "ExtractControlPolygon OutputLayer=Other",
        "ExtractControlPolygon OutputLayer=Input OutputLayer=Current",
        "ExtractControlPolygon Other=Input",
        "ExtractControlPolygon stray",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
        assert_eq!(document.objects().len(), object_count);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    registry.execute(&mut document, "SelNone").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtractControlPolygon"),
        Err(CommandError::NoObjectsSelected)
    ));

    let points = [
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(12.0, 2.0, 0.0).unwrap(),
        Point3::try_new(14.0, 1.0, 0.0).unwrap(),
        Point3::try_new(15.0, 0.0, 0.0).unwrap(),
    ];
    let repeated = NurbsCurve::try_new(
        3,
        vec![points[0], points[0], points[1], points[2], points[3]],
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let repeated_id = document
        .add_geometry(Geometry::NurbsCurve(repeated))
        .unwrap();
    document
        .select_objects_direct([repeated_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    let object_count = document.objects().len();
    assert!(matches!(
        registry.execute(&mut document, "ExtractControlPolygon"),
        Err(CommandError::Geometry(
            GeometryError::DegeneratePolylineSegment { segment: 0 }
        ))
    ));
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.undo_label(), history.as_deref());
}
