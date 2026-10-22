//! primitives conics command regressions.
use super::*;

#[test]
fn conic_supports_rho_through_point_and_apex_pick_orders() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let result = registry
        .execute(&mut document, "Conic 0,0,0 10,0,0 5,5,0 0.25")
        .unwrap();
    assert!(result.contains("Added exact NURBS conic"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("Conic must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 2);
    assert!(curve.is_rational());
    assert_eq!(curve.domain(), 0.0..=1.0);
    assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(
        curve.control_points()[0].point(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        curve.control_points()[1].point(),
        Point3::try_new(5.0, 5.0, 0.0).unwrap()
    );
    assert_eq!(curve.control_points()[1].weight(), 1.0 / 3.0);
    assert_eq!(
        curve.control_points()[2].point(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Conic"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Conic 0 0 0 Apex 5 5 0 10 0 0 0.75")
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("the Apex option must preserve start-to-end direction")
    };
    assert_eq!(
        curve.control_points()[0].point(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(curve.control_points()[1].weight(), 3.0);
    assert_eq!(
        curve.control_points()[2].point(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Conic Default 0,0,0 10,0,0 5,5,0 5,2,37")
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("a through-point conic must create a NURBS curve")
    };
    assert!((curve.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-15);
    assert!(
        curve
            .evaluate(0.5)
            .unwrap()
            .distance_to(Point3::try_new(5.0, 2.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-14
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Conic 0,0,0 0,0,0 0,0,0 0.5")
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("rho input must retain Rhino's degenerate conic controls")
    };
    assert!(
        curve
            .control_points()
            .iter()
            .all(|control| control.point() == Point3::try_new(0.0, 0.0, 0.0).unwrap())
    );
    registry.execute(&mut document, "Undo").unwrap();

    for invalid in [
        "Conic",
        "Conic 0,0,0 10,0,0 5,5,0 0",
        "Conic 0,0,0 10,0,0 5,5,0 1",
        "Conic 0,0,0 10,0,0 5,5,0 nan",
        "Conic 0,0,0 10,0,0 5,5,0 5,0,0",
        "Conic 0,0,0 10,0,0 5,5,0 5,6,0",
        "Conic Apex 0,0,0 5,5,0 10,0,0 0.5 extra",
        "Conic Apex 0,0,0 Apex 5,5,0 10,0,0 0.5",
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
fn parabola_supports_focus_vertex_half_and_marker_forms() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let result = registry
        .execute(&mut document, "Parabola 0,0,1 0,0,2 2,0,4")
        .unwrap();
    assert!(result.contains("full NURBS parabola"));
    let Geometry::NurbsCurve(full) = document.objects().next().unwrap().geometry() else {
        panic!("Parabola's default Focus form must create a NURBS curve")
    };
    let focal_distance = (13.0_f64.sqrt() - 3.0) * 0.5;
    let vertex = Point3::try_new(0.0, 0.0, 1.0 - focal_distance).unwrap();
    assert_eq!(full.degree(), 2);
    assert_eq!(full.domain(), 0.0..=1.0);
    assert!(
        full.evaluate(0.0)
            .unwrap()
            .distance_to(Point3::try_new(-2.0, 0.0, 4.0).unwrap())
            .unwrap()
            < 1.0e-14
    );
    assert!(full.evaluate(0.5).unwrap().distance_to(vertex).unwrap() < 1.0e-14);
    assert!(
        full.evaluate(1.0)
            .unwrap()
            .distance_to(Point3::try_new(2.0, 0.0, 4.0).unwrap())
            .unwrap()
            < 1.0e-14
    );
    assert!(
        full.control_points()[1]
            .point()
            .distance_to(Point3::try_new(0.0, 0.0, 1.0 - 13.0_f64.sqrt()).unwrap())
            .unwrap()
            < 1.0e-14
    );
    assert_eq!(document.undo_label(), Some("Parabola"));

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(
            &mut document,
            "Parabola Vertex 1,2,3 2,2,3 2,5,8 Half=Yes MarkFocus=Yes",
        )
        .unwrap();
    assert!(result.contains("half NURBS parabola"));
    assert!(result.contains("focus point"));
    let mut objects = document.objects();
    let Geometry::Point(marked_focus) = objects.next().unwrap().geometry() else {
        panic!("MarkFocus=Yes must add the focus before the parabola")
    };
    assert_eq!(*marked_focus, Point3::try_new(2.0, 2.0, 3.0).unwrap());
    let Geometry::NurbsCurve(half) = objects.next().unwrap().geometry() else {
        panic!("Half=Yes must add a half-parabola NURBS curve")
    };
    assert_eq!(
        half.control_points()[0].point(),
        Point3::try_new(1.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        half.control_points()[1].point(),
        Point3::try_new(1.0, 3.5, 5.5).unwrap()
    );
    assert_eq!(
        half.control_points()[2].point(),
        Point3::try_new(9.5, 5.0, 8.0).unwrap()
    );
    drop(objects);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);

    for invalid in [
        "Parabola",
        "Parabola Focus 0,0,0 0,0,0 1,0,0",
        "Parabola Focus 0,0,0 0,0,1 0,0,2",
        "Parabola Vertex 0,0,0 0,0,0 1,0,0",
        "Parabola Vertex 0,0,0 0,0,1 0,0,2",
        "Parabola Vertex 0,0,0 0,0,1 nan,0,0",
        "Parabola Vertex 0,0,0 0,0,1 2,0,0 Half=Maybe",
        "Parabola Vertex 0,0,0 0,0,1 2,0,0 Half=Yes Half=No",
        "Parabola Vertex 0,0,0 0,0,1 2,0,0 MarkFocus=Yes MarkFocus=No",
        "Parabola Vertex 0,0,0 0,0,1 2,0,0 Solid=Yes",
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
fn parabola_three_point_supports_all_modes_pick_orders_and_focus_marker() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let result = registry
            .execute(
                &mut document,
                "Parabola3Pt -1,0,0.25 1,0,0.25 3,0,2.25 3,0,3.25 Mode=ThroughPoint PickOrder=Default MarkFocus=Yes",
            )
            .unwrap();
    assert!(result.contains("three-point NURBS parabola"));
    assert!(result.contains("focus point"));
    let mut objects = document.objects();
    let Geometry::Point(focus) = objects.next().unwrap().geometry() else {
        panic!("MarkFocus=Yes must add the focus first")
    };
    assert!(
        focus
            .distance_to(Point3::try_new(0.0, 0.0, 1.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    let Geometry::NurbsCurve(curve) = objects.next().unwrap().geometry() else {
        panic!("the default ThroughPoint mode must add a NURBS curve")
    };
    assert_eq!(curve.degree(), 2);
    assert_eq!(curve.domain(), 0.0..=1.0);
    assert!(
        curve.control_points()[1]
            .point()
            .distance_to(Point3::try_new(1.0, 0.0, -0.75).unwrap())
            .unwrap()
            < 1.0e-12
    );
    drop(objects);
    assert_eq!(document.undo_label(), Some("Parabola3Pt"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(
            &mut document,
            "Parabola3Pt Mode=Focus PickOrder=EndsFirst -1,0,0.25 3,0,2.25 0,0,1 MarkFocus=No",
        )
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("Focus mode must add a NURBS curve")
    };
    assert!(
        curve.control_points()[1]
            .point()
            .distance_to(Point3::try_new(-1.0, 0.0, 2.75).unwrap())
            .unwrap()
            < 1.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    let vertex_forms = [
        "Parabola3Pt -1,0,0.25 0,0,0 3,0,2.25 Mode=Vertex PickOrder=Default MarkFocus=No",
        "Parabola3Pt -1,0,0.25 3,0,2.25 0,0,0 Mode=Vertex PickOrder=EndsFirst MarkFocus=No",
        "Parabola3Pt 0,0,0 -1,0,0.25 3,0,2.25 Mode=Vertex PickOrder=EndsLast MarkFocus=No",
    ];
    for command in vertex_forms {
        registry.execute(&mut document, command).unwrap();
        let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
            panic!("Vertex mode must add a NURBS curve")
        };
        assert_eq!(
            curve.control_points()[0].point(),
            Point3::try_new(-1.0, 0.0, 0.25).unwrap()
        );
        assert!(
            curve.control_points()[1]
                .point()
                .distance_to(Point3::try_new(1.0, 0.0, -0.75).unwrap())
                .unwrap()
                < 1.0e-12
        );
        assert_eq!(
            curve.control_points()[2].point(),
            Point3::try_new(3.0, 0.0, 2.25).unwrap()
        );
        registry.execute(&mut document, "Undo").unwrap();
    }

    for invalid in [
        "Parabola3Pt",
        "Parabola3Pt -1,0,0.25 1,0,0.25 3,0,2.25",
        "Parabola3Pt -1,0,0.25 1,0,0.25 3,0,2.25 1,0,0.25",
        "Parabola3Pt 0,0,0 -1,0,0 1,0,0 Mode=Vertex",
        "Parabola3Pt -1,0,0 1,0,0 0,0,0 Mode=Focus PickOrder=EndsFirst",
        "Parabola3Pt 0,0,0 1,0,0 2,0,0 3,0,0 Mode=Vertex",
        "Parabola3Pt 0,0,0 1,0,0 2,0,0 Mode=Unknown",
        "Parabola3Pt 0,0,0 1,0,0 2,0,0 Mode=Vertex PickOrder=Middle",
        "Parabola3Pt 0,0,0 1,0,0 2,0,0 Mode=Vertex Mode=Focus",
        "Parabola3Pt 0,0,0 1,0,0 2,0,0 Mode=Vertex MarkFocus=Yes MarkFocus=No",
    ] {
        let registry = CommandRegistry::with_builtins();
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}

#[test]
fn hyperbola_supports_all_constructions_branches_markers_and_preview_option() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();

    let result = registry
        .execute(&mut document, "Hyperbola 0,0,0 5,0,0 3.75,3,0")
        .unwrap();
    assert!(result.contains("Added 1 hyperbola branch"));
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("the default center form must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 2);
    assert!(curve.is_rational());
    assert_eq!(curve.domain(), 0.0..=1.0);
    assert_eq!(curve.control_points()[1].weight(), 1.25);
    assert!(
        curve.control_points()[1]
            .point()
            .distance_to(Point3::try_new(2.4, 0.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    let result = registry
            .execute(
                &mut document,
                "Hyperbola FromFoci -5,0,0 5,0,0 3.75,3,0 BothBranches=Yes MarkFoci=Yes ShowAsymptotes=Yes",
            )
            .unwrap();
    assert!(result.contains("Added 2 hyperbola branches"));
    assert!(result.contains("asymptotes are preview-only"));
    let mut objects = document.objects();
    let Geometry::Point(first_focus) = objects.next().unwrap().geometry() else {
        panic!("the negative focus must be added first")
    };
    let Geometry::Point(second_focus) = objects.next().unwrap().geometry() else {
        panic!("the positive focus must be added second")
    };
    assert_eq!(*first_focus, Point3::try_new(-5.0, 0.0, 0.0).unwrap());
    assert_eq!(*second_focus, Point3::try_new(5.0, 0.0, 0.0).unwrap());
    let Geometry::NurbsCurve(negative) = objects.next().unwrap().geometry() else {
        panic!("the negative branch must precede the positive branch")
    };
    let Geometry::NurbsCurve(positive) = objects.next().unwrap().geometry() else {
        panic!("the positive branch must be last")
    };
    assert!(negative.control_points()[0].point().x() < 0.0);
    assert!(positive.control_points()[0].point().x() > 0.0);
    drop(objects);
    assert_eq!(document.undo_label(), Some("Hyperbola"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
            .execute(
                &mut document,
                "Hyperbola FromVertex 3,0,0 5,0,0 3.75,3,0 BothBranches=No MarkFoci=Yes ShowAsymptotes=No",
            )
            .unwrap();
    let mut objects = document.objects();
    let Geometry::Point(focus) = objects.next().unwrap().geometry() else {
        panic!("FromVertex must mark its selected focus")
    };
    assert_eq!(*focus, Point3::try_new(5.0, 0.0, 0.0).unwrap());
    let Geometry::NurbsCurve(curve) = objects.next().unwrap().geometry() else {
        panic!("FromVertex must create a NURBS curve")
    };
    assert!((curve.control_points()[1].weight() - 1.25).abs() < 1.0e-12);
    drop(objects);
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(
            &mut document,
            "Hyperbola FromVertex 4,0,0 5,0,0 5,2.25,0 MarkFoci=No",
        )
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("FromVertex must solve a second coefficient family")
    };
    assert!(
        curve.control_points()[1]
            .point()
            .distance_to(Point3::try_new(3.2, 0.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    assert!((curve.control_points()[1].weight() - 1.25).abs() < 1.0e-12);
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(
            &mut document,
            "Hyperbola FromCoefficient 0,0,0 1,0,0 5,2,0 A=3 B=4 MarkFoci=Yes",
        )
        .unwrap();
    let mut objects = document.objects();
    let Geometry::Point(focus) = objects.next().unwrap().geometry() else {
        panic!("FromCoefficient must mark its positive focus")
    };
    assert_eq!(*focus, Point3::try_new(5.0, 0.0, 0.0).unwrap());
    let Geometry::NurbsCurve(curve) = objects.next().unwrap().geometry() else {
        panic!("FromCoefficient must create a NURBS curve")
    };
    assert!(
        curve.control_points()[0]
            .point()
            .distance_to(Point3::try_new(5.0, 16.0 / 3.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    assert!(
        curve.control_points()[2]
            .point()
            .distance_to(Point3::try_new(5.0, -16.0 / 3.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    drop(objects);
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(
            &mut document,
            "Hyperbola Center 1,2,3 4,6,3 0.85,6.8,3 MarkFoci=No",
        )
        .unwrap();
    let Geometry::NurbsCurve(curve) = document.objects().next().unwrap().geometry() else {
        panic!("an oblique center form must create a NURBS curve")
    };
    assert!(
        curve.control_points()[0]
            .point()
            .distance_to(Point3::try_new(5.65, 3.2, 3.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    assert!(
        curve.control_points()[2]
            .point()
            .distance_to(Point3::try_new(0.85, 6.8, 3.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    registry.execute(&mut document, "Undo").unwrap();

    for invalid in [
        "Hyperbola",
        "Hyperbola 0,0,0 0,0,0 3,2,0",
        "Hyperbola 0,0,0 5,0,0 -3.75,3,0",
        "Hyperbola 0,0,0 5,0,0 3,0,0",
        "Hyperbola FromFoci -5,0,0 -5,0,0 3.75,3,0",
        "Hyperbola FromFoci 5,0,0 -5,0,0 3.75,3,0",
        "Hyperbola FromVertex 3,0,0 3,0,0 4,2,0",
        "Hyperbola FromVertex 3,0,0 5,0,0 3.75,1,0",
        "Hyperbola FromCoefficient 0,0,0 0,0,0 5,2,0 A=3 B=4",
        "Hyperbola FromCoefficient 0,0,0 1,0,0 2,2,0 A=3 B=4",
        "Hyperbola FromCoefficient 0,0,0 1,0,0 3,0,0 A=3 B=4",
        "Hyperbola FromCoefficient 0,0,0 1,0,0 5,2,0 A=0 B=4",
        "Hyperbola FromCoefficient 0,0,0 1,0,0 5,2,0 A=3 B=-1",
        "Hyperbola 0,0,0 5,0,0 3.75,3,0 A=3",
        "Hyperbola 0,0,0 5,0,0 3.75,3,0 BothBranches=Yes BothBranches=No",
        "Hyperbola 0,0,0 5,0,0 3.75,3,0 ShowAsymptotes=Maybe",
    ] {
        let registry = CommandRegistry::with_builtins();
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}
