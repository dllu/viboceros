use super::*;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn offset_line_uses_pick_side_and_selects_new_curve() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 2 0,-1,0")
        .unwrap();
    assert_eq!(document.objects().count(), 2);
    assert!(document.object(source).is_some());
    let output = document.selected_objects().next().unwrap();
    let Geometry::Line(line) = output.geometry() else {
        panic!("offset line")
    };
    assert_eq!(line.start(), point(0.0, -2.0, 0.0));
    assert_eq!(line.end(), point(4.0, -2.0, 0.0));
}

#[test]
fn both_sides_keeps_source_and_adds_two_analytic_circles() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let circle = Circle3::try_new(point(0.0, 0.0, 0.0), 5.0, normal, document.tolerance()).unwrap();
    let source = document.add_geometry(Geometry::Circle(circle)).unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 2 BothSides=Yes")
        .unwrap();
    let radii = document
        .selected_objects()
        .map(|object| {
            let Geometry::Circle(circle) = object.geometry() else {
                panic!("offset circle")
            };
            circle.radius()
        })
        .collect::<Vec<_>>();
    assert_eq!(radii, vec![3.0, 7.0]);
    assert_eq!(document.objects().count(), 3);
}

#[test]
fn unsupported_geometry_does_not_add_partial_outputs() {
    let mut document = Document::default();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let unsupported = document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
        .unwrap();
    document
        .select_objects_direct([line, unsupported], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let result = CommandRegistry::with_builtins().execute(&mut document, "Offset 1 0,2,0");
    assert!(matches!(
        result,
        Err(CommandError::UnsupportedOffsetGeometry)
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn offset_open_polyline_keeps_sharp_corner_and_vertex_parameters() {
    let mut document = Document::default();
    let curve = viboceros_geometry::Polyline3::try_with_parameters(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
        ],
        vec![2.0, 5.0, 9.0],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Polyline(curve)).unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 1,2,0")
        .unwrap();
    let Geometry::Polyline(output) = document.selected_objects().next().unwrap().geometry() else {
        panic!("offset polyline")
    };
    assert_eq!(
        output.vertices(),
        &[
            point(0.0, 1.0, 0.0),
            point(3.0, 1.0, 0.0),
            point(3.0, 4.0, 0.0)
        ]
    );
    assert_eq!(output.parameters(), &[2.0, 5.0, 9.0]);
}

#[test]
fn collapsing_polyline_offset_rolls_back_other_selected_results() {
    let mut document = Document::default();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, -5.0, 0.0),
                point(4.0, -5.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let boundary = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([line, boundary], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut document, "Offset 2 BothSides=Yes")
            .is_err()
    );
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn invalid_and_ambiguous_arguments_are_rejected() {
    assert!(matches!(
        parse(&["0", "0,1,0"]),
        Err(CommandError::Geometry(
            GeometryError::InvalidCurveOffsetDistance
        ))
    ));
    assert!(matches!(parse(&["1"]), Err(CommandError::Usage(_))));
    assert!(matches!(
        parse(&["1", "1,2,3", "BothSides=Yes"]),
        Err(CommandError::Usage(_))
    ));
    let mut document = Document::default();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(line, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut document, "Offset 1 2,0,0"),
        Err(CommandError::Geometry(
            GeometryError::AmbiguousCurveOffsetSide
        ))
    ));
}

#[test]
fn line_offset_uses_active_construction_plane() {
    let mut document = Document::default();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(line, SelectionMode::Replace)
        .unwrap();
    let plane = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        viboceros_geometry::Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        viboceros_geometry::Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut document,
            "Offset 2 0,0,-3",
            CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    let Geometry::Line(offset) = document.selected_objects().next().unwrap().geometry() else {
        panic!("offset line")
    };
    assert_eq!(offset.start(), point(0.0, 0.0, -2.0));
    assert_eq!(offset.end(), point(4.0, 0.0, -2.0));
}

#[test]
fn input_layer_option_uses_source_layer_without_copying_its_name() {
    let mut document = Document::default();
    let source_layer = document
        .add_layer("Source", ColorRgb::new(20, 30, 40))
        .unwrap();
    let source = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(3.0, 0.0, 0.0),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(source_layer).with_name("Original"),
        )
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 BothSides=Yes OutputLayer=Input")
        .unwrap();
    for object in document.selected_objects() {
        assert_eq!(object.attributes().layer_id(), source_layer);
        assert_eq!(object.attributes().name(), None);
    }
}
