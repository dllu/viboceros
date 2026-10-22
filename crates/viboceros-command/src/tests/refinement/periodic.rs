//! refinement periodic command regressions.
use super::*;

#[test]
fn change_degree_updates_mixed_curves_and_surfaces_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Degree changes", ColorRgb::new(48, 96, 144))
        .unwrap();
    let source_curve = periodic_cubic_curve();
    let source_surface = periodic_cubic_u_surface();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("degree curve"),
        )
        .unwrap();
    let surface_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(source_surface.clone()),
            ObjectAttributes::on_layer(layer).with_name("degree surface"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, surface_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ChangeDegree 5,3 Deformable=No")
            .unwrap(),
        "Changed degree of 2 object(s) with Deformable=No (1 curve(s), 1 NURBS surface(s))"
    );
    let curve_object = document.object(curve_id).unwrap();
    let Geometry::NurbsCurve(curve) = curve_object.geometry() else {
        panic!("ChangeDegree must produce a NURBS curve")
    };
    assert_eq!(curve.degree(), 5);
    assert!(!curve.is_periodic());
    assert_eq!(curve.domain(), source_curve.domain());
    assert_eq!(curve_object.attributes().name(), Some("degree curve"));
    let surface_object = document.object(surface_id).unwrap();
    let Geometry::NurbsSurface(surface) = surface_object.geometry() else {
        panic!("ChangeDegree must retain NURBS surface geometry")
    };
    assert_eq!((surface.degree_u(), surface.degree_v()), (5, 3));
    assert!(!surface.is_periodic_u());
    assert_eq!(surface_object.attributes().name(), Some("degree surface"));
    assert!(document.is_selected(curve_id));
    assert!(document.is_selected(surface_id));
    assert!(document.is_selected(point_id));
    assert_eq!(document.undo_label(), Some("ChangeDegree"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface)
    );
}

#[test]
fn change_degree_parses_scalar_and_pair_targets_and_rejects_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(periodic_cubic_curve()))
        .unwrap();
    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let before = document.object(curve_id).unwrap().geometry().clone();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ChangeDegree",
        "ChangeDegree 0",
        "ChangeDegree 12",
        "ChangeDegree 3,",
        "ChangeDegree ,3",
        "ChangeDegree 2,3,4",
        "ChangeDegree nope",
        "ChangeDegree 2 Deformable=Maybe",
        "ChangeDegree 2 Deformable=Yes Deformable=No",
        "ChangeDegree 2 Extra=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.object(curve_id).unwrap().geometry(), &before);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    assert_eq!(
        registry
            .execute(&mut document, "_ChangeDegree _2 Deformable _Yes")
            .unwrap(),
        "Changed degree of 1 object(s) with Deformable=Yes (1 curve(s), 0 NURBS surface(s))"
    );
    let Geometry::NurbsCurve(curve) = document.object(curve_id).unwrap().geometry() else {
        panic!("ChangeDegree must produce a NURBS curve")
    };
    assert_eq!(curve.degree(), 2);

    assert!(matches!(
        registry.execute(&mut document, "ChangeDegree 2"),
        Err(CommandError::NoChangeDegreeObjects)
    ));
}

#[test]
fn make_periodic_smooths_closed_curves_and_surfaces_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Closed sources", ColorRgb::new(54, 108, 162))
        .unwrap();
    let source_curve = periodic_cubic_curve().try_make_non_periodic().unwrap();
    let source_surface = periodic_cubic_u_surface()
        .try_make_non_periodic(SurfaceKnotDirection::U)
        .unwrap();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("closed curve"),
        )
        .unwrap();
    let surface_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(source_surface.clone()),
            ObjectAttributes::on_layer(layer).with_name("closed surface"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, surface_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "MakePeriodic").unwrap(),
        "Made 2 object(s) periodic with Smooth=Yes, replacing the input(s) (1 curve(s), 1 NURBS surface(s))"
    );
    let curve_object = document.object(curve_id).unwrap();
    assert_eq!(curve_object.attributes().layer_id(), layer);
    assert_eq!(curve_object.attributes().name(), Some("closed curve"));
    let Geometry::NurbsCurve(curve) = curve_object.geometry() else {
        panic!("MakePeriodic must retain NURBS curve geometry")
    };
    assert!(curve.is_periodic());
    assert!(curve.is_closed().unwrap());

    let surface_object = document.object(surface_id).unwrap();
    assert_eq!(surface_object.attributes().layer_id(), layer);
    assert_eq!(surface_object.attributes().name(), Some("closed surface"));
    let Geometry::NurbsSurface(surface) = surface_object.geometry() else {
        panic!("MakePeriodic must retain NURBS surface geometry")
    };
    assert!(surface.is_periodic_u());
    assert!(!surface.is_periodic_v());
    assert!(document.is_selected(curve_id));
    assert!(document.is_selected(surface_id));
    assert!(document.is_selected(point_id));
    assert_eq!(document.undo_label(), Some("MakePeriodic"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface)
    );
}

