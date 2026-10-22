//! curves tween command regressions.
use super::*;

#[test]
fn tween_curves_preserves_sources_selection_order_and_output_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let start_layer = document
        .add_layer("Tween start", ColorRgb::new(30, 40, 50))
        .unwrap();
    let end_layer = document
        .add_layer("Tween end", ColorRgb::new(60, 70, 80))
        .unwrap();
    let start = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, -1.0, 0.0).unwrap(),
            Point3::try_new(5.0, 0.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 5.0, 5.0],
    )
    .unwrap();
    let end = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 6.0, 2.0).unwrap(),
            Point3::try_new(1.0, 5.0, 3.0).unwrap(),
            Point3::try_new(3.0, 9.0, -1.0).unwrap(),
            Point3::try_new(5.0, 6.0, 2.0).unwrap(),
        ],
        vec![10.0, 10.0, 10.0, 10.0, 30.0, 30.0, 30.0, 30.0],
    )
    .unwrap();
    let start_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(start),
            ObjectAttributes::on_layer(start_layer).with_name("start"),
        )
        .unwrap();
    let end_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(end),
            ObjectAttributes::on_layer(end_layer).with_name("end"),
        )
        .unwrap();
    document
        .select_objects([start_id, end_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "TweenCurves Number=2 MatchMethod=None OutputLayer=StartCrv",
            )
            .unwrap(),
        "Created 2 tween curve(s) using control-point matching on the start-curve layer"
    );
    assert_eq!(document.objects().len(), 4);
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [start_id, end_id]
    );
    let outputs = document.objects().skip(2).collect::<Vec<_>>();
    assert!(
        outputs
            .iter()
            .all(|object| !document.is_selected(object.id()))
    );
    assert!(
        outputs
            .iter()
            .all(|object| object.attributes().layer_id() == start_layer)
    );
    assert!(
        outputs
            .iter()
            .all(|object| object.attributes().name().is_none())
    );
    let Geometry::NurbsCurve(first) = outputs[0].geometry() else {
        panic!("TweenCurves must create NURBS curves")
    };
    assert_eq!(first.domain(), 0.0..=5.0);
    assert_eq!(
        first.control_points()[0].point(),
        Point3::try_new(0.0, 2.0, 2.0 / 3.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("TweenCurves"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        [start_id, end_id]
    );
}

#[test]
fn tween_curves_sampling_flips_direction_and_uses_end_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let end_layer = document
        .add_layer("Tween destination", ColorRgb::new(90, 100, 110))
        .unwrap();
    let start_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(9.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let end_id = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 6.0, 0.0).unwrap(),
                    Point3::try_new(12.0, 6.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(end_layer),
        )
        .unwrap();
    document
        .select_objects([start_id, end_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(
            &mut document,
            "TweenCurves MatchMethod=SamplePoints SampleNumber=4 FlipEnd=Yes OutputLayer=EndCrv",
        )
        .unwrap();
    let output = document.objects().last().unwrap();
    assert_eq!(output.attributes().layer_id(), end_layer);
    let Geometry::NurbsCurve(curve) = output.geometry() else {
        panic!("sample matching must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 1);
    assert_eq!(curve.control_points().len(), 5);
    assert_eq!(
        curve.control_points()[0].point(),
        Point3::try_new(6.0, 3.0, 0.0).unwrap()
    );
    assert_eq!(
        curve.control_points()[4].point(),
        Point3::try_new(4.5, 3.0, 0.0).unwrap()
    );
    assert!(!curve.is_closed().unwrap());
}

#[test]
fn tween_curves_rejects_invalid_inputs_and_refits_different_structures() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let line = |y| {
        Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, y, 0.0).unwrap(),
                Point3::try_new(5.0, y, 0.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        )
    };
    let ids = [0.0, 2.0, 4.0]
        .into_iter()
        .map(|y| document.add_geometry(line(y)).unwrap())
        .collect::<Vec<_>>();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();

    for command in [
        "TweenCurves Number=0",
        "TweenCurves Number=1000001",
        "TweenCurves Number=nope",
        "TweenCurves MatchMethod=Other",
        "TweenCurves MatchMethod=None SampleNumber=8",
        "TweenCurves MatchMethod=SamplePoints SampleNumber=1",
        "TweenCurves MatchMethod=SamplePoints SampleNumber=10000",
        "TweenCurves OutputLayer=Other",
        "TweenCurves FlipStart=Maybe",
        "TweenCurves Number=1 Number=2",
        "TweenCurves extra",
    ] {
        let object_count = document.objects().len();
        let history = document.undo_label().map(str::to_owned);
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), object_count, "{command}");
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    document
        .select_objects([ids[0], point_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "TweenCurves"),
        Err(CommandError::TweenCurvesRequiresTwoCurves { actual: 1 })
    ));
    document
        .select_objects(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "TweenCurves"),
        Err(CommandError::TweenCurvesRequiresTwoCurves { actual: 3 })
    ));

    document
        .select_objects([ids[0], ids[1]], SelectionMode::Replace)
        .unwrap();
    let object_count = document.objects().len();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "TweenCurves Number=500001"),
        Err(CommandError::Geometry(
            GeometryError::TooManyCurveTweenControlPoints { .. }
        ))
    ));
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.undo_label(), history.as_deref());

    let different_structure = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 8.0, 0.0).unwrap(),
            Point3::try_new(5.0 / 3.0, 8.0, 0.0).unwrap(),
            Point3::try_new(10.0 / 3.0, 8.0, 0.0).unwrap(),
            Point3::try_new(5.0, 8.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let different_structure_id = document
        .add_geometry(Geometry::NurbsCurve(different_structure))
        .unwrap();
    document
        .select_objects([ids[0], different_structure_id], SelectionMode::Replace)
        .unwrap();
    let object_count = document.objects().len();
    assert_eq!(
        registry
            .execute(&mut document, "TweenCurves MatchMethod=Refit")
            .unwrap(),
        "Created 1 tween curve(s) using refit matching on the current layer"
    );
    assert_eq!(document.objects().len(), object_count + 1);
    let Geometry::NurbsCurve(curve) = document.objects().last().unwrap().geometry() else {
        panic!("refit matching must create a NURBS curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(!curve.is_rational());
    assert_eq!(document.undo_label(), Some("TweenCurves"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), object_count);
}
