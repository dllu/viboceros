//! refinement insert control command regressions.
use super::*;

#[test]
fn insert_control_point_updates_a_rational_curve_in_place_and_undoes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document
        .add_layer("Control insertion", ColorRgb::new(31, 89, 147))
        .unwrap();
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([-1.0, 0.0, 0.0], 0.7),
            ([2.0, 5.0, 1.0], 1.6),
            ([6.0, -2.0, 0.0], 0.8),
            ([9.0, 4.0, -1.0], 1.3),
            ([12.0, 0.0, 2.0], 0.9),
        ]
        .into_iter()
        .map(|(point, weight)| {
            WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source.clone()),
            ObjectAttributes::on_layer(layer).with_name("rational control source"),
        )
        .unwrap();
    let unrelated_id = document
        .add_geometry(Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .select_objects([source_id, unrelated_id], SelectionMode::Replace)
        .unwrap();
    let pick = source.evaluate(2.0).unwrap();
    let coordinates = pick.to_array();
    let command = format!(
        "InsertControlPoint {:.17},{:.17},{:.17}",
        coordinates[0], coordinates[1], coordinates[2]
    );
    let parameter = source
        .closest_parameter(pick, document.tolerance())
        .unwrap();
    let expected = source.try_insert_control_point(parameter, false).unwrap();

    assert_eq!(
        registry.execute(&mut document, &command).unwrap(),
        "InsertControlPoint: 1 edit(s)"
    );
    assert_eq!(document.objects().len(), 2);
    let object = document.object(source_id).unwrap();
    assert_eq!(object.attributes().layer_id(), layer);
    assert_eq!(object.attributes().name(), Some("rational control source"));
    assert!(document.is_selected(source_id));
    assert!(document.is_selected(unrelated_id));
    let Geometry::NurbsCurve(inserted) = object.geometry() else {
        panic!("InsertControlPoint must retain NURBS curve geometry")
    };
    assert_eq!(inserted, &expected);
    assert_eq!(inserted.control_points()[3].weight(), 1.0);
    assert_eq!(document.undo_label(), Some("InsertControlPoint"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source)
    );
}

#[test]
fn insert_control_point_maps_rhino_surface_row_directions_and_midpoints() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let controls = (0..4)
        .flat_map(|v| {
            (0..5).map(move |u| {
                Point3::try_new(
                    [0.0, 2.0, 5.0, 8.0, 11.0][u],
                    [0.0, 3.0, 7.0, 10.0][v],
                    (u * v) as Real,
                )
                .unwrap()
            })
        })
        .collect();
    let source = NurbsSurface::try_new(
        2,
        2,
        5,
        4,
        controls,
        vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
        vec![-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(source.clone()))
        .unwrap();
    document
        .select_objects([source_id], SelectionMode::Replace)
        .unwrap();
    let pick = source.evaluate(3.0, 2.0).unwrap();
    let coordinates = pick.to_array();
    let point = format!(
        "{:.17},{:.17},{:.17}",
        coordinates[0], coordinates[1], coordinates[2]
    );
    let (u, v) = source
        .closest_parameters(pick, document.tolerance())
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("InsertControlPoint {point} Direction _U"),
            )
            .unwrap(),
        "InsertControlPoint: 1 edit(s)"
    );
    let Geometry::NurbsSurface(inserted_u_row) = document.object(source_id).unwrap().geometry()
    else {
        panic!("InsertControlPoint must retain NURBS surface geometry")
    };
    assert_eq!(inserted_u_row.control_point_count_u(), 5);
    assert_eq!(inserted_u_row.control_point_count_v(), 5);
    assert_eq!(
        inserted_u_row,
        &source
            .try_insert_control_point(SurfaceKnotDirection::V, u, v, false)
            .unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                &format!("_InsertControlPoint {point} Direction=V Midpoint=_Yes"),
            )
            .unwrap(),
        "InsertControlPoint: 1 edit(s)"
    );
    let Geometry::NurbsSurface(inserted_v_row) = document.object(source_id).unwrap().geometry()
    else {
        panic!("InsertControlPoint must retain NURBS surface geometry")
    };
    assert_eq!(
        (
            inserted_v_row.control_point_count_u(),
            inserted_v_row.control_point_count_v()
        ),
        (6, 4)
    );
    assert_eq!(
        inserted_v_row,
        &source
            .try_insert_command_control_row(SurfaceKnotDirection::U, u, v, true)
            .unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsSurface(source)
    );
}

