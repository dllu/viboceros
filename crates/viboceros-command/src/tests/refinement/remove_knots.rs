//! refinement remove knots command regressions.
use super::*;

#[test]
fn remove_knot_updates_curves_and_surfaces_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Knot removal", ColorRgb::new(37, 91, 145))
        .unwrap();
    let source_curve = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 4.0, 1.0).unwrap(),
            Point3::try_new(5.0, -1.0, 2.0).unwrap(),
            Point3::try_new(7.0, 3.0, -1.0).unwrap(),
            Point3::try_new(9.0, 1.0, 0.0).unwrap(),
            Point3::try_new(12.0, 5.0, 2.0).unwrap(),
            Point3::try_new(15.0, -2.0, 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
    )
    .unwrap();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("removable curve"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "RemoveKnot 2").unwrap(),
        "Removed one knot from selected NURBS curve"
    );
    let object = document.object(curve_id).unwrap();
    let Geometry::NurbsCurve(curve) = object.geometry() else {
        panic!("RemoveKnot must retain NURBS curve geometry")
    };
    assert_eq!(
        curve.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0]
    );
    assert_eq!(object.attributes().layer_id(), layer);
    assert_eq!(object.attributes().name(), Some("removable curve"));
    assert!(document.is_selected(curve_id));
    assert!(document.is_selected(point_id));
    assert_eq!(document.undo_label(), Some("RemoveKnot"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );

    let source_surface = rational_multi_span_surface();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(source_surface.clone()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "_RemoveKnot 0.5,11.3 Direction _V")
            .unwrap(),
        "Removed one knot from selected NURBS surface V direction"
    );
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("RemoveKnot must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_point_count_v(), 3);
    assert_eq!(surface.knots_v(), &[10.0, 10.0, 10.0, 14.0, 14.0, 14.0]);
    assert_eq!(surface.knots_u(), source_surface.knots_u());
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface)
    );
}

#[test]
fn remove_knot_rejects_invalid_or_ineligible_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(5.0, -1.0, 0.0).unwrap(),
            Point3::try_new(8.0, 2.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    let second_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();

    assert!(matches!(
        registry.execute(&mut document, "RemoveKnot 0.4"),
        Err(CommandError::RemoveKnotRequiresOneObject { actual: 0 })
    ));
    document
        .select_objects([curve_id, second_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveKnot 0.4"),
        Err(CommandError::RemoveKnotRequiresOneObject { actual: 2 })
    ));

    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "RemoveKnot",
        "RemoveKnot 0.4,0.5",
        "RemoveKnot nope",
        "RemoveKnot NaN",
        "RemoveKnot 0.4 Direction=Both",
        "RemoveKnot 0.4 Direction=U Direction=V",
        "RemoveKnot 0.4 Extra=Yes",
        "RemoveKnot 0",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(curve_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    let periodic_id = document
        .add_geometry(Geometry::NurbsCurve(periodic_cubic_curve()))
        .unwrap();
    document
        .select_objects([periodic_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveKnot 4"),
        Err(CommandError::Geometry(
            GeometryError::PeriodicKnotRemovalUnsupported { .. }
        ))
    ));
}

#[test]
fn remove_control_point_updates_curves_and_surface_rows_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Control removal", ColorRgb::new(43, 97, 151))
        .unwrap();
    let source_curve = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 4.0, 1.0).unwrap(),
            Point3::try_new(5.0, -1.0, 2.0).unwrap(),
            Point3::try_new(7.0, 3.0, -1.0).unwrap(),
            Point3::try_new(9.0, 1.0, 0.0).unwrap(),
            Point3::try_new(12.0, 5.0, 2.0).unwrap(),
            Point3::try_new(15.0, -2.0, 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
    )
    .unwrap();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("editable curve"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "RemoveControlPoint 3")
            .unwrap(),
        "Removed control point 3 from selected NURBS curve"
    );
    let object = document.object(curve_id).unwrap();
    let Geometry::NurbsCurve(curve) = object.geometry() else {
        panic!("RemoveControlPoint must retain NURBS curve geometry")
    };
    assert_eq!(curve.control_points().len(), 6);
    assert_eq!(
        curve.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0]
    );
    assert_eq!(
        curve.control_points()[3].point(),
        source_curve.control_points()[4].point()
    );
    assert_eq!(object.attributes().layer_id(), layer);
    assert_eq!(object.attributes().name(), Some("editable curve"));
    assert!(document.is_selected(curve_id));
    assert!(document.is_selected(point_id));
    assert_eq!(document.undo_label(), Some("RemoveControlPoint"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );

    let controls = (0..4)
        .flat_map(|v| {
            (0..5).map(move |u| {
                Point3::try_new(u as Real * 2.0, v as Real * 3.0, (u * v) as Real).unwrap()
            })
        })
        .collect();
    let source_surface = NurbsSurface::try_new(
        2,
        2,
        5,
        4,
        controls,
        vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
        vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(source_surface.clone()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "_RemoveControlPoint _1 Direction _V")
            .unwrap(),
        "Removed control point 1 from selected NURBS surface V direction"
    );
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("RemoveControlPoint must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_point_count_u(), 5);
    assert_eq!(surface.control_point_count_v(), 3);
    assert_eq!(surface.knots_u(), source_surface.knots_u());
    assert_eq!(surface.knots_v(), &[-2.0, -2.0, -2.0, 6.0, 6.0, 6.0]);
    for u in 0..5 {
        assert_eq!(
            surface.control_point(u, 0),
            source_surface.control_point(u, 0)
        );
        assert_eq!(
            surface.control_point(u, 1),
            source_surface.control_point(u, 2)
        );
        assert_eq!(
            surface.control_point(u, 2),
            source_surface.control_point(u, 3)
        );
    }

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface)
    );
}

