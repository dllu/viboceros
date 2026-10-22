//! primitives arcs command regressions.
use super::*;

#[test]
fn creates_exact_circles_and_oriented_three_point_arcs_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 1,2,3 5").unwrap();
    registry
        .execute(
            &mut document,
            "Arc StartPoint 1,0,0 ThroughPoint 0,-1,0 -1,0,0",
        )
        .unwrap();

    let mut objects = document.objects();
    let Geometry::Circle(circle) = objects.next().unwrap().geometry() else {
        panic!("expected a circle")
    };
    assert_eq!(circle.center(), Point3::try_new(1.0, 2.0, 3.0).unwrap());
    assert_eq!(circle.radius(), 5.0);
    let Geometry::Arc(arc) = objects.next().unwrap().geometry() else {
        panic!("expected an arc")
    };
    assert!(
        document
            .tolerance()
            .approx_eq(arc.sweep_radians(), std::f64::consts::PI)
    );
    assert!(arc.point_at(0.5).unwrap().is_near(
        Point3::try_new(0.0, -1.0, 0.0).unwrap(),
        document.tolerance()
    ));
    drop(objects);
    assert_eq!(document.undo_label(), Some("Arc"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);

    assert!(
        registry
            .execute(&mut document, "Arc StartPoint 0,0 ThroughPoint 1,0 2,0")
            .is_err()
    );
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Circle"));
}