#[test]
fn insert_control_point_supports_periodic_and_analytic_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let periodic = periodic_cubic_curve();
    let periodic_id = document
        .add_geometry(Geometry::NurbsCurve(periodic.clone()))
        .unwrap();
    document
        .select_objects([periodic_id], SelectionMode::Replace)
        .unwrap();
    let pick = periodic.evaluate(3.4).unwrap().to_array();
    registry
        .execute(
            &mut document,
            &format!(
                "InsertControlPoint {:.17},{:.17},{:.17}",
                pick[0], pick[1], pick[2]
            ),
        )
        .unwrap();
    let Geometry::NurbsCurve(inserted) = document.object(periodic_id).unwrap().geometry() else {
        panic!("InsertControlPoint must retain periodic NURBS geometry")
    };
    assert!(inserted.is_periodic());
    assert_eq!(inserted.control_points().len(), 8);

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
    registry
        .execute(&mut document, "InsertControlPoint 2.5,0,0")
        .unwrap();
    let Geometry::NurbsCurve(line) = document.object(line_id).unwrap().geometry() else {
        panic!("InsertControlPoint must convert an analytic line to NURBS")
    };
    assert_eq!(line.degree(), 1);
    assert_eq!(line.control_points().len(), 3);
    assert_eq!(line.knots(), &[0.0, 0.0, 5.0, 10.0, 10.0]);
}

#[test]
fn insert_control_point_rejects_invalid_requests_atomically() {
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
    let first_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    let second_id = document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();

    assert!(matches!(
        registry.execute(&mut document, "InsertControlPoint 1,1,0"),
        Err(CommandError::InsertControlPointRequiresOneObject { actual: 0 })
    ));
    document
        .select_objects([first_id, second_id], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "InsertControlPoint 1,1,0"),
        Err(CommandError::InsertControlPointRequiresOneObject { actual: 2 })
    ));

    document
        .select_objects([first_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "InsertControlPoint",
        "InsertControlPoint 0,0,0,0",
        "InsertControlPoint NaN,0,0",
        "InsertControlPoint 1,1,0 Direction=Other",
        "InsertControlPoint 1,1,0 Direction=Both",
        "InsertControlPoint 1,1,0 Direction=U Direction=V",
        "InsertControlPoint 1,1,0 Midpoint=Maybe",
        "InsertControlPoint 1,1,0 Midpoint=No Midpoint=Yes",
        "InsertControlPoint 1,1,0 Extra=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(
            document.object(first_id).unwrap().geometry(),
            &Geometry::NurbsCurve(source.clone()),
            "{command}"
        );
        assert_eq!(document.undo_label(), history.as_deref(), "{command}");
    }

    assert_eq!(
        registry
            .execute(&mut document, "InsertControlPoint 0,0,0")
            .unwrap(),
        "InsertControlPoint: no edits"
    );
    assert_eq!(
        document.object(first_id).unwrap().geometry(),
        &Geometry::NurbsCurve(source.clone())
    );
    assert_eq!(document.undo_label(), history.as_deref());

    registry
        .execute(
            &mut document,
            "InsertControlPoint 0 0 0 Direction _V Midpoint _Yes",
        )
        .unwrap();
    let Geometry::NurbsCurve(inserted) = document.object(first_id).unwrap().geometry() else {
        panic!("InsertControlPoint midpoint should edit the NURBS curve")
    };
    assert_eq!(inserted.control_points().len(), 5);
}
