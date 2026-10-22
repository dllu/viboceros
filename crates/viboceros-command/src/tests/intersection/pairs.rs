//! intersection pairs command regressions.
use super::*;

#[test]
fn intersect_creates_current_layer_points_and_leaves_grouped_inputs_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    registry.execute(&mut document, "Line 3,-5 3,5").unwrap();
    let input_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(input_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Group Inputs").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    assert_eq!(document.objects().count(), 3);
    assert!(input_ids.iter().all(|id| !document.is_selected(*id)));
    let output = document.selected_objects().next().unwrap();
    let Geometry::Point(location) = output.geometry() else {
        panic!("curve intersections must create point objects")
    };
    assert_eq!(*location, Point3::try_new(3.0, 0.0, 0.0).unwrap());
    assert_eq!(
        output.attributes(),
        &ObjectAttributes::on_layer(document.current_layer_id())
    );
    assert!(
        !document
            .group_by_name("Inputs")
            .unwrap()
            .members()
            .any(|id| id == output.id())
    );
    assert_eq!(document.undo_label(), Some("Intersect"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 2);
}

#[test]
fn intersect_two_sets_ignores_within_set_pairs_and_assigns_output_layers() {
    for (option, expected_layer) in [
        ("", "Default"),
        (" OutputLayer=FirstSet", "First"),
        (" OutputLayer=SecondSet", "Second"),
    ] {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first_layer = document.add_layer("First", ColorRgb::BLACK).unwrap();
        let second_layer = document.add_layer("Second", ColorRgb::BLACK).unwrap();
        let first = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(0., 0., 0.).unwrap(),
                        Point3::try_new(10., 0., 0.).unwrap(),
                        document.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(first_layer),
            )
            .unwrap();
        let within = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(3., -5., 0.).unwrap(),
                        Point3::try_new(3., 5., 0.).unwrap(),
                        document.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(first_layer),
            )
            .unwrap();
        let second = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(7., -5., 0.).unwrap(),
                        Point3::try_new(7., 5., 0.).unwrap(),
                        document.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(second_layer),
            )
            .unwrap();
        let first_arg = if option == " OutputLayer=FirstSet" {
            document
                .select_objects_direct([first, within], SelectionMode::Replace)
                .unwrap();
            "Selected".to_owned()
        } else {
            format!("{first},{within}")
        };
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    &format!("IntersectTwoSets {first_arg} {second}{option}")
                )
                .unwrap(),
            "Created 1 intersection object(s) from 2 object pair(s)"
        );
        let output = document.selected_objects().next().unwrap();
        assert!(matches!(output.geometry(), Geometry::Point(point)
                if *point == Point3::try_new(7., 0., 0.).unwrap()));
        let layer = document.layer(output.attributes().layer_id()).unwrap();
        assert_eq!(layer.name(), expected_layer);
        assert_eq!(document.objects().count(), 4);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 3);
    }
}

#[test]
fn intersect_two_sets_rejects_invalid_inputs_before_editing() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    registry.execute(&mut document, "Point 5,0").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    let before = format!("{document:?}");
    for input in [
        format!("IntersectTwoSets {}", ids[0]),
        format!("IntersectTwoSets {} nope", ids[0]),
        format!("IntersectTwoSets {} {} OutputLayer=Bad", ids[0], ids[1]),
        format!("IntersectTwoSets {} {}", ids[0], ids[1]),
        format!(
            "IntersectTwoSets {} 00000000-0000-0000-0000-000000000000",
            ids[0]
        ),
    ] {
        assert!(registry.execute(&mut document, &input).is_err(), "{input}");
        assert_eq!(format!("{document:?}"), before, "{input}");
    }
    let history = document.undo_label().map(str::to_owned);
    assert_eq!(
        registry
            .execute(&mut document, "IntersectTwoSets Selected Selected")
            .unwrap(),
        "Created 0 intersection object(s) from 0 object pair(s)"
    );
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.selected_object_count(), 0);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn intersect_outputs_an_exact_overlap_with_the_later_curves_parameterization() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let second = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(7.0, 0.0, 0.0).unwrap(),
            Point3::try_new(3.0, 0.0, 0.0).unwrap(),
        ],
        vec![-2.0, -2.0, 2.0, 2.0],
    )
    .unwrap();
    let input_ids = [
        document.add_geometry(Geometry::NurbsCurve(first)).unwrap(),
        document
            .add_geometry(Geometry::NurbsCurve(second.clone()))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    registry.execute(&mut document, "Intersect").unwrap();
    let output = document.selected_objects().next().unwrap();
    let Geometry::NurbsCurve(overlap) = output.geometry() else {
        panic!("a finite curve overlap must create an exact NURBS curve")
    };
    assert_eq!(overlap, &second);
    assert!(input_ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_supports_transverse_and_overlapping_curve_surface_pairs() {
    let registry = CommandRegistry::with_builtins();
    let mut transverse = Document::default();
    let curve = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(5.0, 5.0, -5.0).unwrap(),
            Point3::try_new(5.0, 5.0, 5.0).unwrap(),
        ],
        vec![-3.0, -3.0, 7.0, 7.0],
    )
    .unwrap();
    let input_ids = [
        transverse
            .add_geometry(Geometry::NurbsCurve(curve))
            .unwrap(),
        transverse
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap(),
    ];
    transverse
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut transverse, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::Point(point) = transverse.selected_objects().next().unwrap().geometry() else {
        panic!("a transverse curve/surface contact must create a point")
    };
    assert!(point.is_near(Point3::try_new(5.0, 5.0, 0.0).unwrap(), Tolerance::DEFAULT));

    let mut overlapping = Document::default();
    let curve = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(2.0, 5.0, 0.0).unwrap(),
            Point3::try_new(8.0, 5.0, 0.0).unwrap(),
        ],
        vec![-2.0, -2.0, 4.0, 4.0],
    )
    .unwrap();
    let input_ids = [
        overlapping
            .add_geometry(Geometry::NurbsCurve(curve.clone()))
            .unwrap(),
        overlapping
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap(),
    ];
    overlapping
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut overlapping, "Intersect").unwrap();
    let Geometry::NurbsCurve(overlap) = overlapping.selected_objects().next().unwrap().geometry()
    else {
        panic!("a curve lying on a surface must create an exact curve")
    };
    assert_eq!(overlap, &curve);
    assert!(input_ids.iter().all(|id| overlapping.object(*id).is_some()));
}

