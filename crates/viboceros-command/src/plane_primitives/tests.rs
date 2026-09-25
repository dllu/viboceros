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
fn three_point_circle_uses_pick_order_and_rejects_degenerate_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle _3Point 4,0,0 0,4,0 -4,0,0")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert!(
        circle
            .center()
            .is_near(point(0.0, 0.0, 0.0), Tolerance::DEFAULT)
    );
    assert!((circle.radius() - 4.0).abs() < 1e-12);
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 0.0, 1.0]
    );
    assert!(
        circle
            .point_at_angle(0.0)
            .unwrap()
            .is_near(point(4.0, 0.0, 0.0), Tolerance::DEFAULT)
    );
    let before = format!("{document:?}");
    for command in [
        "Circle 3Point 0,0,0 0,0,0 1,0,0",
        "Circle 3Point 0,0,0 1,0,0 2,0,0",
        "Circle 3Point 1,0,0 0,1,0",
        "Circle 3Point 1,0,0 0,1,0 -1,0,0 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn three_point_circle_radius_chooses_center_and_second_point_seam() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle 3Point 4,0,0 0,4,0 Radius=5 0,0,0")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    let center_component = 2.0 - 17.0_f64.sqrt() / 2.0_f64.sqrt();
    assert!(circle.center().is_near(
        point(center_component, center_component, 0.0),
        Tolerance::DEFAULT,
    ));
    assert!((circle.radius() - 5.0).abs() < 1e-12);
    assert!(
        circle
            .point_at_angle(0.0)
            .unwrap()
            .is_near(point(0.0, 4.0, 0.0), Tolerance::DEFAULT)
    );
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 0.0, 1.0]
    );

    registry
        .execute(&mut document, "Circle 3Point 4,0,0 0,4,0 Radius 5 2,2,5")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert!(
        circle
            .center()
            .is_near(point(2.0, 2.0, 17.0_f64.sqrt()), Tolerance::DEFAULT,)
    );
    assert!(
        circle
            .point_at_angle(0.0)
            .unwrap()
            .is_near(point(0.0, 4.0, 0.0), Tolerance::DEFAULT)
    );

    let before = format!("{document:?}");
    for command in [
        "Circle 3Point 4,0,0 0,4,0 Radius=2 0,0,0",
        "Circle 3Point 4,0,0 0,4,0 Radius=5 2,2,0",
        "Circle 3Point 4,0,0 0,4,0 Radius=0 0,0,0",
        "Circle 3Point 4,0,0 0,4,0 Radius=5 0,0,0 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn two_point_circle_uses_cplane_seam_and_rejects_normal_diameter() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle _2Point -4,0,0 4,0,0")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.center(), point(0.0, 0.0, 0.0));
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(circle.point_at_angle(0.0).unwrap(), point(4.0, 0.0, 0.0));
    let before = format!("{document:?}");
    for command in [
        "Circle 2Point 0,0,0 0,0,0",
        "Circle 2Point 0,0,1 0,0,5",
        "Circle 2Point 0,0,0",
        "Circle 2Point 0,0,0 4,0,0 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn circle_numeric_size_options_use_the_same_radius_and_keep_history_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let context = context();
    for command in [
        "Circle 1,2,3 3",
        "Circle 1,2,3 _Diameter=6",
        "Circle 1,2,3 _Circumference 18.84955592153876",
        "Circle 1,2,3 Area=28.274333882308138",
    ] {
        registry
            .execute_in_context(&mut document, command, context)
            .unwrap();
        let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
            panic!("circle")
        };
        assert!((circle.radius() - 3.0).abs() < 1e-12);
        assert_eq!(circle.center(), point(1.0, 2.0, 3.0));
        assert_eq!(circle.x_axis(), context.construction_plane.x_axis());
    }
    let before = format!("{document:?}");
    for command in [
        "Circle 0,0,0 Diameter=0",
        "Circle 0,0,0 Diameter=-6",
        "Circle 0,0,0 Circumference=nan",
        "Circle 0,0,0 Area=-1",
        "Circle 0,0,0 Area=28 Area=29",
    ] {
        assert!(
            registry
                .execute_in_context(&mut document, command, context)
                .is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn vertical_circle_uses_cplane_up_and_keeps_pick_direction() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle Vertical 1,2,3 5,2,3")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(circle.point_at_angle(0.0).unwrap(), point(5.0, 2.0, 3.0));

    registry
        .execute(&mut document, "Circle Vertical 1,2,3 Diameter 8 9,2,3")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 1.0, 0.0]
    );
    assert!(
        circle
            .point_at_angle(std::f64::consts::FRAC_PI_2)
            .unwrap()
            .is_near(point(1.0, 2.0, -1.0), Tolerance::DEFAULT)
    );

    registry
        .execute(&mut document, "Circle Vertical 1,2,3 4 9,2,3")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(circle.point_at_angle(0.0).unwrap(), point(5.0, 2.0, 3.0));

    let before = format!("{document:?}");
    for command in [
        "Circle Vertical 1,2,3 1,2,3",
        "Circle Vertical 1,2,3 1,2,7",
        "Circle Vertical 1,2,3 0 5,2,3",
        "Circle Vertical 1,2,3 5,2,3 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn oriented_circle_uses_normal_frame_and_projects_radius_picks() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Circle Orientation 1,2,3 1,3,3 4")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(circle.point_at_angle(0.0).unwrap(), point(1.0, 2.0, 7.0));
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 1.0, 0.0]
    );

    registry
        .execute(&mut document, "Circle Orientation 1,2,3 1,3,3 5,4,3")
        .unwrap();
    let Geometry::Circle(circle) = document.objects().last().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(circle.radius(), 4.0);
    assert_eq!(circle.point_at_angle(0.0).unwrap(), point(5.0, 2.0, 3.0));

    let before = format!("{document:?}");
    for command in [
        "Circle Orientation 1,2,3 1,2,3 4",
        "Circle Orientation 1,2,3 1,3,3 1,5,3",
        "Circle Orientation 1,2,3 1,3,3 Area=-1",
        "Circle Orientation 1,2,3 1,3,3 4 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
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
