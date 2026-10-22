use super::*;

#[test]
fn extrude_curve_matches_rhino_domains_attributes_groups_and_delete_input() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let output_layer = document.current_layer_id();
    let input_layer = document
        .add_layer("Profiles", ColorRgb::new(91, 92, 93))
        .unwrap();
    let line = LineSegment::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let polyline = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(2.0, 1.0, 0.0).unwrap(),
            Point3::try_new(2.0, 4.0, 0.0).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(8.0, 0.0, 0.0).unwrap(),
        2.0,
        UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
        tolerance,
    )
    .unwrap();
    let existing_nurbs = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(12.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(13.0, 2.0, 0.0).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(15.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
    )
    .unwrap();
    let geometries = [
        ("line", Geometry::Line(line)),
        ("polyline", Geometry::Polyline(polyline.clone())),
        ("circle", Geometry::Circle(circle)),
        ("nurbs", Geometry::NurbsCurve(existing_nurbs.clone())),
        (
            "point",
            Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
        ),
    ];
    let source_ids = geometries
        .into_iter()
        .map(|(name, geometry)| {
            document
                .add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(input_layer)
                        .with_name(name)
                        .with_object_color(ColorRgb::new(12, 34, 56)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let group = document
        .add_group(
            Some("Extrusion profiles".to_owned()),
            source_ids.iter().copied(),
        )
        .unwrap();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "ExtrudeCrv 5 Output=Surface Solid=No")
        .unwrap();
    assert_eq!(document.objects().len(), 9);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(document.group(group).unwrap().members().len(), 5);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    for output in &outputs {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
    }
    let surfaces = outputs
        .iter()
        .map(|object| {
            object
                .geometry()
                .nurbs_surface_representation()
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(surfaces[0].knots_u(), &[0.0, 0.0, 4.0, 4.0]);
    assert_eq!(surfaces[0].knots_v(), &[0.0, 0.0, 5.0, 5.0]);
    assert_eq!(
        surfaces[0].evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        surfaces[0].evaluate(4.0, 5.0).unwrap(),
        Point3::try_new(4.0, 0.0, 5.0).unwrap()
    );
    // Rhino ExtrudeCrv and ToNURBS retain native polyline parameters.
    // See curve_native_extrusion.json and nurbs_conversion.json.
    assert_eq!(surfaces[1].knots_u(), &[0.0, 0.0, 1.0, 2.0, 2.0]);
    assert_eq!(surfaces[2].domain_u(), 0.0..=circle.length().unwrap());
    assert!(surfaces[2].is_rational());
    assert_eq!(surfaces[3].knots_u(), existing_nurbs.knots());
    assert_eq!(surfaces[3].domain_u(), 2.0..=7.0);
    assert_eq!(document.undo_label(), Some("ExtrudeCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.group(group).unwrap().members().len(), 5);
    registry
        .execute(
            &mut document,
            "ExtrudeCrv 0,0,0 0,3,4 BothSides=Yes DeleteInput=Yes",
        )
        .unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source_ids[4]]
    );
    assert!(
        source_ids[..4]
            .iter()
            .all(|id| document.object(*id).is_none())
    );
    assert!(document.is_selected(source_ids[4]));
    let both_sides_outputs = document
        .objects()
        .filter(|object| object.id() != source_ids[4])
        .collect::<Vec<_>>();
    assert_eq!(both_sides_outputs.len(), 4);
    assert!(
        both_sides_outputs
            .iter()
            .all(|object| !document.is_selected(object.id()))
    );
    let both_sides_line = both_sides_outputs[0]
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    assert_eq!(both_sides_line.domain_v(), 0.0..=10.0);
    assert_eq!(
        both_sides_line.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(0.0, -3.0, -4.0).unwrap()
    );
    assert!(
        both_sides_line
            .evaluate(0., 10.)
            .unwrap()
            .distance_to(Point3::try_new(0., 3., 4.).unwrap())
            .unwrap()
            < 1e-12
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.group(group).unwrap().members().len(), 5);
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ExtrudeCrv 0",
        "ExtrudeCrv 0,0,0 0,0,0",
        "ExtrudeCrv 5 BothSides=Yes BothSides=No",
        "ExtrudeCrv 5 DeleteInput=Maybe",
        "ExtrudeCrv 5 Output=SubD",
        "ExtrudeCrv 5 Solid=Yes Solid=No",
        "ExtrudeCrv 5 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.group(group).unwrap().members().len(), 5);
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_objects_direct([source_ids[4]], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtrudeCrv 5"),
        Err(CommandError::NoExtrudableCurves)
    ));
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn extrude_curve_solid_caps_closed_planar_profiles_and_leaves_open_profiles_as_surfaces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let rectangle = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(8.0, 0.0, 0.0).unwrap(),
        2.0,
        UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
        tolerance,
    )
    .unwrap();
    let line = LineSegment::try_new(
        Point3::try_new(12.0, 0.0, 0.0).unwrap(),
        Point3::try_new(15.0, 0.0, 0.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let nonplanar = Polyline3::try_new(
        vec![
            Point3::try_new(18.0, 0.0, 0.0).unwrap(),
            Point3::try_new(20.0, 0.0, 0.0).unwrap(),
            Point3::try_new(20.0, 3.0, 0.0).unwrap(),
            Point3::try_new(18.0, 3.0, 1.0).unwrap(),
            Point3::try_new(18.0, 0.0, 0.0).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let source_ids = [
        document
            .add_geometry(Geometry::Polyline(rectangle))
            .unwrap(),
        document.add_geometry(Geometry::Circle(circle)).unwrap(),
        document.add_geometry(Geometry::Line(line)).unwrap(),
        document
            .add_geometry(Geometry::Polyline(nonplanar))
            .unwrap(),
    ];
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "ExtrudeCrv 5 Solid=Yes")
            .unwrap(),
        "Extruded 4 curve object(s) into 2 exact capped solid(s) and 2 exact surface(s)"
    );
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    let Geometry::Brep(rectangle_solid) = outputs[0].geometry() else {
        panic!("closed rectangle must produce a capped B-rep")
    };
    assert!(rectangle_solid.is_solid());
    assert!((rectangle_solid.signed_volume(tolerance).unwrap() - 30.0).abs() < 1.0e-11);
    let Geometry::Brep(circle_solid) = outputs[1].geometry() else {
        panic!("circle must produce a capped B-rep")
    };
    assert!(circle_solid.is_solid());
    let expected_circle_volume = std::f64::consts::PI * 4.0 * 5.0;
    assert!(
        (circle_solid.signed_volume(tolerance).unwrap() - expected_circle_volume).abs()
            / expected_circle_volume
            < 2.0e-12
    );
    let circle_display = circle_solid.tessellate(8, tolerance).unwrap();
    assert!(circle_display.topology().is_solid());
    assert!(
        (circle_display.signed_volume().unwrap() - expected_circle_volume).abs()
            / expected_circle_volume
            < 0.01
    );
    assert!(matches!(outputs[2].geometry(), Geometry::Brep(b) if b.is_extrusion()&&!b.is_solid()));
    assert!(matches!(outputs[3].geometry(), Geometry::Brep(b) if !b.is_extrusion()&&!b.is_solid()));
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    assert_eq!(document.undo_label(), Some("ExtrudeCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert!(source_ids.iter().all(|id| document.object(*id).is_some()));
}
