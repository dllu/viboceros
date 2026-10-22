//! curves closure command regressions.
use super::*;

#[test]
fn closes_polylines_without_changing_identity_attributes_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Boundary")
        .unwrap();
    let layer_id = document.current_layer_id();
    registry
        .execute(&mut document, "Polyline 0,0 4,0 2,3")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Loop").unwrap();

    assert_eq!(
        registry.execute(&mut document, "CloseCrv").unwrap(),
        "Closed 1 of 1 selected curve(s): 1 with a line, 0 by moving an endpoint; 0 unchanged"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.attributes().layer_id(), layer_id);
    let Geometry::PolyCurve(closed) = object.geometry() else {
        panic!("expected a polycurve retaining the original polyline")
    };
    assert!(closed.is_closed().unwrap());
    assert_eq!(closed.segments().len(), 2);
    assert_eq!(
        closed.segments()[0].extract_point_locations().unwrap(),
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(closed.parameters(), &[0.0, 2.0, 2.0 + 13.0_f64.sqrt()]);
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("Loop")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("CloseCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert!(matches!(
        document.object(id).unwrap().geometry(),
        Geometry::Polyline(polyline) if !polyline.is_closed()
    ));
    registry.execute(&mut document, "Redo").unwrap();
    assert!(matches!(
        document.object(id).unwrap().geometry(),
        Geometry::PolyCurve(curve) if curve.is_closed().unwrap()
    ));

    registry.execute(&mut document, "Line 10,0 14,0").unwrap();
    let line_id = document.objects().last().unwrap().id();
    document.clear_selection();
    document.select_object(line_id, SelectionMode::Add).unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert_eq!(
        registry.execute(&mut document, "CloseCrv").unwrap(),
        "Closed 0 of 1 selected curve(s): 0 with a line, 0 by moving an endpoint; 1 unchanged"
    );
    assert!(matches!(
        document.object(line_id).unwrap().geometry(),
        Geometry::Line(_)
    ));
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn closes_near_polyline_endpoints_and_respects_wide_gap_policy_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Polyline 0,0 2,0 2,2 0,0.0000000005")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    registry.execute(&mut document, "SelAll").unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "CloseCrv CloseWideGapsWithLine=No Tolerance=0.000000001",
            )
            .unwrap(),
        "Closed 1 of 1 selected curve(s): 0 with a line, 1 by moving an endpoint; 0 unchanged"
    );
    let Geometry::Polyline(closed) = document.object(id).unwrap().geometry() else {
        panic!("expected a closed polyline")
    };
    assert!(closed.is_closed());
    assert_eq!(closed.segment_count(), 3);

    registry.execute(&mut document, "Undo").unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert_eq!(
        registry
            .execute(
                &mut document,
                "CloseCrv CloseWideGapsWithLine No Tolerance 0.0000000001",
            )
            .unwrap(),
        "Closed 0 of 1 selected curve(s): 0 with a line, 0 by moving an endpoint; 1 unchanged"
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(matches!(
        document.object(id).unwrap().geometry(),
        Geometry::Polyline(polyline) if !polyline.is_closed()
    ));

    registry.execute(&mut document, "Point 1,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut document, "CloseCrv"),
        Err(CommandError::UnsupportedCloseCurveGeometry)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert!(
        registry
            .execute(&mut document, "CloseCrv Tolerance=-1")
            .is_err()
    );
}

#[test]
fn changes_rational_curve_seam_without_losing_identity_attributes_groups_or_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SeamTests")
        .unwrap();
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([0.0, 0.0, 0.0], 0.5),
            ([3.0, -1.0, 0.0], 1.3),
            ([5.0, 3.0, 0.0], 0.8),
            ([0.0, 4.0, 0.0], 1.7),
            ([0.0, 0.0, 0.0], 2.0),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(Point3::try_from(coordinates).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![4.0, 4.0, 4.0, 5.0, 9.0, 12.0, 12.0, 12.0],
    )
    .unwrap();
    let expected = source.try_change_closed_seam(7.0).unwrap();
    let attributes =
        ObjectAttributes::on_layer(document.current_layer_id()).with_name("Weighted closed loop");
    let id = document
        .add_geometry_with_attributes(Geometry::NurbsCurve(source.clone()), attributes.clone())
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry.execute(&mut document, "Group SeamSet").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "CrvSeam Parameter=7")
            .unwrap(),
        "Set the selected closed curve seam at parameter 7"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.attributes(), &attributes);
    let Geometry::NurbsCurve(actual) = object.geometry() else {
        panic!("seam relocation must preserve the NURBS class");
    };
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(actual.knots(), expected.knots());
    assert_eq!(actual.is_periodic(), expected.is_periodic());
    assert_eq!(
        actual.control_points().len(),
        expected.control_points().len()
    );
    // Native refinement and projection differ from portable arithmetic by
    // an ULP; definition geometry is compared numerically, history exactly.
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-12);
        assert!((a.weight() - e.weight()).abs() < 1e-12);
    }
    let relocated_geometry = object.geometry().clone();
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("SeamSet")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("CrvSeam"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.object(id).unwrap().geometry(), &relocated_geometry);
}

