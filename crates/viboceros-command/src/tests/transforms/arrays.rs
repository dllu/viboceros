//! transforms arrays command regressions.
use super::*;

#[test]
fn rectangular_array_matches_rhino_unit_cell_selection_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("First"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Second"),
        )
        .unwrap();
    document
        .add_group(Some("Pair".to_owned()), [first, second])
        .unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "Array 3 2 2 2 -1 4 Mode=_UnitCell")
        .unwrap();
    assert!(message.contains("creating 22 copy object(s)"));
    assert_eq!(document.objects().len(), 24);
    assert_eq!(document.groups().len(), 12);
    assert_eq!(document.undo_label(), Some("Array"));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );

    let mut expected_named_points = Vec::new();
    let mut expected_groups = Vec::new();
    for z_index in 0..2 {
        for y_index in 0..2 {
            for x_index in 0..3 {
                let translation = [
                    2.0 * f64::from(x_index),
                    -f64::from(y_index),
                    4.0 * f64::from(z_index),
                ];
                let group = vec![
                    [
                        1.0 + translation[0],
                        2.0 + translation[1],
                        3.0 + translation[2],
                    ],
                    [
                        4.0 + translation[0],
                        2.0 + translation[1],
                        3.0 + translation[2],
                    ],
                ];
                expected_named_points.push((group[0], "First".to_owned()));
                expected_named_points.push((group[1], "Second".to_owned()));
                expected_groups.push(group);
            }
        }
    }
    expected_named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    expected_groups.sort_by(|left, right| left.partial_cmp(right).unwrap());

    let mut actual_named_points = document
        .objects()
        .map(|object| {
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected rectangular-array points")
            };
            (
                point.to_array(),
                object.attributes().name().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    actual_named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    assert_eq!(actual_named_points, expected_named_points);

    let mut actual_groups = document
        .groups()
        .map(|group| {
            let mut points = group
                .members()
                .map(|id| match document.object(id).unwrap().geometry() {
                    Geometry::Point(point) => point.to_array(),
                    _ => panic!("expected grouped rectangular-array points"),
                })
                .collect::<Vec<_>>();
            points.sort_by(|left, right| left.partial_cmp(right).unwrap());
            points
        })
        .collect::<Vec<_>>();
    actual_groups.sort_by(|left, right| left.partial_cmp(right).unwrap());
    assert_eq!(actual_groups, expected_groups);

    let copy_ids = document
        .objects()
        .map(|object| object.id())
        .filter(|id| ![first, second].contains(id))
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 12);
}

#[test]
fn rectangular_array_fill_uses_the_selected_extents() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("First"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Second"),
        )
        .unwrap();
    document
        .add_group(Some("Pair".to_owned()), [first, second])
        .unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Array 3 2 1 10 -6 0 Mode Fill")
        .unwrap();
    assert_eq!(document.objects().len(), 12);
    assert_eq!(document.groups().len(), 6);
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );

    let mut actual = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => point.to_array(),
            _ => panic!("expected rectangular-array points"),
        })
        .collect::<Vec<_>>();
    actual.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let mut expected = Vec::new();
    for y in [0.0, -6.0] {
        for x in [0.0, 3.5, 7.0] {
            expected.push([1.0 + x, 2.0 + y, 3.0]);
            expected.push([4.0 + x, 2.0 + y, 3.0]);
        }
    }
    expected.sort_by(|left, right| left.partial_cmp(right).unwrap());
    assert_eq!(actual, expected);
}