#[test]
fn remove_control_point_supports_periodic_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = periodic_cubic_curve();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "RemoveControlPoint 1")
            .unwrap(),
        "Removed control point 1 from selected NURBS curve"
    );
    let Geometry::NurbsCurve(curve) = document.object(curve_id).unwrap().geometry() else {
        panic!("RemoveControlPoint must retain NURBS curve geometry")
    };
    assert!(curve.is_periodic());
    assert_eq!(curve.control_points().len(), 6);
    assert_eq!(
        curve.knots(),
        &[-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
}

#[test]
fn remove_control_point_rejects_invalid_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(5.0, -1.0, 0.0).unwrap(),
            Point3::try_new(8.0, 2.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    let second_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();

    assert!(matches!(
        registry.execute(&mut document, "RemoveControlPoint 1"),
        Err(CommandError::RemoveControlPointRequiresOneObject { actual: 0 })
    ));
    document
        .select_objects([curve_id, second_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveControlPoint 1"),
        Err(CommandError::RemoveControlPointRequiresOneObject { actual: 2 })
    ));

    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "RemoveControlPoint",
        "RemoveControlPoint nope",
        "RemoveControlPoint -1",
        "RemoveControlPoint 1.5",
        "RemoveControlPoint 1 Direction=Both",
        "RemoveControlPoint 1 Direction=U Direction=V",
        "RemoveControlPoint 1 Extra=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(curve_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }
    assert!(matches!(
        registry.execute(&mut document, "RemoveControlPoint 4"),
        Err(CommandError::Geometry(
            GeometryError::ControlPointIndexOutOfRange { .. }
        ))
    ));
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let line = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let line_id = document
        .add_geometry(Geometry::NurbsCurve(line.clone()))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveControlPoint 0"),
        Err(CommandError::Geometry(
            GeometryError::InsufficientControlPoints { .. }
        ))
    ));
    assert_eq!(
        document.object(line_id).unwrap().geometry(),
        &Geometry::NurbsCurve(line)
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn remove_multi_knot_updates_selected_curves_and_surfaces_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Multiple knots", ColorRgb::new(71, 109, 151))
        .unwrap();
    let source_curve = multiple_knot_curve();
    let source_surface = NurbsSurface::try_extruded_curve(
        &source_curve,
        Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 4.0).unwrap(),
    )
    .unwrap();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("stacked curve"),
        )
        .unwrap();
    let surface_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(source_surface.clone()),
            ObjectAttributes::on_layer(layer).with_name("stacked surface"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, surface_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "RemoveMultiKnot").unwrap(),
        "Removed 2 multiple knot(s) from 2 object(s) (1 curve(s), 1 NURBS surface(s))"
    );
    let Geometry::NurbsCurve(curve) = document.object(curve_id).unwrap().geometry() else {
        panic!("RemoveMultiKnot must retain NURBS curve geometry")
    };
    assert_eq!(curve.control_points().len(), 9);
    assert_eq!(
        curve.knots(),
        &[
            0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0
        ]
    );
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("RemoveMultiKnot must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 2);
    assert_eq!(
        document.object(curve_id).unwrap().attributes().name(),
        Some("stacked curve")
    );
    assert_eq!(
        document.object(surface_id).unwrap().attributes().name(),
        Some("stacked surface")
    );
    assert_eq!(document.undo_label(), Some("RemoveMultiKnot"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve.clone())
    );
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface.clone())
    );

    assert_eq!(
        registry
            .execute(
                &mut document,
                "_RemoveMultiKnot _RemoveFullyMultipleKnots=_Yes _MaxKinkAngle=135",
            )
            .unwrap(),
        "Removed 6 multiple knot(s) from 2 object(s) (1 curve(s), 1 NURBS surface(s))"
    );
    let Geometry::NurbsCurve(curve) = document.object(curve_id).unwrap().geometry() else {
        panic!("RemoveMultiKnot must retain NURBS curve geometry")
    };
    assert_eq!(curve.control_points().len(), 7);
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("RemoveMultiKnot must retain NURBS surface geometry")
    };
    assert_eq!(surface.control_point_count_u(), 7);
}