#[test]
fn intersect_supports_curve_brep_points_and_exact_face_overlaps() {
    let registry = CommandRegistry::with_builtins();
    let mut crossing = Document::default();
    let curve = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(-5.0, 5.0, 5.0).unwrap(),
            Point3::try_new(15.0, 5.0, 5.0).unwrap(),
        ],
        vec![0.0, 0.0, 20.0, 20.0],
    )
    .unwrap();
    let input_ids = [
        crossing.add_geometry(Geometry::NurbsCurve(curve)).unwrap(),
        crossing
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
    ];
    crossing
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut crossing, "Intersect").unwrap(),
        "Created 2 intersection object(s) from 1 object pair(s)"
    );
    let mut points = crossing
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("a transverse curve/B-rep contact must create points"),
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    assert_eq!(
        points,
        vec![
            Point3::try_new(0.0, 5.0, 5.0).unwrap(),
            Point3::try_new(10.0, 5.0, 5.0).unwrap(),
        ]
    );

    let mut overlapping = Document::default();
    let curve = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(-5.0, 5.0, 10.0).unwrap(),
            Point3::try_new(15.0, 5.0, 10.0).unwrap(),
        ],
        vec![0.0, 0.0, 20.0, 20.0],
    )
    .unwrap();
    let input_ids = [
        overlapping
            .add_geometry(Geometry::NurbsCurve(curve))
            .unwrap(),
        overlapping
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
    ];
    overlapping
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut overlapping, "Intersect").unwrap();
    let Geometry::NurbsCurve(overlap) = overlapping.selected_objects().next().unwrap().geometry()
    else {
        panic!("a curve on a B-rep face must create an exact source subcurve")
    };
    let domain = overlap.domain();
    assert!((*domain.start() - 5.0).abs() < 1.0e-10);
    assert!((*domain.end() - 15.0).abs() < 1.0e-10);
    assert!(
        overlap
            .evaluate(*domain.start())
            .unwrap()
            .is_near(Point3::try_new(0.0, 5.0, 10.0).unwrap(), Tolerance::DEFAULT)
    );
    assert!(overlap.evaluate(*domain.end()).unwrap().is_near(
        Point3::try_new(10.0, 5.0, 10.0).unwrap(),
        Tolerance::DEFAULT
    ));
}

#[test]
fn intersect_supports_transverse_planar_surface_pairs() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_ids = [
        document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(vertical_intersection_surface(
                -5.0, 15.0,
            )))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(curve) = document.selected_objects().next().unwrap().geometry() else {
        panic!("transverse planar surfaces must create an exact line")
    };
    assert_eq!(curve.domain(), 0.0..=10.0);
    assert!(
        curve
            .evaluate(0.0)
            .unwrap()
            .is_near(Point3::try_new(0.0, 5.0, 0.0).unwrap(), Tolerance::DEFAULT)
    );
    assert!(
        curve
            .evaluate(10.0)
            .unwrap()
            .is_near(Point3::try_new(10.0, 5.0, 0.0).unwrap(), Tolerance::DEFAULT)
    );
}
