use super::*;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
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
