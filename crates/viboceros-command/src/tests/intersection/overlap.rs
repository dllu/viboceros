//! intersection overlap command regressions.
use super::*;

#[test]
fn intersect_outputs_coincident_bilinear_patch_boundaries() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let shifted = NurbsSurface::try_bilinear([
        Point3::try_new(5.0, 0.0, 0.0).unwrap(),
        Point3::try_new(15.0, 0.0, 0.0).unwrap(),
        Point3::try_new(15.0, 10.0, 0.0).unwrap(),
        Point3::try_new(5.0, 10.0, 0.0).unwrap(),
    ])
    .unwrap();
    let input_ids = [
        document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(shifted))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    assert!(input_ids.iter().all(|id| !document.is_selected(*id)));
    let output = document.selected_objects().next().unwrap();
    let Geometry::NurbsCurve(boundary) = output.geometry() else {
        panic!("coincident bilinear patches must create an overlap boundary")
    };
    assert_eq!(boundary.degree(), 1);
    assert_eq!(
        boundary
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            Point3::try_new(5.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 10.0, 0.0).unwrap(),
            Point3::try_new(5.0, 10.0, 0.0).unwrap(),
            Point3::try_new(5.0, 0.0, 0.0).unwrap(),
        ]
    );
    assert!(output.attributes().name().is_none());
    assert_eq!(output.attributes().layer_id(), document.current_layer_id());
}

