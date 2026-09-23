use super::*;
use viboceros_geometry::{CurveSegment3, PolyCurve3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn offset_linear_polycurve_applies_corner_rule_and_through_point() {
    let mut document = Document::default();
    let first = LineSegment::try_new(
        point(0.0, 0.0, 0.0),
        point(4.0, 0.0, 0.0),
        document.tolerance(),
    )
    .unwrap();
    let second = LineSegment::try_new(
        point(4.0, 0.0, 0.0),
        point(4.0, 4.0, 0.0),
        document.tolerance(),
    )
    .unwrap();
    let source = PolyCurve3::try_with_segment_domains(
        vec![CurveSegment3::Line(first), CurveSegment3::Line(second)],
        vec![10.0, 12.0, 20.0],
    )
    .unwrap();
    let id = document.add_geometry(Geometry::PolyCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 0,2,0")
        .unwrap();
    let Geometry::Polyline(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("linear polycurve offset")
    };
    assert_eq!(
        out.vertices(),
        &[
            point(0.0, 1.0, 0.0),
            point(3.0, 1.0, 0.0),
            point(3.0, 4.0, 0.0)
        ]
    );
    assert_eq!(out.domain(), 10.0..=20.0);
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset ThroughPoint=0,1,0")
        .unwrap();
    let Geometry::Polyline(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("linear polycurve through-point offset")
    };
    assert_eq!(out.vertices()[0], point(0.0, 1.0, 0.0));
}

#[test]
fn offset_curved_polycurve_none_selects_both_convex_gap_pieces() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let line = LineSegment::try_new(
        point(0.0, 0.0, 0.0),
        point(2.0, 0.0, 0.0),
        document.tolerance(),
    )
    .unwrap();
    let circle = Circle3::try_from_center_point(
        point(1.0, 0.0, 0.0),
        point(2.0, 0.0, 0.0),
        normal,
        document.tolerance(),
    )
    .unwrap();
    let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
    let source =
        PolyCurve3::try_new(vec![CurveSegment3::Line(line), CurveSegment3::Arc(arc)]).unwrap();
    let id = document.add_geometry(Geometry::PolyCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 0.2 3,-1,0 Corner=None")
        .unwrap();
    let pieces = document
        .selected_objects()
        .map(|object| {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("NURBS offset part")
            };
            curve.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(pieces.len(), 2);
    assert!(
        pieces[0]
            .evaluate(*pieces[0].domain().end())
            .unwrap()
            .distance_to(point(2.0, -0.2, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
    assert!(
        pieces[1]
            .evaluate(*pieces[1].domain().start())
            .unwrap()
            .distance_to(point(2.2, 0.0, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset ThroughPoint=0,-0.2,0 Corner=None")
        .unwrap();
    assert_eq!(document.selected_object_count(), 2);
}

#[test]
fn offset_open_nurbs_curve_uses_pick_side_and_through_point() {
    let mut document = Document::default();
    let source = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(4.0, 2.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let id = document.add_geometry(Geometry::NurbsCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 0.35 0,1,0")
        .unwrap();
    let Geometry::NurbsCurve(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("NURBS offset")
    };
    assert!(
        out.evaluate(0.0)
            .unwrap()
            .distance_to(point(0.0, 0.35, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset ThroughPoint=0,0.5,0")
        .unwrap();
    let Geometry::NurbsCurve(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("NURBS through-point offset")
    };
    assert!(
        out.evaluate(0.0)
            .unwrap()
            .distance_to(point(0.0, 0.5, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
}

#[test]
fn offset_ellipse_creates_smooth_curve_and_through_point_reaches_pick() {
    let mut document = Document::default();
    let frame = CommandContext::default().construction_plane;
    let ellipse = Ellipse3::try_new(
        point(0.0, 0.0, 0.0),
        5.0,
        3.0,
        frame.x_axis(),
        frame.y_axis(),
        document.tolerance(),
    )
    .unwrap();
    let id = document.add_geometry(Geometry::Ellipse(ellipse)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 0.8 8,0,0")
        .unwrap();
    let Geometry::NurbsCurve(outside) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("smooth offset")
    };
    assert!(outside.is_closed().unwrap());
    assert!(
        outside
            .evaluate(*outside.domain().start())
            .unwrap()
            .distance_to(point(5.8, 0.0, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset ThroughPoint=5.8,0,0")
        .unwrap();
    let Geometry::NurbsCurve(through) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("smooth through offset")
    };
    assert!(
        through
            .evaluate(*through.domain().start())
            .unwrap()
            .distance_to(point(5.8, 0.0, 0.0))
            .unwrap()
            <= document.tolerance().absolute()
    );
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
fn chamfer_option_adds_straight_bridge_at_convex_corner() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 1,2,0 Corner=Chamfer")
        .unwrap();
    let Geometry::Polyline(output) = document.selected_objects().next().unwrap().geometry() else {
        panic!("offset polyline")
    };
    assert_eq!(
        output.vertices(),
        &[
            point(0.0, 1.0, 0.0),
            point(4.0, 1.0, 0.0),
            point(5.0, 0.0, 0.0),
            point(5.0, -4.0, 0.0),
        ]
    );
    assert!(matches!(
        parse(&["1", "1,2,0", "Corner=Smooth"]),
        Err(CommandError::Usage(_))
    ));
}

#[test]
fn round_option_creates_selected_tangent_polycurve() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 1,2,0 Corner=Round")
        .unwrap();
    let Geometry::PolyCurve(output) = document.selected_objects().next().unwrap().geometry() else {
        panic!("round polycurve")
    };
    assert_eq!(output.segments().len(), 3);
    assert!(matches!(
        output.segments()[1],
        viboceros_geometry::CurveSegment3::Arc(_)
    ));
    assert_eq!(document.objects().count(), 2);
}

#[test]
fn round_both_sides_keeps_inner_sharp_and_outer_rounded() {
    let mut document = Document::default();
    let source = document
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
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 BothSides=Yes Corner=Round")
        .unwrap();
    let results = document
        .selected_objects()
        .map(|object| object.geometry())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert!(matches!(results[0], Geometry::Polyline(_)));
    assert!(matches!(results[1], Geometry::PolyCurve(_)));
    assert_eq!(document.objects().count(), 3);
}

#[test]
fn none_option_selects_each_disconnected_offset_piece() {
    let mut document = Document::default();
    let source = document
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
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let message = CommandRegistry::with_builtins()
        .execute(&mut document, "Offset 1 BothSides=Yes Corner=None")
        .unwrap();
    assert!(message.contains("5 offset curve(s)"));
    let results = document
        .selected_objects()
        .map(|object| object.geometry())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 5);
    assert!(matches!(results[0], Geometry::Polyline(_)));
    assert!(
        results[1..]
            .iter()
            .all(|geometry| matches!(geometry, Geometry::Line(_)))
    );
    assert_eq!(document.objects().count(), 6);
}

#[test]
fn through_point_chooses_distance_for_each_selected_curve() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
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
    let circle = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0, 0.0), 5.0, normal, document.tolerance()).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([line, circle], SelectionMode::Replace)
        .unwrap();
    let message = CommandRegistry::with_builtins()
        .execute(&mut document, "Offset ThroughPoint=2,2,0")
        .unwrap();
    assert!(message.contains("through 2.000000,2.000000,0.000000"));
    let results = document
        .selected_objects()
        .map(|object| object.geometry())
        .collect::<Vec<_>>();
    let [Geometry::Line(offset_line), Geometry::Circle(offset_circle)] = results.as_slice() else {
        panic!("analytic offsets")
    };
    assert_eq!(offset_line.start(), point(0.0, 2.0, 0.0));
    assert!((offset_circle.radius() - 8.0_f64.sqrt()).abs() <= document.tolerance().absolute());
    assert_eq!(document.objects().count(), 4);
}

#[test]
fn through_point_failure_leaves_selected_sources_unchanged() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(-10.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let circle = Circle3::try_new(point(0.0, 0.0, 0.0), 5.0, normal, document.tolerance()).unwrap();
    let arc = document
        .add_geometry(Geometry::Arc(
            CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([line, arc], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut document, "Offset ThroughPoint=-7,2,0",),
        Err(CommandError::Geometry(
            GeometryError::OffsetThroughPointNoSolution
        ))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(matches!(
        parse(&["ThroughPoint=2,2,0", "BothSides=Yes"]),
        Err(CommandError::Usage(_))
    ));
}

#[test]
fn through_point_reaches_a_polyline_chamfer() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            "Offset ThroughPoint=4.5,0.5,0 Corner=Chamfer",
        )
        .unwrap();
    let Geometry::Polyline(output) = document.selected_objects().next().unwrap().geometry() else {
        panic!("chamfer")
    };
    assert_eq!(output.vertices()[1], point(4.0, 1.0, 0.0));
    assert_eq!(output.vertices()[2], point(5.0, 0.0, 0.0));
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