#[test]
fn make_periodic_accepts_smooth_no_and_rejects_invalid_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(5.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "MakePeriodic"),
        Err(CommandError::NoMakePeriodicObjects)
    ));

    let source = periodic_cubic_curve().try_make_non_periodic().unwrap();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "MakePeriodic Smooth=Maybe",
        "MakePeriodic Smooth=Yes Smooth=No",
        "MakePeriodic DeleteInput=Maybe",
        "MakePeriodic DeleteInput=Yes DeleteInput=No",
        "MakePeriodic Direction=U",
        "MakePeriodic extra",
    ] {
        assert!(
            matches!(
                registry.execute(&mut document, command),
                Err(CommandError::Usage(MAKE_PERIODIC_USAGE))
            ),
            "{command}"
        );
        assert_eq!(
            document.object(curve_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    assert_eq!(
        registry
            .execute(&mut document, "_MakePeriodic Smooth _No")
            .unwrap(),
        "Made 1 object(s) periodic with Smooth=No, replacing the input(s) (1 curve(s), 0 NURBS surface(s))"
    );
    let Geometry::NurbsCurve(curve) = document.object(curve_id).unwrap().geometry() else {
        panic!("MakePeriodic must retain NURBS curve geometry")
    };
    assert!(curve.is_periodic());
    assert_eq!(document.undo_label(), Some("MakePeriodic"));
}

#[test]
fn make_periodic_converts_a_double_closed_surface_u_then_v() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let coordinates = [0.0, 2.0, 4.0, 1.0, 0.0];
    let controls = (0..5)
        .flat_map(|v| {
            (0..5).map(move |u| {
                Point3::try_new(
                    coordinates[u],
                    coordinates[v],
                    coordinates[u] * coordinates[v] * 0.1,
                )
                .unwrap()
            })
        })
        .collect();
    let source = NurbsSurface::try_new(
        2,
        2,
        5,
        5,
        controls,
        vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
        vec![-2.0, -2.0, -2.0, 1.0, 4.0, 9.0, 9.0, 9.0],
    )
    .unwrap();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_objects([surface_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "MakePeriodic Smooth=No")
        .unwrap();
    let first = document.object(surface_id).unwrap().geometry().clone();
    let Geometry::NurbsSurface(surface) = &first else {
        panic!("MakePeriodic must retain NURBS surface geometry")
    };
    assert!(surface.is_periodic_u());
    assert!(!surface.is_periodic_v());

    registry
        .execute(&mut document, "MakePeriodic Smooth=No")
        .unwrap();
    let Geometry::NurbsSurface(surface) = document.object(surface_id).unwrap().geometry() else {
        panic!("MakePeriodic must retain NURBS surface geometry")
    };
    assert!(surface.is_periodic_u());
    assert!(surface.is_periodic_v());

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(surface_id).unwrap().geometry(), &first);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source)
    );
}

