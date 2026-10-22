//! primitives spirals command regressions.
use super::*;

#[test]
fn helix_supports_turn_pitch_radius_point_and_reverse_twist_forms() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let result = registry
        .execute(&mut document, "Helix 0,0,0 0,0,10 2 Turns=3")
        .unwrap();
    assert!(result.contains("Added NURBS helix"));
    assert!(result.contains("turns 3.000000"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("Helix must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(!curve.is_rational());
    assert_eq!(curve.control_points().len(), 363);
    assert_eq!(curve.domain(), 0.0..=3.0);
    assert!(
        curve
            .evaluate(1.0 / 24.0)
            .unwrap()
            .distance_to(
                Point3::try_new(
                    2.0 * (std::f64::consts::PI / 12.0).cos(),
                    2.0 * (std::f64::consts::PI / 12.0).sin(),
                    10.0 / 72.0,
                )
                .unwrap()
            )
            .unwrap()
            < 2.0e-12
    );
    assert_eq!(document.undo_label(), Some("Helix"));

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(
            &mut document,
            "Helix 1,2,3 1,12,3 4,2,3 Mode=Pitch Pitch=4 ReverseTwist=Yes",
        )
        .unwrap();
    assert!(result.contains("turns 2.500000, reverse twist"));
    let Geometry::NurbsCurve(reverse) = document.objects().next().unwrap().geometry() else {
        panic!("the pitch form must create a NURBS curve")
    };
    assert_eq!(reverse.control_points().len(), 303);
    assert!(
        reverse
            .evaluate(1.0 / 120.0)
            .unwrap()
            .distance_to(
                Point3::try_new(
                    1.0 + 3.0 * (std::f64::consts::PI / 60.0).cos(),
                    2.0 + 10.0 / 300.0,
                    3.0 + 3.0 * (std::f64::consts::PI / 60.0).sin(),
                )
                .unwrap()
            )
            .unwrap()
            < 2.0e-12
    );
    assert!(
        reverse
            .evaluate(2.5)
            .unwrap()
            .distance_to(Point3::try_new(-2.0, 12.0, 3.0).unwrap())
            .unwrap()
            < 2.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Helix 0 0 0 0 0 3 1 Pitch=1")
        .unwrap();
    let Geometry::NurbsCurve(inferred_pitch) = document.objects().next().unwrap().geometry() else {
        panic!("Pitch without an explicit mode must select pitch mode")
    };
    assert_eq!(inferred_pitch.control_points().len(), 363);
    registry.execute(&mut document, "Undo").unwrap();

    for invalid in [
        "Helix",
        "Helix 0,0,0 0,0,0 2 Turns=3",
        "Helix 0,0,0 0,0,10 0 Turns=3",
        "Helix 0,0,0 0,0,10 0,0,5 Turns=3",
        "Helix 0,0,0 0,0,10 2 Turns=0",
        "Helix 0,0,0 0,0,10 2 Pitch=-1",
        "Helix 0,0,0 0,0,10 2 Turns=3 Pitch=2",
        "Helix 0,0,0 0,0,10 2 Mode=Turns Pitch=2",
        "Helix 0,0,0 0,0,10 2 Mode=Pitch Turns=3",
        "Helix 0,0,0 0,0,10 2 Mode=Pitch",
        "Helix 0,0,0 0,0,10 2 ReverseTwist=Maybe",
        "Helix 0,0,0 0,0,10 2 ReverseTwist=Yes ReverseTwist=No",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}

#[test]
fn spiral_supports_independent_numeric_and_picked_radii_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let result = registry
        .execute(&mut document, "Spiral 0,0,0 0,0,6 1 4 Turns=2")
        .unwrap();
    assert!(result.contains("Added NURBS spiral"));
    assert!(result.contains("radii 1.000000 to 4.000000"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("Spiral must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(!curve.is_rational());
    assert_eq!(curve.control_points().len(), 243);
    assert_eq!(curve.domain(), 0.0..=2.0);
    assert!(
        curve
            .evaluate(1.0)
            .unwrap()
            .distance_to(Point3::try_new(2.5, 0.0, 3.0).unwrap())
            .unwrap()
            < 2.0e-12
    );
    assert_eq!(document.undo_label(), Some("Spiral"));

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(
            &mut document,
            "Spiral 1,2,3 1,12,3 4,2,3 1,12,-2 Pitch=4 ReverseTwist=Yes",
        )
        .unwrap();
    assert!(result.contains("radii 3.000000 to 5.000000"));
    assert!(result.contains("turns 2.500000, reverse twist"));
    let Geometry::NurbsCurve(reverse) = document.objects().next().unwrap().geometry() else {
        panic!("picked radii and inferred pitch mode must create a NURBS curve")
    };
    assert_eq!(reverse.control_points().len(), 303);
    assert!(
        reverse
            .evaluate(2.5)
            .unwrap()
            .distance_to(Point3::try_new(-4.0, 12.0, 3.0).unwrap())
            .unwrap()
            < 2.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Spiral 0 0 0 0 0 3 0 2 Mode=Turns")
        .unwrap();
    let Geometry::NurbsCurve(zero_start) = document.objects().next().unwrap().geometry() else {
        panic!("a numeric zero start radius must be supported")
    };
    assert_eq!(zero_start.control_points().len(), 363);
    assert!(
        zero_start
            .evaluate(3.0)
            .unwrap()
            .distance_to(Point3::try_new(2.0, 0.0, 3.0).unwrap())
            .unwrap()
            < 2.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Spiral 0,0,0 0,0,6 1 0 0 4 Turns=2")
        .unwrap();
    let Geometry::NurbsCurve(space_point) = document.objects().next().unwrap().geometry() else {
        panic!("a space-separated first radius point must be accepted")
    };
    assert_eq!(space_point.control_points().len(), 243);
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(
            &mut document,
            "Spiral Flat 0,0,0 1,0,0 0,4,0 Turns=1 ReverseTwist=Yes",
        )
        .unwrap();
    let Geometry::NurbsCurve(flat) = document.objects().next().unwrap().geometry() else {
        panic!("Flat must create a planar NURBS spiral")
    };
    assert_eq!(flat.control_points().len(), 123);
    assert_eq!(flat.domain(), 0.0..=1.0);
    assert!(
        flat.control_points()
            .iter()
            .all(|control| { control.point().z().abs() < 2.0e-12 })
    );
    assert!(
        flat.evaluate(0.5)
            .unwrap()
            .distance_to(Point3::try_new(-2.5, 0.0, 0.0).unwrap())
            .unwrap()
            < 2.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Spiral Flat 1,2,3 2 5 Turns=1 Axis=0,1,0")
        .unwrap();
    let Geometry::NurbsCurve(oriented_flat) = document.objects().next().unwrap().geometry() else {
        panic!("Flat Axis must orient the spiral plane")
    };
    assert!(
        oriented_flat
            .control_points()
            .iter()
            .all(|control| (control.point().y() - 2.0).abs() < 2.0e-12)
    );
    registry.execute(&mut document, "Undo").unwrap();

    for invalid in [
        "Spiral",
        "Spiral 0,0,0 0,0,0 1 2 Turns=3",
        "Spiral 0,0,0 0,0,6 0 0 Turns=3",
        "Spiral 0,0,0 0,0,6 -1 2 Turns=3",
        "Spiral 0,0,0 0,0,6 1 -2 Turns=3",
        "Spiral 0,0,0 0,0,6 1 2 Turns=0",
        "Spiral 0,0,0 0,0,6 1 2 Pitch=0",
        "Spiral 0,0,0 0,0,6 1 2 Turns=3 Pitch=2",
        "Spiral 0,0,0 0,0,6 1 2 Mode=Turns Pitch=2",
        "Spiral 0,0,0 0,0,6 1 2 Mode=Pitch Turns=3",
        "Spiral 0,0,0 0,0,6 1 2 Mode=Pitch",
        "Spiral 0,0,0 0,0,6 1 2 ReverseTwist=Maybe",
        "Spiral 0,0,0 0,0,6 1 2 Flat=Yes",
        "Spiral 0,0,0 0,0,6 1 2 AroundCurve=1",
        "Spiral Flat",
        "Spiral Flat 0,0,0 1 2 Pitch=1",
        "Spiral Flat 0,0,0 1 2 Mode=Pitch Pitch=1",
        "Spiral Flat 0,0,0 1 2 Axis=0,0,0",
        "Spiral Flat 0,0,0 1 2 Axis=0,0,1 Axis=0,1,0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}

#[test]
fn spiral_around_curve_supports_named_and_selected_rails_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let rail = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(0.0, 0.0, 10.0).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("Rail"),
        )
        .unwrap();

    let message = registry
        .execute(
            &mut document,
            "Spiral AroundCurve 1,0,0 2 PathName=rail Turns=1 PointsPerTurn=12",
        )
        .unwrap();
    assert!(message.contains("around curve"));
    assert!(message.contains("radii 1.000000 to 2.000000"));
    let curve = document
        .objects()
        .find(|object| object.id() != rail)
        .map(|object| object.geometry())
        .and_then(|geometry| match geometry {
            Geometry::NurbsCurve(curve) => Some(curve),
            _ => None,
        })
        .expect("AroundCurve must add a NURBS curve");
    assert_eq!(curve.control_points().len(), 15);
    assert!((*curve.domain().end() - 1.5 * (10.0 + std::f64::consts::TAU)).abs() < 1.0e-12);
    assert!(
        curve
            .evaluate(*curve.domain().end() * 0.5)
            .unwrap()
            .distance_to(Point3::try_new(-1.5, 0.0, 5.0).unwrap())
            .unwrap()
            < 3.0e-12
    );
    assert_eq!(document.undo_label(), Some("Spiral"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    document
        .select_object(rail, SelectionMode::Replace)
        .unwrap();
    let message = registry
        .execute(
            &mut document,
            "Spiral AroundCurve 1 0 0 1 Pitch=5 ReverseTwist=Yes",
        )
        .unwrap();
    assert!(message.contains("turns 2.000000"));
    assert!(message.contains("reverse twist"));
    let reverse = document
        .objects()
        .find(|object| object.id() != rail)
        .and_then(|object| match object.geometry() {
            Geometry::NurbsCurve(curve) => Some(curve),
            _ => None,
        })
        .expect("selected AroundCurve rail must add a NURBS curve");
    assert_eq!(reverse.control_points().len(), 27);
    assert!(reverse.evaluate(reverse.knots()[4]).unwrap().y() < 0.0);
    registry.execute(&mut document, "Undo").unwrap();

    for invalid in [
        "Spiral AroundCurve",
        "Spiral AroundCurve 1 2 PathName=Rail",
        "Spiral AroundCurve 0,0,2 1 PathName=Rail",
        "Spiral AroundCurve 1,0,0 -1 PathName=Rail",
        "Spiral AroundCurve 1,0,0 1 PathName=Missing",
        "Spiral AroundCurve 1,0,0 1 PathName=Rail PointsPerTurn=4",
        "Spiral AroundCurve 1,0,0 1 PathName=Rail PointsPerTurn=12 PointsPerTurn=16",
        "Spiral AroundCurve 1,0,0 1 PathName=Rail Turns=2 Pitch=1",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn catenary_supports_all_constraint_and_output_modes_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let message = registry
        .execute(
            &mut document,
            "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 PointCount=8",
        )
        .unwrap();
    assert!(message.contains("smooth NURBS catenary"));
    assert!(message.contains("8 points"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("the default through-point mode must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 8);
    assert!(
        curve
            .evaluate(*curve.domain().end())
            .unwrap()
            .distance_to(Point3::try_new(10.0, 0.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    assert_eq!(document.undo_label(), Some("Catenary"));
    registry.execute(&mut document, "Undo").unwrap();

    let message = registry
            .execute(
                &mut document,
                "Catenary 0,0,0 10,0,-2 0,0,-1 13 Mode=Length Output=Polyline PointCount=7 MarkApex=Yes",
            )
            .unwrap();
    assert!(message.contains("polyline catenary"));
    assert!(message.contains("length 13.000000"));
    assert!(message.contains("apex point"));
    let mut objects = document.objects();
    let Geometry::Point(apex) = objects.next().unwrap().geometry() else {
        panic!("MarkApex=Yes must add the analytic apex first")
    };
    assert!(apex.z() < -4.6);
    let Geometry::Polyline(polyline) = objects.next().unwrap().geometry() else {
        panic!("Output=Polyline must add a polyline")
    };
    assert_eq!(polyline.vertices().len(), 7);
    assert!(
        polyline.vertices()[3]
            .distance_to(Point3::try_new(5.0, 0.0, -4.618_680_100_732_663).unwrap())
            .unwrap()
            < 2.0e-12
    );
    drop(objects);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);

    let parameter = registry
        .execute(
            &mut document,
            "Catenary 1 2 3 8 4 -1 1 2 2 3 Mode=Parameter PointCount=9",
        )
        .unwrap();
    assert!(parameter.contains("parameter 3.000000"));
    registry.execute(&mut document, "Undo").unwrap();
    let apex = registry
        .execute(
            &mut document,
            "Catenary 0,0,0 10,0,-2 0,0,-1 7,4,-4 Mode=Apex Output=Polyline PointCount=7",
        )
        .unwrap();
    assert!(apex.contains("polyline catenary"));
    registry.execute(&mut document, "Undo").unwrap();

    for command in [
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,2 MarkApex=Yes",
        "Catenary 0,0,0 10,0,0 0,0,-1 5,0,2 Mode=Apex MarkApex=Yes",
        "Catenary 0,0,0 10,0,0 0,0,-1 -4 Mode=Parameter MarkApex=Yes",
        "Catenary 0,0,0 10,0,0 0,0,-1 -14 Mode=Length MarkApex=Yes",
    ] {
        registry.execute(&mut document, command).unwrap();
        let Geometry::Point(apex) = document.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert!(apex.z() > 0., "{command}");
        registry.execute(&mut document, "Undo").unwrap();
    }
    for invalid in [
        "Catenary",
        "Catenary 0,0,0 10,0,0 0,0,0 3,0,-2",
        "Catenary 0,0,0 0,0,-10 0,0,-1 4 Mode=Parameter",
        "Catenary 0,0,0 10,0,0 0,0,-1 0 Mode=Parameter",
        "Catenary 0,0,0 10,0,0 0,0,-1 10 Mode=Length",
        "Catenary 0,0,0 10,0,0 0,0,-1 10,0,0",
        "Catenary 0,0,0 10,0,-2 0,0,-1 5,0,-1 Mode=Apex",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 PointCount=3",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 Output=Polyline PointCount=1",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 Mode=Unknown",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 Output=Mesh",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 MarkApex=Maybe",
        "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 Mode=ThroughPoint Mode=Apex",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}