#[test]
fn arc_center_angle_uses_construction_plane_and_rejects_invalid_sweeps() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let plane = Frame3::try_from_directions(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    registry
        .execute_in_context(
            &mut document,
            "Arc Center 1,2,3 1,2,7 135",
            CommandContext {
                construction_plane: plane,
            },
        )
        .unwrap();
    let Geometry::Arc(arc) = document.objects().next().unwrap().geometry() else {
        panic!("expected an arc")
    };
    assert!((arc.sweep_radians().to_degrees() - 135.0).abs() < 1e-12);
    assert!(arc.point_at(0.0).unwrap().is_near(
        Point3::try_new(1.0, 2.0, 7.0).unwrap(),
        document.tolerance()
    ));
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 -90")
        .unwrap();
    let Geometry::Arc(reverse) = document.objects().nth(1).unwrap().geometry() else {
        panic!("expected reverse arc")
    };
    assert!(reverse.point_at(1.0).unwrap().y() < 2.0);
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 360")
        .unwrap();
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 450")
        .unwrap();
    let Geometry::Arc(full) = document.objects().nth(2).unwrap().geometry() else {
        panic!("expected full arc")
    };
    assert!((full.sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
    let Geometry::Arc(overfull) = document.objects().nth(3).unwrap().geometry() else {
        panic!("expected capped arc")
    };
    assert!((overfull.sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
    for input in [
        "Arc Center 1,2,3 1,2,3 90",
        "Arc Center 1,2,3 1,2,7 0",
        "Arc Center 1,2,3 1,2,7 nan",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 4);
    }
}

#[test]
fn arc_center_length_uses_signed_arc_length_and_rolls_back_invalid_sizes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(
            &mut document,
            "Arc Center 1,2,3 5,2,3 Length=6.283185307179586",
        )
        .unwrap();
    registry
        .execute(
            &mut document,
            "Arc Center 1,2,3 5,2,3 Length -6.283185307179586",
        )
        .unwrap();
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 Length=30")
        .unwrap();
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 Length=-30")
        .unwrap();
    let arcs = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected an arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 4);
    assert!((*arcs[0].domain().end() - std::f64::consts::TAU).abs() < 1e-12);
    assert!(arcs[0].point_at(1.0).unwrap().y() > 2.0);
    assert!(arcs[1].point_at(1.0).unwrap().y() < 2.0);
    assert!((arcs[2].sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
    assert!((arcs[3].sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
    assert!(arcs[3].point_at(0.25).unwrap().y() < 2.0);
    for input in [
        "Arc Center 1,2,3 5,2,3 Length=0",
        "Arc Center 1,2,3 5,2,3 Length=nan",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 4);
    }
}

#[test]
fn arc_center_endpoint_constrains_radius_and_supports_both_sweep_directions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Arc Center 1,2,3 5,2,3 End=1,6,3")
        .unwrap();
    registry
        .execute(
            &mut document,
            "Arc Center 1,2,3 5,2,3 End=1,10,3 Direction=Counterclockwise",
        )
        .unwrap();
    let arcs = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 2);
    assert!((arcs[0].sweep_radians().to_degrees() - 270.0).abs() < 1e-12);
    assert!((arcs[1].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    assert!(arcs[0].point_at(0.25).unwrap().y() < 2.0);
    assert!(arcs[1].point_at(0.25).unwrap().y() > 2.0);
    for arc in arcs {
        assert!(arc.end().unwrap().is_near(
            Point3::try_new(1.0, 6.0, 3.0).unwrap(),
            document.tolerance()
        ));
    }
    for input in [
        "Arc Center 1,2,3 5,2,3 End=1,2,3",
        "Arc Center 1,2,3 5,2,3 End=1,6,3 Direction=Sideways",
        "Arc Center 1,2,3 5,2,3 90 Direction=Clockwise",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 2);
    }
}

#[test]
fn arc_start_point_direction_uses_tangent_and_endpoint_in_three_dimensions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in [
        "Arc StartPoint 0,0,0 Direction=1,0,0 2,2,0",
        "Arc StartPoint 0,0,0 Direction 1,0,0 -2,2,0",
        "Arc StartPoint 0,0,0 Direction=1,0,0 2,2,3",
        "Arc StartPoint 0,0,0 2,2,0 1,0,0",
    ] {
        registry.execute(&mut document, input).unwrap();
    }
    let arcs = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 4);
    assert!((arcs[0].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    assert!((arcs[1].sweep_radians().to_degrees() - 270.0).abs() < 1e-12);
    assert!(
        arcs[2]
            .end()
            .unwrap()
            .is_near(Point3::try_new(2.0, 2.0, 3.0).unwrap(), Tolerance::DEFAULT)
    );
    for input in [
        "Arc StartPoint 0,0,0 Direction=0,0,0 2,2,0",
        "Arc StartPoint 0,0,0 Direction=1,0,0 2,0,0",
        "Arc StartPoint 0,0,0 Direction=1,0,0 0,0,0",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 4);
    }
}

#[test]
fn arc_start_point_center_orders_inputs_and_supports_signed_sweep_and_endpoint() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in [
        "Arc StartPoint 5,2,3 Center=1,2,3 90",
        "Arc StartPoint 5,2,3 Center 1,2,3 Length=-6.283185307179586",
        "Arc StartPoint 5,2,3 Center=1,2,3 End=1,10,3 Direction=Counterclockwise",
    ] {
        registry.execute(&mut document, input).unwrap();
    }
    let arcs = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 3);
    assert!((arcs[0].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    assert!(arcs[1].point_at(0.25).unwrap().y() < 2.0);
    assert!((arcs[2].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    for input in [
        "Arc StartPoint 5,2,3 Center=5,2,3 90",
        "Arc StartPoint 5,2,3 Center=1,2,3 Length=0",
        "Arc StartPoint 5,2,3 Center=1,2,3 End=1,2,3",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 3);
    }
}

#[test]
fn arc_center_midpoint_handles_sweep_boundaries_and_endpoint_directions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for input in [
        "Arc Center 1,2,3 Midpoint 1,6,3 90",
        "Arc Center 1,2,3 Midpoint 1,6,3 Length=-6.283185307179586",
        "Arc Center 1,2,3 Midpoint 1,6,3 360",
        "Arc Center 1,2,3 Midpoint 1,6,3 End=5,2,3 Direction=Counterclockwise",
        "Arc Center 1,2,3 Midpoint 1,6,3 End=9,2,3 Direction=Clockwise",
    ] {
        registry.execute(&mut document, input).unwrap();
    }
    let arcs = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 5);
    let midpoint = Point3::try_new(1.0, 6.0, 3.0).unwrap();
    assert!(
        arcs[0]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!(
        arcs[1]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!((arcs[2].sweep_radians() - std::f64::consts::PI).abs() < 1e-12);
    assert!(
        arcs[2]
            .start()
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!((arcs[3].sweep_radians() - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert!(
        arcs[3]
            .start()
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!(
        arcs[4]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    for input in [
        "Arc Center 1,2,3 Midpoint 1,2,3 90",
        "Arc Center 1,2,3 Midpoint 1,6,3 End=1,10,3",
        "Arc Center 1,2,3 Midpoint 1,6,3 0",
        "Arc Center 1,2,3 Midpoint 1,6,3 Length=0",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(document.objects().len(), 5);
    }
}