#[test]
fn make_periodic_can_retain_inputs_and_copy_attributes_and_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Periodic copies", ColorRgb::new(61, 122, 183))
        .unwrap();
    let source_curve = periodic_cubic_curve().try_make_non_periodic().unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer)
                .with_name("retained curve")
                .with_object_color(ColorRgb::new(17, 83, 149)),
        )
        .unwrap();
    let group_id = document
        .add_group(Some("Periodic source".to_owned()), [source_id])
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "MakePeriodic DeleteInput _No Smooth _No",)
            .unwrap(),
        "Made 1 object(s) periodic with Smooth=No, retaining the input(s) (1 curve(s), 0 NURBS surface(s))"
    );
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );
    assert!(document.is_selected(source_id));
    let copy = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    assert!(!document.is_selected(copy.id()));
    assert_eq!(
        copy.attributes(),
        document.object(source_id).unwrap().attributes()
    );
    assert!(matches!(
        copy.geometry(),
        Geometry::NurbsCurve(curve) if curve.is_periodic()
    ));
    let members = document
        .group(group_id)
        .unwrap()
        .members()
        .collect::<Vec<_>>();
    assert_eq!(members.len(), 2);
    assert!(members.contains(&source_id));
    assert!(members.contains(&copy.id()));
    assert_eq!(document.undo_label(), Some("MakePeriodic"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.is_selected(source_id));
    assert_eq!(
        document
            .group(group_id)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source_id]
    );
}

#[test]
fn make_non_periodic_clamps_curves_and_surfaces_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Periodic sources", ColorRgb::new(52, 104, 156))
        .unwrap();
    let source_curve = periodic_cubic_curve();
    let source_surface = periodic_cubic_u_surface();
    let curve_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source_curve.clone()),
            ObjectAttributes::on_layer(layer).with_name("periodic curve"),
        )
        .unwrap();
    let surface_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(source_surface.clone()),
            ObjectAttributes::on_layer(layer).with_name("periodic surface"),
        )
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([curve_id, surface_id, point_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "MakeNonPeriodic").unwrap(),
        "Made 2 object(s) non-periodic without changing their shape (1 curve(s), 1 NURBS surface(s))"
    );
    let curve_object = document.object(curve_id).unwrap();
    assert_eq!(curve_object.attributes().layer_id(), layer);
    assert_eq!(curve_object.attributes().name(), Some("periodic curve"));
    let Geometry::NurbsCurve(curve) = curve_object.geometry() else {
        panic!("MakeNonPeriodic must retain NURBS curve geometry")
    };
    assert!(!curve.is_periodic());
    assert!(curve.is_closed().unwrap());
    for sample in 0..=32 {
        let parameter = source_curve.parameter_at(sample as Real / 32.0).unwrap();
        assert!(curve.evaluate(parameter).unwrap().is_near(
            source_curve.evaluate(parameter).unwrap(),
            Tolerance::DEFAULT
        ));
    }

    let surface_object = document.object(surface_id).unwrap();
    assert_eq!(surface_object.attributes().layer_id(), layer);
    assert_eq!(surface_object.attributes().name(), Some("periodic surface"));
    let Geometry::NurbsSurface(surface) = surface_object.geometry() else {
        panic!("MakeNonPeriodic must retain NURBS surface geometry")
    };
    assert!(!surface.is_periodic_u());
    assert!(!surface.is_periodic_v());
    for u in [2.0, 2.75, 4.0, 5.5, 6.0] {
        for v in [0.0, 1.25, 5.0] {
            assert!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .is_near(source_surface.evaluate(u, v).unwrap(), Tolerance::DEFAULT)
            );
        }
    }
    assert!(document.is_selected(curve_id));
    assert!(document.is_selected(surface_id));
    assert!(document.is_selected(point_id));
    assert_eq!(document.undo_label(), Some("MakeNonPeriodic"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source_curve)
    );
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source_surface)
    );
}

#[test]
fn make_non_periodic_rejects_ineligible_selection_and_arguments_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let line_id = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(5.0, 0.0, 0.0).unwrap(),
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([line_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "MakeNonPeriodic"),
        Err(CommandError::NoPeriodicObjects)
    ));

    let source = periodic_cubic_curve();
    let curve_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    document
        .select_objects([curve_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "MakeNonPeriodic extra"),
        Err(CommandError::Usage("MakeNonPeriodic"))
    ));
    assert_eq!(
        document.object(curve_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
