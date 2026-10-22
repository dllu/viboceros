//! surfaces creation command regressions.
use super::*;

#[test]
fn creates_and_tessellates_a_four_corner_surface_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let message = registry
        .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,2 0,3,2")
        .unwrap();
    assert!(message.contains("four-corner NURBS surface"));
    let Geometry::NurbsSurface(surface) = document.objects().next().unwrap().geometry() else {
        panic!("expected a NURBS surface")
    };
    assert_eq!(surface.degree_u(), 1);
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(
        surface.evaluate(0.5, 0.5).unwrap(),
        Point3::try_new(2.0, 1.5, 1.0).unwrap()
    );
    assert_eq!(
        combined_document_mesh(&document).unwrap().triangles().len(),
        2 * SURFACE_EXPORT_SAMPLES_PER_SPAN * SURFACE_EXPORT_SAMPLES_PER_SPAN
    );
    assert_eq!(document.undo_label(), Some("SrfPt"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);

    assert!(
        registry
            .execute(&mut document, "SrfPt 0,0 1,0 2,0 3,0")
            .is_err()
    );
    assert_eq!(document.objects().len(), 0);
}

#[test]
fn planar_surface_creates_exact_trimmed_faces_and_deletes_only_valid_inputs() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let output_layer = document.current_layer_id();
    let input_layer = document
        .add_layer("Planar boundaries", ColorRgb::new(91, 92, 93))
        .unwrap();
    let concave = Polyline3::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 2.0).unwrap(),
            Point3::try_new(3.0, 0.0, 2.0).unwrap(),
            Point3::try_new(3.0, 1.0, 2.0).unwrap(),
            Point3::try_new(1.0, 1.0, 2.0).unwrap(),
            Point3::try_new(1.0, 3.0, 2.0).unwrap(),
            Point3::try_new(0.0, 3.0, 2.0).unwrap(),
            Point3::try_new(0.0, 0.0, 2.0).unwrap(),
        ],
        tolerance,
    )
    .unwrap();
    let circle = Circle3::try_new(
        Point3::try_new(8.0, 0.0, 2.0).unwrap(),
        2.0,
        UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
        tolerance,
    )
    .unwrap();
    let line = LineSegment::try_new(
        Point3::try_new(12.0, 0.0, 2.0).unwrap(),
        Point3::try_new(15.0, 0.0, 2.0).unwrap(),
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
            .add_geometry_with_attributes(
                Geometry::Polyline(concave),
                ObjectAttributes::on_layer(input_layer).with_name("concave"),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Circle(circle),
                ObjectAttributes::on_layer(input_layer).with_name("circle"),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Line(line),
                ObjectAttributes::on_layer(input_layer).with_name("open"),
            )
            .unwrap(),
        document
            .add_geometry_with_attributes(
                Geometry::Polyline(nonplanar),
                ObjectAttributes::on_layer(input_layer).with_name("nonplanar"),
            )
            .unwrap(),
    ];
    let group = document
        .add_group(Some("Planar inputs".to_owned()), source_ids)
        .unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "PlanarSrf DeleteInput=No")
            .unwrap(),
        "Created 2 exact trimmed planar surface(s)"
    );
    let outputs = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert_eq!(document.group(group).unwrap().members().len(), 4);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    for output in &outputs {
        assert!(!document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
    }
    let Geometry::Brep(concave_face) = outputs[0].geometry() else {
        panic!("PlanarSrf must create a trimmed B-rep face")
    };
    assert_eq!(concave_face.faces().len(), 1);
    assert!(!concave_face.is_closed());
    assert!(
        (concave_face
            .tessellate(1, tolerance)
            .unwrap()
            .area()
            .unwrap()
            - 5.0)
            .abs()
            < 1.0e-12
    );
    let Geometry::Brep(circle_face) = outputs[1].geometry() else {
        panic!("PlanarSrf must preserve a rational circular trim")
    };
    let expected_circle_area = std::f64::consts::PI * 4.0;
    assert!(
        (circle_face
            .tessellate(8, tolerance)
            .unwrap()
            .area()
            .unwrap()
            - expected_circle_area)
            .abs()
            / expected_circle_area
            < 0.01
    );
    assert_eq!(document.undo_label(), Some("PlanarSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "PlanarSurface DeleteInput=Yes")
            .unwrap(),
        "Created 2 exact trimmed planar surface(s), deleting the input curves"
    );
    assert_eq!(document.objects().len(), 4);
    assert!(document.object(source_ids[0]).is_none());
    assert!(document.object(source_ids[1]).is_none());
    assert!(document.object(source_ids[2]).is_some());
    assert!(document.object(source_ids[3]).is_some());
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        [source_ids[2], source_ids[3]].into_iter().collect()
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct(source_ids, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "PlanarSrf DeleteInput=Maybe",
        "PlanarSrf DeleteInput=Yes DeleteInput=No",
        "PlanarSrf extra",
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
        .select_objects_direct(source_ids[2..].iter().copied(), SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "PlanarSrf"),
        Err(CommandError::NoPlanarSurfaceBoundaries)
    ));
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn planar_surface_builds_holes_and_alternating_nested_islands_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let rectangle = |min_x, min_y, max_x, max_y| {
        Geometry::Polyline(
            Polyline3::try_new(
                vec![
                    Point3::try_new(min_x, min_y, 0.0).unwrap(),
                    Point3::try_new(max_x, min_y, 0.0).unwrap(),
                    Point3::try_new(max_x, max_y, 0.0).unwrap(),
                    Point3::try_new(min_x, max_y, 0.0).unwrap(),
                    Point3::try_new(min_x, min_y, 0.0).unwrap(),
                ],
                tolerance,
            )
            .unwrap(),
        )
    };
    let outer = document
        .add_geometry(rectangle(0.0, 0.0, 10.0, 10.0))
        .unwrap();
    let hole = document
        .add_geometry(rectangle(2.0, 2.0, 8.0, 8.0))
        .unwrap();
    let island = document
        .add_geometry(rectangle(4.0, 4.0, 6.0, 6.0))
        .unwrap();
    let inputs = [outer, hole, island];
    document
        .select_objects_direct([hole, island, outer], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "PlanarSrf DeleteInput=No")
            .unwrap(),
        "Created 2 exact trimmed planar surface(s)"
    );
    let mut output_regions = document
        .objects()
        .filter(|object| !inputs.contains(&object.id()))
        .map(|object| {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("PlanarSrf output must be a B-rep")
            };
            (brep.faces()[0].loops().len(), brep.area(tolerance).unwrap())
        })
        .collect::<Vec<_>>();
    output_regions.sort_by(|left, right| left.1.total_cmp(&right.1));
    assert_eq!(output_regions.len(), 2);
    assert_eq!(output_regions[0].0, 1);
    assert!((output_regions[0].1 - 4.0).abs() < 1.0e-12);
    assert_eq!(output_regions[1].0, 2);
    assert!((output_regions[1].1 - 64.0).abs() < 1.0e-12);
    assert!(inputs.iter().all(|id| document.is_selected(*id)));
    assert_eq!(document.undo_label(), Some("PlanarSrf"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 3);
    document
        .select_objects_direct([outer, island, hole], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "PlanarSrf DeleteInput=Yes")
            .unwrap(),
        "Created 2 exact trimmed planar surface(s), deleting the input curves"
    );
    assert_eq!(document.objects().len(), 2);
    assert!(inputs.iter().all(|id| document.object(*id).is_none()));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 3);
    assert!(inputs.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn planar_surface_keeps_partially_overlapping_curves_independent() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let rectangle = |min_x, min_y, max_x, max_y| {
        Geometry::Polyline(
            Polyline3::try_new(
                vec![
                    Point3::try_new(min_x, min_y, 0.0).unwrap(),
                    Point3::try_new(max_x, min_y, 0.0).unwrap(),
                    Point3::try_new(max_x, max_y, 0.0).unwrap(),
                    Point3::try_new(min_x, max_y, 0.0).unwrap(),
                    Point3::try_new(min_x, min_y, 0.0).unwrap(),
                ],
                tolerance,
            )
            .unwrap(),
        )
    };
    let inputs = [
        document
            .add_geometry(rectangle(0.0, 0.0, 10.0, 10.0))
            .unwrap(),
        document
            .add_geometry(rectangle(2.0, 2.0, 6.0, 6.0))
            .unwrap(),
        document
            .add_geometry(rectangle(4.0, 4.0, 8.0, 8.0))
            .unwrap(),
        document
            .add_geometry(rectangle(0.5, 0.5, 1.5, 1.5))
            .unwrap(),
    ];
    document
        .select_objects_direct(inputs, SelectionMode::Replace)
        .unwrap();

    let expected_count = if cfg!(feature = "native-smlib") { 2 } else { 3 };
    assert_eq!(
        registry.execute(&mut document, "PlanarSrf").unwrap(),
        format!("Created {expected_count} exact trimmed planar surface(s)")
    );
    let mut output_regions = document
        .objects()
        .filter(|object| !inputs.contains(&object.id()))
        .map(|object| {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("PlanarSrf output must be a B-rep")
            };
            (brep.faces()[0].loops().len(), brep.area(tolerance).unwrap())
        })
        .collect::<Vec<_>>();
    output_regions.sort_by(|left, right| left.1.total_cmp(&right.1));
    let expected = if cfg!(feature = "native-smlib") {
        vec![(1, 16.), (3, 83.)]
    } else {
        vec![(1, 16.), (1, 16.), (2, 99.)]
    };
    assert_eq!(output_regions.len(), expected.len());
    for ((loops, area), (expected_loops, expected_area)) in output_regions.into_iter().zip(expected)
    {
        assert_eq!(loops, expected_loops);
        assert!((area - expected_area).abs() < 1e-12);
    }
}