#[test]
fn changes_polyline_and_analytic_curve_seams_from_model_points() {
    let registry = CommandRegistry::with_builtins();
    let mut polyline_document = Document::default();
    registry
        .execute(&mut polyline_document, "Polyline 0,0 4,0 4,3 0,3 0,0")
        .unwrap();
    let polyline_id = polyline_document.objects().next().unwrap().id();
    registry.execute(&mut polyline_document, "SelAll").unwrap();
    registry
        .execute(&mut polyline_document, "CrvSeam 4,1")
        .unwrap();
    let Geometry::Polyline(polyline) = polyline_document.object(polyline_id).unwrap().geometry()
    else {
        panic!("seam editing must preserve the native polyline")
    };
    assert!(
        polyline
            .evaluate(*polyline.domain().start())
            .unwrap()
            .is_near(
                Point3::try_new(4.0, 1.0, 0.0).unwrap(),
                polyline_document.tolerance(),
            )
    );
    assert!(polyline.is_closed());

    let mut circle_document = Document::default();
    registry
        .execute(&mut circle_document, "Circle 0,0 5")
        .unwrap();
    let circle_id = circle_document.objects().next().unwrap().id();
    registry.execute(&mut circle_document, "SelAll").unwrap();
    registry
        .execute(&mut circle_document, "CrvSeam 0,5")
        .unwrap();
    let Geometry::Circle(circle) = circle_document.object(circle_id).unwrap().geometry() else {
        panic!("seam editing must preserve the analytic circle")
    };
    assert!(circle.evaluate(*circle.domain().start()).unwrap().is_near(
        Point3::try_new(0.0, 5.0, 0.0).unwrap(),
        circle_document.tolerance(),
    ));
    assert_eq!(*circle.domain().start(), 5.0 * std::f64::consts::FRAC_PI_2);
}

#[test]
fn curve_seam_rejects_invalid_or_ambiguous_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut empty = Document::default();
    for command in [
        "CrvSeam",
        "CrvSeam Parameter",
        "CrvSeam Parameter=1 extra",
        "CrvSeam 1,2,3 extra",
    ] {
        assert!(matches!(
            registry.execute(&mut empty, command),
            Err(CommandError::Usage(CURVE_SEAM_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut empty, "CrvSeam 0,0"),
        Err(CommandError::CurveSeamRequiresOneClosedCurve { actual: 0 })
    ));

    registry.execute(&mut empty, "Line 0,0 3,0").unwrap();
    registry.execute(&mut empty, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut empty, "CrvSeam 1,0"),
        Err(CommandError::CurveSeamRequiresOneClosedCurve { actual: 0 })
    ));

    let mut multiple = Document::default();
    registry.execute(&mut multiple, "Circle 0,0 2").unwrap();
    registry
        .execute(&mut multiple, "Polyline 5,0 8,0 8,3 5,0")
        .unwrap();
    registry.execute(&mut multiple, "SelAll").unwrap();
    let before = multiple.objects().cloned().collect::<Vec<_>>();
    let history = multiple.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut multiple, "CrvSeam 1,1"),
        Err(CommandError::CurveSeamRequiresOneClosedCurve { actual: 2 })
    ));
    assert_eq!(
        multiple.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(multiple.undo_label(), history.as_deref());

    multiple.clear_selection();
    let circle_id = multiple.objects().next().unwrap().id();
    multiple
        .select_object(circle_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut multiple, "CrvSeam Parameter=99")
        .unwrap();
    assert_eq!(
        *multiple
            .object(circle_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain()
            .start(),
        99.0
    );
    registry.execute(&mut multiple, "Undo").unwrap();
    assert_eq!(
        multiple.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(multiple.undo_label(), history.as_deref());
}