#[test]
fn intersect_outputs_exact_bent_planar_strip_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let rectangle = NurbsSurface::try_bilinear([
        point(2.0, -1.0),
        point(8.0, -1.0),
        point(8.0, 5.0),
        point(2.0, 5.0),
    ])
    .and_then(|surface| surface.try_reparameterized(2.0..=8.0, -1.0..=5.0))
    .unwrap();
    let strip = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            point(0.0, 0.0),
            point(5.0, 2.0),
            point(10.0, 0.0),
            point(0.0, 10.0),
            point(5.0, 10.0),
            point(10.0, 10.0),
        ],
        vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let inputs = [
        document
            .add_geometry(Geometry::NurbsSurface(rectangle))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(strip))
            .unwrap(),
    ];
    document
        .select_objects_direct(inputs, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(curve) = document.selected_objects().next().unwrap().geometry() else {
        panic!("a bent planar strip overlap must produce a curve")
    };
    assert!(curve.is_closed().unwrap());
    assert_eq!(curve.degree(), 2);
    assert!((*curve.domain().start() + 0.64).abs() < 1e-10);
    assert!((*curve.domain().end() - 20.08).abs() < 1e-10);
    assert_eq!(curve.control_points().len(), 9);
    assert!(
        curve.control_points()[0]
            .point()
            .is_near(point(2.0, 0.64), Tolerance::DEFAULT)
    );
    assert!(
        curve.control_points()[1]
            .point()
            .is_near(point(5.0, 1.36), Tolerance::DEFAULT)
    );
    assert!(inputs.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_coincident_rational_patch_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let rational = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (point(0.0, 0.0), 1.0),
            (point(0.0, 10.0), 2.0),
            (point(10.0, 0.0), 5.0),
            (point(10.0, 10.0), 11.0),
        ]
        .into_iter()
        .map(|(point, weight)| viboceros_geometry::WeightedPoint3::try_new(point, weight))
        .collect::<Result<Vec<_>, _>>()
        .unwrap(),
        vec![0.0, 0.0, 10.0, 10.0],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let shifted = NurbsSurface::try_bilinear([
        point(5.0, 0.0),
        point(15.0, 0.0),
        point(15.0, 10.0),
        point(5.0, 10.0),
    ])
    .unwrap();
    let inputs = [
        document
            .add_geometry(Geometry::NurbsSurface(rational))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(shifted))
            .unwrap(),
    ];
    document
        .select_objects_direct(inputs, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(boundary) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("rational patch intersection must create an overlap boundary")
    };
    assert!(boundary.is_closed().unwrap());
    assert!((boundary.length(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1e-9);
    assert!(inputs.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_joins_planar_surface_brep_sections() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let section = NurbsSurface::try_bilinear([
        Point3::try_new(-5.0, -5.0, 5.0).unwrap(),
        Point3::try_new(15.0, -5.0, 5.0).unwrap(),
        Point3::try_new(15.0, 15.0, 5.0).unwrap(),
        Point3::try_new(-5.0, 15.0, 5.0).unwrap(),
    ])
    .unwrap();
    let input_ids = [
        document
            .add_geometry(Geometry::NurbsSurface(section))
            .unwrap(),
        document
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    assert!(input_ids.iter().all(|id| !document.is_selected(*id)));
    let output = document.selected_objects().next().unwrap();
    let Geometry::NurbsCurve(section) = output.geometry() else {
        panic!("a planar box section must produce one joined intersection curve")
    };
    assert!(section.is_closed().unwrap());
    assert_eq!(section.domain(), 0.0..=40.0);
    assert_eq!(section.control_points().len(), 5);
    assert!(output.attributes().name().is_none());
    assert_eq!(output.attributes().layer_id(), document.current_layer_id());
}

#[test]
fn intersect_creates_coincident_planar_surface_disk_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let plane = NurbsSurface::try_bilinear([
        Point3::try_new(-3.0, -3.0, 0.0).unwrap(),
        Point3::try_new(3.0, -3.0, 0.0).unwrap(),
        Point3::try_new(3.0, 3.0, 0.0).unwrap(),
        Point3::try_new(-3.0, 3.0, 0.0).unwrap(),
    ])
    .unwrap();
    let boundary = Circle3::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        2.0,
        UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let disk = Brep::try_planar_face(&boundary, Tolerance::DEFAULT).unwrap();
    let input_ids = [
        document
            .add_geometry(Geometry::NurbsSurface(plane))
            .unwrap(),
        document.add_geometry(Geometry::Brep(disk)).unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(curve) = document.selected_objects().next().unwrap().geometry() else {
        panic!("coincident disk must create its circular boundary")
    };
    assert!(curve.is_closed().unwrap());
    assert_eq!(curve.control_points(), boundary.control_points());
    assert!(input_ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_creates_coincident_trimmed_brep_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let rectangle = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 4.0, 0.0).unwrap(),
        Point3::try_new(0.0, 4.0, 0.0).unwrap(),
    ])
    .unwrap();
    let [south, _north] = Brep::try_split_rectangular_surface_face_v(
        rectangle,
        0.0..=1.0,
        0.0..=1.0,
        0.5,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let partial = NurbsSurface::try_bilinear([
        Point3::try_new(1.0, -1.0, 0.0).unwrap(),
        Point3::try_new(3.0, -1.0, 0.0).unwrap(),
        Point3::try_new(3.0, 3.0, 0.0).unwrap(),
        Point3::try_new(1.0, 3.0, 0.0).unwrap(),
    ])
    .unwrap();
    let second = Brep::try_surface_face(partial, Tolerance::DEFAULT).unwrap();
    let input_ids = [
        document.add_geometry(Geometry::Brep(south)).unwrap(),
        document.add_geometry(Geometry::Brep(second)).unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(curve) = document.selected_objects().next().unwrap().geometry() else {
        panic!("coincident trimmed faces must create their shared perimeter")
    };
    assert!(curve.is_closed().unwrap());
    for corner in [
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap(),
        Point3::try_new(3.0, 2.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.0).unwrap(),
    ] {
        let parameter = curve.closest_parameter(corner, Tolerance::DEFAULT).unwrap();
        assert!(
            curve
                .evaluate(parameter)
                .unwrap()
                .distance_to(corner)
                .unwrap()
                < 1e-9
        );
    }
}

#[test]
fn intersect_joins_planar_brep_brep_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_ids = [
        document
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
        document
            .add_geometry(Geometry::Brep(intersection_box_with_intervals([
                [5.0, 15.0],
                [5.0, 15.0],
                [5.0, 15.0],
            ])))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    assert!(input_ids.iter().all(|id| !document.is_selected(*id)));
    let output = document.selected_objects().next().unwrap();
    let Geometry::NurbsCurve(intersection) = output.geometry() else {
        panic!("overlapping planar B-reps must create one joined curve")
    };
    assert!(intersection.is_closed().unwrap());
    assert_eq!(intersection.control_points().len(), 7);
    assert_eq!(intersection.domain(), 0.0..=30.0);
    assert!(output.attributes().name().is_none());
    assert_eq!(output.attributes().layer_id(), document.current_layer_id());
}

#[test]
fn intersect_identical_boxes_creates_four_edge_paths() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let ids = [
        document
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
        document
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 4 intersection object(s) from 1 object pair(s)"
    );
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 4);
    let mut edge_count = 0;
    for output in outputs {
        let Geometry::NurbsCurve(curve) = output.geometry() else {
            panic!("identical boxes must create only linear curves")
        };
        assert_eq!(curve.degree(), 1);
        edge_count += curve.control_points().len() - 1;
    }
    assert_eq!(edge_count, 12);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_rolls_back_unsupported_coincident_brep_pairs() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let input_ids = [
        document
            .add_geometry(Geometry::Brep(intersection_cylinder()))
            .unwrap(),
        document
            .add_geometry(Geometry::Brep(intersection_box()))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();
    let before_objects = document.objects().cloned().collect::<Vec<_>>();
    let before_selection = document.selected_object_ids().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);

    assert!(matches!(
        registry.execute(&mut document, "Intersect"),
        Err(CommandError::Geometry(
            GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident planar surfaces outside certified convex or monotone strip patches"
            }
        ))
    ));
    assert_eq!(
        document.objects().cloned().collect::<Vec<_>>(),
        before_objects
    );
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        before_selection
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn intersect_retains_pairwise_duplicates_and_clears_selection_without_hits() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Line 0,0 10,0", "Line 5,-5 5,5", "Line 0,-5 10,5"] {
        registry.execute(&mut document, command).unwrap();
    }
    let input_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 3 intersection object(s) from 3 object pair(s)"
    );
    assert_eq!(document.selected_object_count(), 3);
    assert!(document.selected_objects().all(|object| {
        matches!(
            object.geometry(),
            Geometry::Point(point) if *point == Point3::try_new(5.0, 0.0, 0.0).unwrap()
        )
    }));

    let mut no_hit = Document::default();
    registry.execute(&mut no_hit, "Line 0,0 10,0").unwrap();
    registry.execute(&mut no_hit, "Line 0,2 10,2").unwrap();
    let ids = no_hit
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    no_hit
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let history = no_hit.undo_label().map(str::to_owned);
    assert_eq!(
        registry.execute(&mut no_hit, "Intersect").unwrap(),
        "Created 0 intersection object(s) from 1 object pair(s)"
    );
    assert_eq!(no_hit.selected_object_count(), 0);
    assert_eq!(no_hit.objects().count(), 2);
    assert_eq!(no_hit.undo_label(), history.as_deref());
}

#[test]
fn intersect_rejects_invalid_selection_and_arguments_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "Intersect extra"),
        Err(CommandError::Usage("Intersect"))
    ));
    assert!(matches!(
        registry.execute(&mut document, "Intersect"),
        Err(CommandError::IntersectRequiresAtLeastTwoObjects { actual: 0 })
    ));
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    registry.execute(&mut document, "Point 5,0").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before_objects = document.objects().cloned().collect::<Vec<_>>();
    let before_selection = document.selected_object_ids().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Intersect"),
        Err(CommandError::UnsupportedIntersectGeometry)
    ));
    assert_eq!(
        document.objects().cloned().collect::<Vec<_>>(),
        before_objects
    );
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        before_selection
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