#[test]
fn remove_multi_knot_rejects_invalid_and_ineligible_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "RemoveMultiKnot"),
        Err(CommandError::NoMultipleKnotsRemoved)
    ));

    let source = multiple_knot_curve();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "RemoveMultiKnot RemoveFullyMultipleKnots=Maybe",
        "RemoveMultiKnot RemoveFullyMultipleKnots=No RemoveFullyMultipleKnots=Yes",
        "RemoveMultiKnot MaxKinkAngle=-1",
        "RemoveMultiKnot MaxKinkAngle=181",
        "RemoveMultiKnot MaxKinkAngle=NaN",
        "RemoveMultiKnot MaxKinkAngle=1 MaxKinkAngle=2",
        "RemoveMultiKnot Extra=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(curve_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    let simple = NurbsCurve::try_clamped_uniform(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
        ],
    )
    .unwrap();
    let simple_id = document.add_geometry(Geometry::NurbsCurve(simple)).unwrap();
    document
        .select_objects([simple_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveMultiKnot"),
        Err(CommandError::NoMultipleKnotsRemoved)
    ));

    let periodic_id = document
        .add_geometry(Geometry::NurbsCurve(periodic_cubic_curve()))
        .unwrap();
    document
        .select_objects([curve_id, periodic_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "RemoveMultiKnot"),
        Err(CommandError::Geometry(
            GeometryError::PeriodicKnotRemovalUnsupported { .. }
        ))
    ));
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn symmetrical_knot_parameter_handles_the_full_finite_range() {
    assert_eq!(mirrored_parameter(0.0, -Real::MAX, Real::MAX).unwrap(), 0.0);
    assert_eq!(
        mirrored_parameter(-Real::MAX, -Real::MAX, Real::MAX).unwrap(),
        Real::MAX
    );
    assert_eq!(
        mirrored_parameter(Real::MAX, -Real::MAX, Real::MAX).unwrap(),
        -Real::MAX
    );
}
