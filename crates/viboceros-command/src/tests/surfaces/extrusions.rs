//! surfaces extrusions command regressions.
use super::*;

#[test]
fn extrude_curve_along_curve_matches_rhino_sum_surface_and_path_retention() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let output_layer = document.current_layer_id();
    let input_layer = document
        .add_layer("Profiles", ColorRgb::new(91, 92, 93))
        .unwrap();
    let line = LineSegment::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let rational_profile = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(4.0, 1.0, 0.0).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(3.0, 1.0, 0.0).unwrap(), 1.0).unwrap(),
        ],
        vec![3.0, 3.0, 3.0, 8.0, 8.0, 8.0],
    )
    .unwrap();
    let path = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(11.0, 2.0, 1.0).unwrap(), 0.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(12.0, 3.0, 4.0).unwrap(), 1.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
    )
    .unwrap();
    let source_ids = [
        document
            .add_geometry_with_attributes(
                Geometry::Line(line),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("line")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(rational_profile.clone()),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("rational")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(input_layer).with_name("point"),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(path.clone()),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("Rail")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
    ];
    let group = document
        .add_group(Some("Along-curve inputs".to_owned()), source_ids)
        .unwrap();
    document
        .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
        .unwrap();

    registry
            .execute(
                &mut document,
                "ExtrudeCrvAlongCrv PathName=Rail Output=Surface Solid=No DeleteInput=No SplitAtTangents=No",
            )
            .unwrap();
    assert_eq!(document.objects().len(), 6);
    assert_eq!(document.group(group).unwrap().members().len(), 4);
    assert!(source_ids[..3].iter().all(|id| document.is_selected(*id)));
    assert!(!document.is_selected(source_ids[3]));
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in &outputs {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
    }
    let surfaces = outputs
        .iter()
        .map(|object| match object.geometry() {
            Geometry::NurbsSurface(surface) => surface,
            _ => panic!("ExtrudeCrvAlongCrv must create NURBS surfaces"),
        })
        .collect::<Vec<_>>();
    let line_surface = surfaces
        .iter()
        .find(|surface| surface.control_point_count_u() == 2)
        .unwrap();
    assert_eq!(line_surface.degree_u(), 1);
    assert_eq!(line_surface.degree_v(), path.degree());
    assert_eq!(line_surface.knots_u(), &[0.0, 0.0, 2.0, 2.0]);
    assert_eq!(line_surface.knots_v(), path.knots());
    assert_eq!(
        line_surface.control_point(0, 0).unwrap().point(),
        Point3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        line_surface.control_point(1, 1).unwrap().point(),
        Point3::try_new(1.0, 4.0, 1.0).unwrap()
    );
    assert_eq!(line_surface.control_point(1, 1).unwrap().weight(), 0.5);
    let rational_surface = surfaces
        .iter()
        .find(|surface| surface.control_point_count_u() == 3)
        .unwrap();
    assert_eq!(rational_surface.knots_u(), rational_profile.knots());
    assert_eq!(rational_surface.knots_v(), path.knots());
    assert_eq!(
        rational_surface.control_point(1, 1).unwrap().point(),
        Point3::try_new(5.0, 3.0, 1.0).unwrap()
    );
    assert_eq!(
        rational_surface.control_point(1, 1).unwrap().weight(),
        0.375
    );
    assert_eq!(document.undo_label(), Some("ExtrudeCrvAlongCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
        .unwrap();
    document
        .select_objects_direct([source_ids[3]], SelectionMode::Add)
        .unwrap();
    registry
        .execute(&mut document, "ExtrudeCrvAlongCrv DeleteInput=Yes")
        .unwrap();
    assert_eq!(document.objects().len(), 4);
    let remaining_group_members = document.group(group).unwrap().members().collect::<Vec<_>>();
    assert_eq!(remaining_group_members.len(), 2);
    assert!(remaining_group_members.contains(&source_ids[2]));
    assert!(remaining_group_members.contains(&source_ids[3]));
    assert!(document.object(source_ids[0]).is_none());
    assert!(document.object(source_ids[1]).is_none());
    assert!(document.is_selected(source_ids[2]));
    assert!(!document.is_selected(source_ids[3]));
    assert!(
        document
            .objects()
            .filter(|object| ![source_ids[2], source_ids[3]].contains(&object.id()))
            .all(|object| !document.is_selected(object.id()))
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids[..3].iter().copied(), SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ExtrudeCrvAlongCrv PathName=Missing",
        "ExtrudeCrvAlongCrv PathName=point",
        "ExtrudeCrvAlongCrv PathName=Rail DeleteInput=Maybe",
        "ExtrudeCrvAlongCrv PathName=Rail Output=SubD",
        "ExtrudeCrvAlongCrv PathName=Rail Solid=Yes Solid=No",
        "ExtrudeCrvAlongCrv PathName=Rail SplitAtTangents=Maybe",
        "ExtrudeCrvAlongCrv PathName=Rail SplitAtTangents=Yes SplitAtTangents=No",
        "ExtrudeCrvAlongCrv PathName=Rail extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.group(group).unwrap().members().len(), 4);
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_objects_direct([source_ids[3]], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtrudeCrvAlongCrv"),
        Err(CommandError::CurveAlongCurveProfilesRequired)
    ));
    document
        .select_objects_direct([source_ids[2]], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtrudeCrvAlongCrv PathName=Rail"),
        Err(CommandError::NoCurveAlongCurveProfiles)
    ));
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn extrude_curve_along_curve_solid_caps_closed_planar_profiles() {
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
    let path = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(30.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(31.0, 4.0, 2.0).unwrap(), 0.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(32.0, 3.0, 5.0).unwrap(), 1.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
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
    let path_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(path.clone()),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("SolidRail"),
        )
        .unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtrudeCrvAlongCrv PathName=SolidRail Solid=Yes DeleteInput=No",
            )
            .unwrap(),
        "Extruded 4 curve object(s) along the path"
    );
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()) && object.id() != path_id)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    let Geometry::Brep(rectangle_solid) = outputs[0].geometry() else {
        panic!("closed rectangle must produce a capped path B-rep")
    };
    assert!(rectangle_solid.is_solid());
    assert!((rectangle_solid.signed_volume(tolerance).unwrap() - 30.0).abs() < 1.0e-10);
    assert_eq!(rectangle_solid.faces()[0].surface().knots_v(), path.knots());
    let rectangle_display = rectangle_solid.tessellate(2, tolerance).unwrap();
    assert!(rectangle_display.topology().is_solid());
    assert!((rectangle_display.signed_volume().unwrap() - 30.0).abs() < 1.0e-10);
    let Geometry::Brep(circle_solid) = outputs[1].geometry() else {
        panic!("circle must produce a capped path B-rep")
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
    assert!(matches!(outputs[2].geometry(), Geometry::NurbsSurface(_)));
    assert!(
        matches!(outputs[3].geometry(), Geometry::Brep(b) if !b.is_solid() && b.faces().len()==4)
    );
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    assert!(!document.is_selected(path_id));
    assert_eq!(document.undo_label(), Some("ExtrudeCrvAlongCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 5);
    assert!(source_ids.iter().all(|id| document.object(*id).is_some()));
    assert!(document.object(path_id).is_some());
}

#[test]
fn extrude_curve_to_point_matches_rhino_nurbs_form_and_document_behavior() {
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
    let rational_curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(12.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(13.0, 2.0, 0.0).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(15.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
    )
    .unwrap();
    let source_ids = [
        document
            .add_geometry_with_attributes(
                Geometry::Line(line),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("line")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(rational_curve.clone()),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("rational")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(input_layer).with_name("point"),
            )
            .unwrap(),
    ];
    let group = document
        .add_group(Some("Apex profiles".to_owned()), source_ids)
        .unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();

    registry
        .execute(
            &mut document,
            "ExtrudeCrvToPoint 1,2,5 Output=Surface Solid=No",
        )
        .unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.group(group).unwrap().members().len(), 3);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in &outputs {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
    }
    let surfaces = outputs
        .iter()
        .map(|object| match object.geometry() {
            Geometry::NurbsSurface(surface) => surface,
            _ => panic!("ExtrudeCrvToPoint must create NURBS surfaces"),
        })
        .collect::<Vec<_>>();
    let apex = Point3::try_new(1.0, 2.0, 5.0).unwrap();
    let line_apex_distance = Point3::try_new(0.0, 0.0, 0.0)
        .unwrap()
        .distance_to(apex)
        .unwrap();
    assert_eq!(surfaces[0].degree_u(), 1);
    assert_eq!(surfaces[0].degree_v(), 1);
    assert_eq!(surfaces[0].control_point_count_u(), 2);
    assert_eq!(surfaces[0].control_point_count_v(), 2);
    assert_eq!(
        surfaces[0].knots_u(),
        &[0.0, 0.0, line_apex_distance, line_apex_distance]
    );
    assert_eq!(surfaces[0].knots_v(), &[0.0, 0.0, 4.0, 4.0]);
    assert_eq!(
        surfaces[0].evaluate(0.0, 4.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(surfaces[0].evaluate(line_apex_distance, 2.0).unwrap(), apex);
    assert_eq!(surfaces[1].degree_u(), 1);
    assert_eq!(surfaces[1].degree_v(), rational_curve.degree());
    assert_eq!(surfaces[1].knots_v(), rational_curve.knots());
    assert_eq!(surfaces[1].domain_v(), rational_curve.domain());
    for (index, control) in rational_curve.control_points().iter().enumerate() {
        assert_eq!(surfaces[1].control_point(0, index), Some(*control));
        let apex_control = surfaces[1].control_point(1, index).unwrap();
        assert_eq!(apex_control.point(), apex);
        assert_eq!(apex_control.weight(), control.weight());
    }
    assert_eq!(document.undo_label(), Some("ExtrudeCrvToPoint"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "ExtrudeCrvToPoint 1 2 5 DeleteInput=Yes")
        .unwrap();
    assert_eq!(document.objects().len(), 3);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source_ids[2]]
    );
    assert!(document.object(source_ids[0]).is_none());
    assert!(document.object(source_ids[1]).is_none());
    assert!(document.is_selected(source_ids[2]));
    assert!(
        document
            .objects()
            .filter(|object| object.id() != source_ids[2])
            .all(|object| !document.is_selected(object.id()))
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ExtrudeCrvToPoint",
        "ExtrudeCrvToPoint 0,0,0",
        "ExtrudeCrvToPoint 1,2,5 DeleteInput=Maybe",
        "ExtrudeCrvToPoint 1,2,5 DeleteInput=Yes DeleteInput=No",
        "ExtrudeCrvToPoint 1,2,5 Output=SubD",
        "ExtrudeCrvToPoint 1,2,5 Solid=Yes Solid=No",
        "ExtrudeCrvToPoint 1,2,5 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.group(group).unwrap().members().len(), 3);
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_objects_direct([source_ids[2]], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "ExtrudeCrvToPoint 1,2,5"),
        Err(CommandError::NoExtrudableCurves)
    ));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn extrude_curve_to_point_solid_caps_closed_planar_profiles() {
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
            .execute(
                &mut document,
                "ExtrudeCrvToPoint 1,2,6 Solid=Yes DeleteInput=No",
            )
            .unwrap(),
        "Extruded 4 curve object(s) to 2 exact capped B-rep solid(s) and 2 exact NURBS apex surface(s)"
    );
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    let Geometry::Brep(rectangle_solid) = outputs[0].geometry() else {
        panic!("closed rectangle must produce a capped apex B-rep")
    };
    assert!(rectangle_solid.is_solid());
    assert!((rectangle_solid.signed_volume(tolerance).unwrap() - 12.0).abs() < 1.0e-11);
    let rectangle_display = rectangle_solid.tessellate(1, tolerance).unwrap();
    assert!(rectangle_display.topology().is_solid());
    assert!((rectangle_display.signed_volume().unwrap() - 12.0).abs() < 1.0e-11);
    let Geometry::Brep(circle_solid) = outputs[1].geometry() else {
        panic!("circle must produce a capped apex B-rep")
    };
    assert!(circle_solid.is_solid());
    let expected_circle_volume = std::f64::consts::PI * 4.0 * 6.0 / 3.0;
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
    assert!(matches!(outputs[2].geometry(), Geometry::NurbsSurface(_)));
    assert!(matches!(outputs[3].geometry(), Geometry::NurbsSurface(_)));
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    assert_eq!(document.undo_label(), Some("ExtrudeCrvToPoint"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert!(source_ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn revolve_matches_rhino_exact_surface_attributes_groups_and_delete_input() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let output_layer = document.current_layer_id();
    let input_layer = document
        .add_layer("Profiles", ColorRgb::new(91, 92, 93))
        .unwrap();
    let line = LineSegment::try_new(
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 3.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let rational_curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 1.5).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(4.0, 0.0, 3.0).unwrap(), 1.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
    )
    .unwrap();
    let source_ids = [
        document
            .add_geometry_with_attributes(
                Geometry::Line(line),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("line")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(rational_curve.clone()),
                ObjectAttributes::on_layer(input_layer)
                    .with_name("rational")
                    .with_object_color(ColorRgb::new(12, 34, 56)),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
                ObjectAttributes::on_layer(input_layer).with_name("point"),
            )
            .unwrap(),
    ];
    let group = document
        .add_group(Some("Revolve profiles".to_owned()), source_ids)
        .unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();

    registry
        .execute(
            &mut document,
            "Revolve 0,0,0 0,0,1 FullCircle=Yes Output=Surface Deformable=No SplitAtTangents=No",
        )
        .unwrap();
    assert_eq!(document.objects().len(), 5);
    assert_eq!(document.group(group).unwrap().members().len(), 3);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in &outputs {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(output.attributes().object_color(), ColorRgb::BLACK);
    }
    let surfaces = outputs
        .iter()
        .map(|object| match object.geometry() {
            Geometry::NurbsSurface(surface) => surface,
            _ => panic!("Revolve must create NURBS surfaces"),
        })
        .collect::<Vec<_>>();
    assert_eq!(surfaces[0].degree_u(), 2);
    assert_eq!(surfaces[0].degree_v(), 1);
    assert_eq!(surfaces[0].control_point_count_u(), 9);
    assert_eq!(surfaces[0].domain_u(), 0.0..=4.0 * std::f64::consts::PI);
    assert_eq!(surfaces[0].domain_v(), 0.0..=3.0);
    assert_eq!(
        surfaces[0].control_point(8, 0),
        surfaces[0].control_point(0, 0)
    );
    assert_eq!(surfaces[1].degree_v(), rational_curve.degree());
    assert_eq!(surfaces[1].knots_v(), rational_curve.knots());
    assert_eq!(surfaces[1].domain_u(), 0.0..=8.0 * std::f64::consts::PI);
    assert_eq!(
        surfaces[1].control_point(1, 1).unwrap().weight(),
        0.75 * 0.5_f64.sqrt()
    );
    assert_eq!(document.undo_label(), Some("Revolve"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "Revolve 0 0 0 0 0 1 120 StartAngle=30 DeleteInput=Yes SplitAtTangents=Yes",
        )
        .unwrap();
    assert_eq!(document.objects().len(), 3);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source_ids[2]]
    );
    assert!(document.object(source_ids[0]).is_none());
    assert!(document.object(source_ids[1]).is_none());
    assert!(document.is_selected(source_ids[2]));
    let partial_outputs = document
        .objects()
        .filter(|object| object.id() != source_ids[2])
        .collect::<Vec<_>>();
    assert_eq!(partial_outputs.len(), 2);
    assert!(
        partial_outputs
            .iter()
            .all(|object| !document.is_selected(object.id()))
    );
    let Geometry::NurbsSurface(partial_line) = partial_outputs[0].geometry() else {
        panic!("expected a partially revolved line surface")
    };
    assert_eq!(partial_line.control_point_count_u(), 5);
    assert_eq!(partial_line.domain_u(), 0.0..=2.0 * 120.0_f64.to_radians());
    assert!(partial_line.evaluate(0.0, 0.0).unwrap().is_near(
        Point3::try_new(3.0_f64.sqrt(), 1.0, 0.0).unwrap(),
        tolerance
    ));
    assert!(
        partial_line
            .evaluate(*partial_line.domain_u().end(), 3.0)
            .unwrap()
            .is_near(
                Point3::try_new(-3.0_f64.sqrt(), 1.0, 3.0).unwrap(),
                tolerance
            )
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Revolve",
        "Revolve 0,0,0 0,0,0 90",
        "Revolve 0,0,0 0,0,1 0",
        "Revolve 0,0,0 0,0,1 361",
        "Revolve 0,0,0 0,0,1 90 FullCircle=Yes",
        "Revolve 0,0,0 0,0,1 FullCircle=Maybe",
        "Revolve 0,0,0 0,0,1 90 DeleteInput=Yes DeleteInput=No",
        "Revolve 0,0,0 0,0,1 90 Output=SubD",
        "Revolve 0,0,0 0,0,1 90 Deformable=Yes",
        "Revolve 0,0,0 0,0,1 90 SplitAtTangents=Maybe",
        "Revolve 0,0,0 0,0,1 90 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.group(group).unwrap().members().len(), 3);
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_objects_direct([source_ids[2]], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Revolve 0,0,0 0,0,1 90"),
        Err(CommandError::NoRevolvableCurves)
    ));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.undo_label(), history.as_deref());
}