#[test]
fn rectangular_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    document.select_all();
    let history = document.undo_label().map(str::to_owned);

    assert!(matches!(
        registry.execute(&mut document, "Array 0 2 1 1 1 0"),
        Err(CommandError::InvalidArrayDimensionCount(value)) if value == "0"
    ));
    assert!(matches!(
        registry.execute(&mut document, "Array 1000001 2 1 1 1 0"),
        Err(CommandError::TooManyArrayObjects { maximum })
            if maximum == MAX_ARRAY_OBJECTS
    ));
    assert!(matches!(
        registry.execute(&mut document, "Array 2 1 1 1 0 0 Mode=Maybe"),
        Err(CommandError::Usage(ARRAY_USAGE))
    ));
    assert!(matches!(
        registry.execute(&mut document, "Array 2 1 1 0 0 0 Mode=Fill"),
        Err(CommandError::Geometry(GeometryError::Degenerate {
            context: "array fill length"
        }))
    ));
    assert!(
        registry
            .execute(&mut document, "Array 3 1 1 1e308 0 0")
            .is_err()
    );
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn linear_array_matches_rhino_spacing_selection_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("First"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(4.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Second"),
        )
        .unwrap();
    document
        .add_group(Some("Pair".to_owned()), [first, second])
        .unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "ArrayLinear 4 0,0,0 2,-1,3")
        .unwrap();
    assert!(message.contains("creating 6 copy object(s)"));
    assert_eq!(document.objects().len(), 8);
    assert_eq!(document.groups().len(), 4);
    assert_eq!(document.undo_label(), Some("ArrayLinear"));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );

    let mut named_points = document
        .objects()
        .map(|object| {
            let Geometry::Point(point) = object.geometry() else {
                panic!("expected arrayed points")
            };
            (
                point.to_array(),
                object.attributes().name().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    named_points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    assert_eq!(
        named_points,
        vec![
            ([1.0, 2.0, 3.0], "First".to_owned()),
            ([3.0, 1.0, 6.0], "First".to_owned()),
            ([4.0, 2.0, 3.0], "Second".to_owned()),
            ([5.0, 0.0, 9.0], "First".to_owned()),
            ([6.0, 1.0, 6.0], "Second".to_owned()),
            ([7.0, -1.0, 12.0], "First".to_owned()),
            ([8.0, 0.0, 9.0], "Second".to_owned()),
            ([10.0, -1.0, 12.0], "Second".to_owned()),
        ]
    );
    let mut group_points = document
        .groups()
        .map(|group| {
            let mut points = group
                .members()
                .map(|id| match document.object(id).unwrap().geometry() {
                    Geometry::Point(point) => point.to_array(),
                    _ => panic!("expected grouped array points"),
                })
                .collect::<Vec<_>>();
            points.sort_by(|left, right| left.partial_cmp(right).unwrap());
            points
        })
        .collect::<Vec<_>>();
    group_points.sort_by(|left, right| left.partial_cmp(right).unwrap());
    assert_eq!(
        group_points,
        vec![
            vec![[1.0, 2.0, 3.0], [4.0, 2.0, 3.0]],
            vec![[3.0, 1.0, 6.0], [6.0, 1.0, 6.0]],
            vec![[5.0, 0.0, 9.0], [8.0, 0.0, 9.0]],
            vec![[7.0, -1.0, 12.0], [10.0, -1.0, 12.0]],
        ]
    );

    let array_ids = document
        .objects()
        .map(|object| object.id())
        .filter(|id| ![first, second].contains(id))
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
    assert!(document.is_selected(first));
    assert!(document.is_selected(second));
    registry.execute(&mut document, "Redo").unwrap();
    assert!(array_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 4);
}

#[test]
fn linear_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    document.select_all();
    let history = document.undo_label().map(str::to_owned);

    assert!(matches!(
        registry.execute(&mut document, "ArrayLinear 1 0,0,0 1,0,0"),
        Err(CommandError::InvalidArrayItemCount(value)) if value == "1"
    ));
    assert!(matches!(
        registry.execute(&mut document, "ArrayLinear 500002 0,0,0 1,0,0"),
        Err(CommandError::TooManyArrayObjects { maximum })
            if maximum == MAX_ARRAY_OBJECTS
    ));
    assert!(
        registry
            .execute(&mut document, "ArrayLinear 3 0,0,0 1e308,0,0")
            .is_err()
    );
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn polar_array_matches_rhino_full_sweep_selection_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(4.0, 1.0, 0.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("First"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(1.0, -1.0, 2.0).unwrap(),
                    Point3::try_new(2.0, -0.5, 3.0).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Second"),
        )
        .unwrap();
    document
        .add_group(Some("Pair".to_owned()), [first, second])
        .unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(&mut document, "ArrayPolar 4 0,0,0 360")
        .unwrap();
    assert!(message.contains("creating 6 copy object(s)"));
    assert_eq!(document.objects().len(), 8);
    assert_eq!(document.groups().len(), 4);
    assert_eq!(document.undo_label(), Some("ArrayPolar"));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    assert_eq!(
        document
            .objects()
            .filter(|object| object.attributes().name() == Some("First"))
            .count(),
        4
    );

    let mut first_lines = document
        .objects()
        .filter(|object| object.attributes().name() == Some("First"))
        .map(|object| match object.geometry() {
            Geometry::Line(line) => (line.start(), line.end()),
            _ => panic!("expected arrayed lines"),
        })
        .collect::<Vec<_>>();
    first_lines.sort_by(|left, right| left.0.to_array().partial_cmp(&right.0.to_array()).unwrap());
    let expected = [
        (
            Point3::try_new(-2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(-4.0, -1.0, 0.0).unwrap(),
        ),
        (
            Point3::try_new(0.0, -2.0, 0.0).unwrap(),
            Point3::try_new(1.0, -4.0, 0.0).unwrap(),
        ),
        (
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(-1.0, 4.0, 0.0).unwrap(),
        ),
        (
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 1.0, 0.0).unwrap(),
        ),
    ];
    for ((start, end), (expected_start, expected_end)) in first_lines.into_iter().zip(expected) {
        assert!(start.is_near(expected_start, document.tolerance()));
        assert!(end.is_near(expected_end, document.tolerance()));
    }

    let copy_ids = document
        .objects()
        .map(|object| object.id())
        .filter(|id| ![first, second].contains(id))
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
    assert!(document.is_selected(first));
    assert!(document.is_selected(second));
    registry.execute(&mut document, "Redo").unwrap();
    assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 4);
}

#[test]
fn polar_array_no_rotate_uses_the_combined_selection_bounds_center() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let sources = [
        ("First", [2.0, 0.0, 0.0], [4.0, 1.0, 0.0]),
        ("Second", [1.0, -1.0, 2.0], [2.0, -0.5, 3.0]),
    ];
    let ids = sources
        .iter()
        .map(|(name, start, end)| {
            document
                .add_geometry_with_attributes(
                    Geometry::Line(
                        LineSegment::try_new(
                            Point3::try_from(*start).unwrap(),
                            Point3::try_from(*end).unwrap(),
                            document.tolerance(),
                        )
                        .unwrap(),
                    ),
                    ObjectAttributes::on_layer(layer).with_name(*name),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    document
        .add_group(Some("Pair".to_owned()), ids.iter().copied())
        .unwrap();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "ArrayPolar 4 0,0,0 180 Rotate=_No")
        .unwrap();
    let mut first_lines = document
        .objects()
        .filter(|object| object.attributes().name() == Some("First"))
        .map(|object| match object.geometry() {
            Geometry::Line(line) => (line.start(), line.end()),
            _ => panic!("expected arrayed lines"),
        })
        .collect::<Vec<_>>();
    first_lines.sort_by(|left, right| left.0.to_array().partial_cmp(&right.0.to_array()).unwrap());
    let root_three_over_two = 3.0_f64.sqrt() * 1.25;
    let expected_starts = [
        Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
        Point3::try_new(-1.75, root_three_over_two, 0.0).unwrap(),
        Point3::try_new(0.75, root_three_over_two, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
    ];
    let source_direction = Vector3::try_new(2.0, 1.0, 0.0).unwrap();
    for ((start, end), expected_start) in first_lines.into_iter().zip(expected_starts) {
        assert!(start.is_near(expected_start, document.tolerance()));
        let direction = start.vector_to(end).unwrap();
        assert!(
            direction
                .to_array()
                .into_iter()
                .zip(source_direction.to_array())
                .all(|(actual, expected)| document.tolerance().approx_eq(actual, expected))
        );
    }
    assert_eq!(document.groups().len(), 4);
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        ids.into_iter().collect()
    );
}

#[test]
fn polar_array_supports_multi_turn_sweeps_and_cumulative_z_offset() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 2,0,0").unwrap();
    let original = document.objects().next().unwrap().id();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "ArrayPolar 4 0,0,0 720 ZOffset 2")
        .unwrap();

    let mut points = document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected arrayed points"),
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.z().partial_cmp(&right.z()).unwrap());
    let root_three = 3.0_f64.sqrt();
    let expected = [
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(-1.0, -root_three, 2.0).unwrap(),
        Point3::try_new(-1.0, root_three, 4.0).unwrap(),
        Point3::try_new(2.0, 0.0, 6.0).unwrap(),
    ];
    for (actual, expected) in points.into_iter().zip(expected) {
        assert!(actual.is_near(expected, document.tolerance()));
    }
}

#[test]
fn polar_array_rejects_invalid_or_unbounded_output_without_partial_copies() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    document.select_all();
    let history = document.undo_label().map(str::to_owned);

    assert!(matches!(
        registry.execute(&mut document, "ArrayPolar 1 0,0,0 360"),
        Err(CommandError::InvalidArrayItemCount(value)) if value == "1"
    ));
    assert!(matches!(
        registry.execute(&mut document, "ArrayPolar 3 0,0,0 0"),
        Err(CommandError::InvalidPolarArrayAngle(value)) if value == "0"
    ));
    assert!(matches!(
        registry.execute(&mut document, "ArrayPolar 500002 0,0,0 360"),
        Err(CommandError::TooManyArrayObjects { maximum })
            if maximum == MAX_ARRAY_OBJECTS
    ));
    assert!(
        registry
            .execute(&mut document, "ArrayPolar 3 0,0,0 180 ZOffset=1e308",)
            .is_err()
    );
    assert!(matches!(
        registry.execute(&mut document, "ArrayPolar 3 0,0,0 180 Rotate=No Rotate=Yes"),
        Err(CommandError::Usage(ARRAY_POLAR_USAGE))
    ));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}
