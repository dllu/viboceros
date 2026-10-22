//! curves through command regressions.
use super::*;

#[test]
fn curve_through_point_objects_sorts_culls_and_retains_sources() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let points = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.0).unwrap(),
        Point3::try_new(2.0 + 2.0e-10, -1.0, 1.0).unwrap(),
        Point3::try_new(2.0, -1.0, 1.0).unwrap(),
        Point3::try_new(2.0, -1.0, 1.0).unwrap(),
        Point3::try_new(4.0, 3.0, 0.0).unwrap(),
        Point3::try_new(6.0, -2.0, 1.0).unwrap(),
        Point3::try_new(8.0, 1.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
    ];
    let ids = points
        .into_iter()
        .map(|point| document.add_geometry(Geometry::Point(point)).unwrap())
        .collect::<Vec<_>>();
    document
        .select_objects(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(
            &mut document,
            "CurveThroughPt CurveType=Interpolated Knots=Uniform Degree=3",
        )
        .unwrap();
    assert!(message.contains("through 8 unique point object(s)"));
    assert_eq!(document.objects().len(), ids.len() + 1);
    let output = document.objects().last().unwrap();
    assert!(!document.is_selected(output.id()));
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("CurveThroughPt must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 10);
    assert_eq!(
        curve.evaluate(*curve.domain().start()).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap()
    );
    assert!(
        curve
            .evaluate(*curve.domain().end())
            .unwrap()
            .distance_to(Point3::try_new(4.0, 3.0, 0.0).unwrap())
            .unwrap()
            < 1.0e-12
    );
    assert!(ids.iter().all(|id| document.is_selected(*id)));
    assert_eq!(document.undo_label(), Some("CurveThroughPt"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), ids.len());
    assert!(ids.iter().all(|id| document.is_selected(*id)));

    let message = registry
        .execute(
            &mut document,
            "CurveThroughPt CurveType=ControlPoint Degree=3 Closed=Yes",
        )
        .unwrap();
    assert!(message.contains("Added closed degree 3 control-point curve"));
    let Geometry::NurbsCurve(closed) = document.objects().last().unwrap().geometry() else {
        panic!("closed CurveThroughPt must create a NURBS curve")
    };
    assert!(closed.is_periodic());
    assert!(closed.is_closed().unwrap());
    assert_eq!(closed.domain(), 0.0..=8.0);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), ids.len());
}

#[test]
fn curve_through_polylines_copies_or_replaces_sources_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let current_layer = document.current_layer_id();
    let source_layer = document
        .add_layer("Curve sources", ColorRgb::new(70, 80, 90))
        .unwrap();
    let open = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 1.0, 0.0).unwrap(),
            Point3::try_new(4.0, -1.0, 0.0).unwrap(),
            Point3::try_new(7.0, 2.0, 1.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let closed = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 5.0, 0.0).unwrap(),
            Point3::try_new(2.0, 5.0, 0.0).unwrap(),
            Point3::try_new(2.0, 7.0, 0.0).unwrap(),
            Point3::try_new(0.0, 7.0, 0.0).unwrap(),
            Point3::try_new(0.0, 5.0, 0.0).unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(source_layer)
        .with_name("Source polyline")
        .with_object_color(ColorRgb::new(11, 22, 33));
    let open_id = document
        .add_geometry_with_attributes(Geometry::Polyline(open.clone()), attributes.clone())
        .unwrap();
    let closed_id = document
        .add_geometry_with_attributes(Geometry::Polyline(closed.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Curve sources".to_owned()), [open_id, closed_id])
        .unwrap();
    document
        .select_objects([open_id, closed_id], SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "CurveThroughPolyline Degree=3")
        .unwrap();
    assert!(message.contains("Created 2 control-point curve(s)"));
    assert!(message.contains("retaining the input(s)"));
    assert_eq!(document.objects().len(), 4);
    assert!(document.is_selected(open_id));
    assert!(document.is_selected(closed_id));
    for output in document.objects().skip(2) {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().layer_id(), current_layer);
        let Geometry::NurbsCurve(_) = output.geometry() else {
            panic!("CurveThroughPolyline must create NURBS curves")
        };
    }
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);

    let message = registry
        .execute(
            &mut document,
            "CurveThroughPolyline CurveType=Interpolated Knots=Chord DeleteInput=Yes",
        )
        .unwrap();
    assert!(message.contains("replacing the input(s)"));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [open_id, closed_id]
    );
    let Geometry::NurbsCurve(open_curve) = document.object(open_id).unwrap().geometry() else {
        panic!("the open polyline must be replaced in place")
    };
    assert!(!open_curve.is_periodic());
    let Geometry::NurbsCurve(closed_curve) = document.object(closed_id).unwrap().geometry() else {
        panic!("the closed polyline must be replaced in place")
    };
    assert!(closed_curve.is_periodic());
    assert_eq!(document.object(open_id).unwrap().attributes(), &attributes);
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([open_id, closed_id])
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(open_id).unwrap().geometry(),
        &Geometry::Polyline(open)
    );
    assert_eq!(
        document.object(closed_id).unwrap().geometry(),
        &Geometry::Polyline(closed)
    );
}

#[test]
fn curve_through_commands_reject_invalid_inputs_without_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for invalid in [
        "CurveThroughPt",
        "CurveThroughPt Degree=0",
        "CurveThroughPt Degree=12",
        "CurveThroughPt CurveType=Interpolated Degree=5",
        "CurveThroughPt CurveType=Other",
        "CurveThroughPt Knots=Other",
        "CurveThroughPt Closed=Maybe",
        "CurveThroughPt DeleteInput=Yes",
        "CurveThroughPolyline",
        "CurveThroughPolyline Closed=Yes",
        "CurveThroughPolyline DeleteInput=Maybe",
    ] {
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{invalid}");
    }

    let coincident = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let first = document.add_geometry(Geometry::Point(coincident)).unwrap();
    let second = document.add_geometry(Geometry::Point(coincident)).unwrap();
    document
        .select_objects([first, second], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "CurveThroughPt"),
        Err(CommandError::NotEnoughCurveThroughPoints)
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}
