use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn context() -> CommandContext {
    CommandContext {
        construction_plane: Frame3::try_from_directions(
            point(10.0, 20.0, 30.0),
            Vector3::try_new(0.6, 0.8, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    }
}

#[test]
fn plane_primitives_use_context_axes_without_reinterpreting_world_points() {
    let registry = CommandRegistry::with_builtins();
    let context = context();
    let center = point(2.0, 3.0, 4.0);
    let mut document = Document::default();
    registry
        .execute_in_context(&mut document, "Circle 2,3,4 5", context)
        .unwrap();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.center(), center);
    assert_eq!(circle.x_axis(), context.construction_plane.x_axis());
    assert_eq!(
        circle.normal().unwrap(),
        context.construction_plane.z_axis()
    );
    assert_eq!(circle.radius(), 5.0);
    assert_eq!(document.undo_label(), Some("Circle"));
    registry
        .execute_in_context(&mut document, "Undo", context)
        .unwrap();
    assert_eq!(document.objects().len(), 0);
    registry
        .execute_in_context(&mut document, "Redo", context)
        .unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn rectangle_normalizes_corner_order_on_a_translated_oblique_plane() {
    let registry = CommandRegistry::with_builtins();
    let context = context();
    let frame = context
        .construction_plane
        .with_origin(point(1e12, -2e12, 3e12));
    let opposite = frame.point_at([-4.0, 6.0, 0.0]).unwrap();
    let mut document = Document::default();
    registry
        .execute_in_context(
            &mut document,
            &format!(
                "Rectangle {} {}",
                format_point(frame.origin()),
                format_point(opposite)
            ),
            context,
        )
        .unwrap();
    let Geometry::Polyline(rectangle) = document.objects().next().unwrap().geometry() else {
        panic!("rectangle")
    };
    for (actual, local) in rectangle.vertices().iter().zip([
        [-4.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 6.0, 0.0],
        [-4.0, 6.0, 0.0],
        [-4.0, 0.0, 0.0],
    ]) {
        assert!(actual.distance_to(frame.point_at(local).unwrap()).unwrap() < 0.001);
    }
}

#[test]
fn failed_contextual_primitive_preserves_objects_selection_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let id = document.objects().next().unwrap().id();
    for command in [
        "Circle 0,0,0 -1",
        "Rectangle 0,0,0 0,0,0",
        "Box 0,0,0 1,2,0 0",
        "MeshPlane 0,0,0 2,3,0 XCount=0",
        "Polygon 2 0,0,0 1",
    ] {
        assert!(
            registry
                .execute_in_context(&mut document, command, context())
                .is_err()
        );
        assert_eq!(document.objects().len(), 1);
        assert!(document.is_selected(id));
        assert_eq!(document.undo_label(), Some("Point"));
    }
}

#[test]
fn picked_circle_and_polygon_tilt_and_box_corners_project_like_the_prompt() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle 2,-1,3 5,3,7")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert!((circle.radius() - 41.0_f64.sqrt()).abs() < 1e-12);
    assert!(
        circle
            .point_at_angle(0.0)
            .unwrap()
            .distance_to(point(5.0, 3.0, 7.0))
            .unwrap()
            < 1e-12
    );
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Circle 0,0,0 0,0,5")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.x_axis().as_vector().to_array(), [0.0, 0.0, 1.0]);
    assert_eq!(circle.y_axis().as_vector().to_array(), [0.0, 1.0, 0.0]);
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Box 0,0,1 2,3,9 4")
        .unwrap();
    assert_eq!(
        document.objects().next().unwrap().geometry().bounds().max(),
        point(2.0, 3.0, 5.0)
    );
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Polygon 5 0,0,0 5")
        .unwrap();
    let Geometry::Polyline(polygon) = document.objects().next().unwrap().geometry() else {
        panic!("polygon")
    };
    assert!((*polygon.domain().end() - polygon.length().unwrap()).abs() < 1e-12);
}
