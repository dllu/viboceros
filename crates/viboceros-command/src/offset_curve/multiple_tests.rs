use super::*;
use viboceros_geometry::{CurveSegment3, PolyCurve3};

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

#[test]
fn closed_linear_polycurve_offsets_as_region() {
    let mut document = Document::default();
    let vertices = [
        point(0.0, 0.0),
        point(4.0, 0.0),
        point(4.0, 4.0),
        point(0.0, 4.0),
        point(0.0, 0.0),
    ];
    let segments = vertices
        .windows(2)
        .map(|edge| {
            CurveSegment3::Line(
                LineSegment::try_new(edge[0], edge[1], document.tolerance()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let source = PolyCurve3::try_new(segments).unwrap();
    let id = document.add_geometry(Geometry::PolyCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 2,2,0 OffsetCount=1")
        .unwrap();
    let Geometry::Polyline(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("closed polycurve offset")
    };
    assert!(out.is_closed());
    assert_eq!(out.vertices()[0], point(0.5, 0.5));
}

#[test]
fn closed_smooth_polycurve_offsets_as_region() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let circle = Circle3::try_new(point(0.0, 0.0), 5.0, normal, document.tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let midpoint = circle.domain().start().midpoint(*circle.domain().end());
    let (first, second) = circle.try_split(midpoint).unwrap();
    let source = PolyCurve3::try_new(vec![
        CurveSegment3::NurbsCurve(first),
        CurveSegment3::NurbsCurve(second),
    ])
    .unwrap();
    let id = document.add_geometry(Geometry::PolyCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 0,0,0 OffsetCount=1")
        .unwrap();
    let Geometry::NurbsCurve(out) = document.selected_objects().next().unwrap().geometry() else {
        panic!("smooth polycurve region offset")
    };
    assert!(out.is_closed().unwrap());
    for index in 0..=64 {
        let t = *out.domain().start()
            + (*out.domain().end() - *out.domain().start()) * index as Real / 64.0;
        assert!(
            (out.evaluate(t)
                .unwrap()
                .distance_to(point(0.0, 0.0))
                .unwrap()
                - 4.5)
                .abs()
                <= document.tolerance().absolute()
        );
    }
}

#[test]
fn closed_linear_nurbs_offsets_inward_and_outward() {
    let mut document = Document::default();
    let polygon = Polyline3::try_new(
        vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 4.0),
            point(0.0, 4.0),
            point(0.0, 0.0),
        ],
        document.tolerance(),
    )
    .unwrap();
    let id = document
        .add_geometry(Geometry::NurbsCurve(polygon.to_native_nurbs().unwrap()))
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 2,2,0 OffsetCount=1")
        .unwrap();
    let Geometry::Polyline(inner) = document.selected_objects().next().unwrap().geometry() else {
        panic!("inward linear NURBS offset")
    };
    assert_eq!(inner.vertices()[0], point(0.5, 0.5));
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 5,5,0 OffsetCount=1")
        .unwrap();
    let Geometry::Polyline(outer) = document.selected_objects().next().unwrap().geometry() else {
        panic!("outward linear NURBS offset")
    };
    assert_eq!(outer.vertices()[0], point(-0.5, -0.5));
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            "OffsetMultiple 0.5 5,5,0 Corner=None OffsetCount=1",
        )
        .unwrap();
    assert_eq!(document.selected_object_count(), 4);
}

#[test]
fn open_nurbs_multiple_offsets_follow_pick_side() {
    let mut document = Document::default();
    let source = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(2.0, 0.0), point(4.0, 2.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let id = document.add_geometry(Geometry::NurbsCurve(source)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.2 0,1,0 OffsetCount=2")
        .unwrap();
    let starts = document
        .selected_objects()
        .map(|object| {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("NURBS offset")
            };
            curve.evaluate(0.0).unwrap().y()
        })
        .collect::<Vec<_>>();
    assert!((starts[0] - 0.2).abs() <= document.tolerance().absolute());
    assert!((starts[1] - 0.4).abs() <= document.tolerance().absolute());
}

#[test]
fn closed_rational_nurbs_regions_offset_with_nested_island() {
    let normal = CommandContext::default().construction_plane.z_axis();
    let tolerance = Document::default().tolerance();
    let outer = Circle3::try_new(point(0.0, 0.0), 5.0, normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let inner = Circle3::try_new(point(0.0, 0.0), 2.0, normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    for reverse in [false, true] {
        let mut document = Document::default();
        let outer = if reverse {
            outer.reversed().unwrap()
        } else {
            outer.clone()
        };
        let inner = if reverse {
            inner.reversed().unwrap()
        } else {
            inner.clone()
        };
        let outer_id = document.add_geometry(Geometry::NurbsCurve(outer)).unwrap();
        let inner_id = document.add_geometry(Geometry::NurbsCurve(inner)).unwrap();
        document
            .select_objects_direct([outer_id, inner_id], SelectionMode::Replace)
            .unwrap();
        CommandRegistry::with_builtins()
            .execute(&mut document, "OffsetMultiple 0.5 4,0,0 OffsetCount=1")
            .unwrap();
        let radii = document
            .selected_objects()
            .map(|object| {
                let Geometry::NurbsCurve(curve) = object.geometry() else {
                    panic!("NURBS region offset")
                };
                curve
                    .evaluate(*curve.domain().start())
                    .unwrap()
                    .distance_to(point(0.0, 0.0))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!((radii[0] - 4.5).abs() <= tolerance.absolute());
        assert!((radii[1] - 2.5).abs() <= tolerance.absolute());
    }
}

#[test]
fn intersecting_closed_nurbs_regions_leave_document_unchanged() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let first = Circle3::try_new(point(0.0, 0.0), 3.0, normal, document.tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let second = Circle3::try_new(point(4.0, 0.0), 3.0, normal, document.tolerance())
        .unwrap()
        .to_nurbs()
        .unwrap();
    let first_id = document.add_geometry(Geometry::NurbsCurve(first)).unwrap();
    let second_id = document.add_geometry(Geometry::NurbsCurve(second)).unwrap();
    document
        .select_objects_direct([first_id, second_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let result = CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.25 0,0,0 OffsetCount=1");
    assert!(matches!(
        result,
        Err(CommandError::Geometry(
            GeometryError::IntersectingOffsetRegions
        ))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn ellipse_multiple_offsets_preserve_smooth_closed_outputs() {
    let mut document = Document::default();
    let frame = CommandContext::default().construction_plane;
    let ellipse = Ellipse3::try_new(
        point(0.0, 0.0),
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
        .execute(&mut document, "OffsetMultiple 0.5 8,0,0 OffsetCount=2")
        .unwrap();
    let starts = document
        .selected_objects()
        .map(|object| {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("smooth offset")
            };
            assert!(curve.is_closed().unwrap());
            curve.evaluate(*curve.domain().start()).unwrap().x()
        })
        .collect::<Vec<_>>();
    assert!((starts[0] - 5.5).abs() <= document.tolerance().absolute());
    assert!((starts[1] - 6.0).abs() <= document.tolerance().absolute());
}

#[test]
fn ellipse_container_reverses_nested_circle_island() {
    let mut document = Document::default();
    let frame = CommandContext::default().construction_plane;
    let outer = document
        .add_geometry(Geometry::Ellipse(
            Ellipse3::try_new(
                point(0.0, 0.0),
                10.0,
                6.0,
                frame.x_axis(),
                frame.y_axis(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let inner = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0), 2.0, frame.z_axis(), document.tolerance()).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([outer, inner], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 4,0,0 OffsetCount=1")
        .unwrap();
    let mut outputs = document.selected_objects();
    let Geometry::NurbsCurve(container) = outputs.next().unwrap().geometry() else {
        panic!("ellipse offset")
    };
    assert!(
        (container.evaluate(*container.domain().start()).unwrap().x() - 9.5).abs()
            <= document.tolerance().absolute()
    );
    let Geometry::Circle(island) = outputs.next().unwrap().geometry() else {
        panic!("circle island")
    };
    assert_eq!(island.radius(), 2.5);
}

#[test]
fn open_curves_offset_toward_pick_with_requested_count() {
    let mut document = Document::default();
    let first = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(0.0, 0.0), point(4.0, 0.0), document.tolerance()).unwrap(),
        ))
        .unwrap();
    let second = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(0.0, 4.0), point(4.0, 4.0), document.tolerance()).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.5 0,2,0 OffsetCount=2")
        .unwrap();
    let ys = document
        .selected_objects()
        .map(|object| {
            let Geometry::Line(line) = object.geometry() else {
                panic!("line")
            };
            line.start().y()
        })
        .collect::<Vec<_>>();
    assert_eq!(ys, vec![0.5, 1.0, 3.5, 3.0]);
    assert_eq!(document.objects().count(), 6);
}

#[test]
fn nested_circles_reverse_island_direction() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let outer = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0), 10.0, normal, document.tolerance()).unwrap(),
        ))
        .unwrap();
    let island = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0), 3.0, normal, document.tolerance()).unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([outer, island], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 1 6,0,0 OffsetCount=2")
        .unwrap();
    let radii = document
        .selected_objects()
        .map(|object| {
            let Geometry::Circle(circle) = object.geometry() else {
                panic!("circle")
            };
            circle.radius()
        })
        .collect::<Vec<_>>();
    assert_eq!(radii, vec![9.0, 8.0, 4.0, 5.0]);
}

#[test]
fn nested_closed_polylines_respect_winding_and_island_depth() {
    let mut document = Document::default();
    let outer = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(-10.0, -10.0),
                    point(10.0, -10.0),
                    point(10.0, 10.0),
                    point(-10.0, 10.0),
                    point(-10.0, -10.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    // Clockwise inner boundary: its inward signed offset has the opposite sign.
    let island = document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(-3.0, -3.0),
                    point(-3.0, 3.0),
                    point(3.0, 3.0),
                    point(3.0, -3.0),
                    point(-3.0, -3.0),
                ],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([outer, island], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 1 6,0,0 OffsetCount=1")
        .unwrap();
    let outputs = document
        .selected_objects()
        .map(|object| {
            let Geometry::Polyline(curve) = object.geometry() else {
                panic!("polyline")
            };
            curve.vertices().to_vec()
        })
        .collect::<Vec<_>>();
    assert_eq!(outputs[0][0], point(-9.0, -9.0));
    assert_eq!(outputs[1][0], point(-4.0, -4.0));
}

#[test]
fn intersecting_closed_sources_do_not_change_document() {
    for separation in [4.0, 8.0] {
        let mut document = Document::default();
        let normal = CommandContext::default().construction_plane.z_axis();
        let a = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(point(0.0, 0.0), 4.0, normal, document.tolerance()).unwrap(),
            ))
            .unwrap();
        let b = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(point(separation, 0.0), 4.0, normal, document.tolerance())
                    .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut document, "OffsetMultiple 1 1,1,0")
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn collapse_on_later_copy_is_atomic() {
    let mut document = Document::default();
    let normal = CommandContext::default().construction_plane.z_axis();
    let id = document
        .add_geometry(Geometry::Circle(
            Circle3::try_new(point(0.0, 0.0), 1.5, normal, document.tolerance()).unwrap(),
        ))
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut document, "OffsetMultiple 1 0,0,0 OffsetCount=2")
            .is_err()
    );
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.selected_object_count(), 1);
}

#[test]
fn self_crossing_closed_polyline_cannot_supply_an_island_region() {
    let mut document = Document::default();
    let polyline = viboceros_geometry::Polyline3::try_new(
        vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 4.0),
            point(0.0, 4.0),
            point(2.0, -1.0),
            point(0.0, 0.0),
        ],
        document.tolerance(),
    )
    .unwrap();
    let id = document.add_geometry(Geometry::Polyline(polyline)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let result = CommandRegistry::with_builtins()
        .execute(&mut document, "OffsetMultiple 0.25 2,2,0 OffsetCount=2");
    assert!(matches!(
        result,
        Err(CommandError::Geometry(
            GeometryError::SelfIntersectingOffsetRegion
        ))
    ));
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.selected_object_count(), 1);
}
