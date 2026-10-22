//! transforms curve array command regressions.
use super::*;

#[test]
fn curve_array_items_match_rhino_selection_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let tolerance = document.tolerance();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("First"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(0.0, 2.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Second"),
        )
        .unwrap();
    document
        .add_group(Some("Pair".to_owned()), [first, second])
        .unwrap();
    let path = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Rail"),
        )
        .unwrap();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();

    let message = registry
        .execute(
            &mut document,
            "ArrayCrv Items=4 Orientation=_NoRotation PathName=rail",
        )
        .unwrap();
    assert!(message.contains("creating 6 copy object(s)"));
    assert_eq!(document.objects().len(), 9);
    assert_eq!(document.groups().len(), 4);
    assert_eq!(document.undo_label(), Some("ArrayCrv"));
    assert!(!document.is_selected(path));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );

    let mut first_points = document
        .objects()
        .filter(|object| object.attributes().name() == Some("First"))
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected curve-array points"),
        })
        .collect::<Vec<_>>();
    first_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    for (actual, expected_x) in first_points
        .into_iter()
        .zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0])
    {
        assert!(actual.is_near(Point3::try_new(expected_x, 0.0, 0.0).unwrap(), tolerance));
    }

    let copy_ids = document
        .objects()
        .map(|object| object.id())
        .filter(|id| ![first, second, path].contains(id))
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 3);
    // Native Undo retains copied group definitions, without copied members.
    assert_eq!(document.groups().len(), 4);
    registry.execute(&mut document, "Redo").unwrap();
    assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 4);
}

#[test]
fn curve_array_distance_basepoint_and_closed_path_match_rhino() {
    let registry = CommandRegistry::with_builtins();

    let mut distance_document = Document::default();
    let layer = distance_document.current_layer_id();
    let tolerance = distance_document.tolerance();
    let source = distance_document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Source"),
        )
        .unwrap();
    distance_document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Rail"),
        )
        .unwrap();
    distance_document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut distance_document,
            "ArrayCrv Distance 3 Orientation NoRotation PathName Rail",
        )
        .unwrap();
    let mut distance_points = named_curve_array_points(&distance_document, "Source");
    distance_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    assert_eq!(
        distance_points,
        [0.0, 3.0, 6.0, 9.0].map(|x| Point3::try_new(x, 0.0, 0.0).unwrap())
    );

    let mut base_document = Document::default();
    let layer = base_document.current_layer_id();
    let tolerance = base_document.tolerance();
    let base_source = base_document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(20.0, 0.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Source"),
        )
        .unwrap();
    base_document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Rail"),
        )
        .unwrap();
    base_document
        .select_object(base_source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut base_document,
            "ArrayCurve 4 BasePoint=20,0,0 Orientation=NoRotation PathName=Rail",
        )
        .unwrap();
    let mut base_points = named_curve_array_points(&base_document, "Source");
    base_points.sort_by(|left, right| left.x().total_cmp(&right.x()));
    assert_eq!(base_points.len(), 5);
    for (actual, expected_x) in
        base_points
            .into_iter()
            .zip([0.0, 10.0 / 3.0, 20.0 / 3.0, 10.0, 20.0])
    {
        assert!(actual.is_near(Point3::try_new(expected_x, 0.0, 0.0).unwrap(), tolerance));
    }
    assert!(base_document.is_selected(base_source));

    let mut closed_document = Document::default();
    let layer = closed_document.current_layer_id();
    let tolerance = closed_document.tolerance();
    let closed_source = closed_document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(10.0, 0.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Source"),
        )
        .unwrap();
    closed_document
        .add_geometry_with_attributes(
            Geometry::Circle(
                Circle3::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    10.0,
                    UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Rail"),
        )
        .unwrap();
    closed_document
        .select_object(closed_source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut closed_document,
            "ArrayCrv 4 Orientation=NoRotation PathName=Rail",
        )
        .unwrap();
    let closed_points = named_curve_array_points(&closed_document, "Source");
    assert_eq!(closed_points.len(), 4);
    for expected in [
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
        Point3::try_new(-10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, -10.0, 0.0).unwrap(),
    ] {
        assert!(
            closed_points
                .iter()
                .any(|actual| actual.is_near(expected, tolerance))
        );
    }
}

fn named_curve_array_points(document: &Document, name: &str) -> Vec<Point3> {
    document
        .objects()
        .filter(|object| object.attributes().name() == Some(name))
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected named curve-array points"),
        })
        .collect()
}

#[test]
fn curve_array_orientation_policies_match_planar_rhino_frames() {
    let registry = CommandRegistry::with_builtins();
    for (orientation, rotates) in [
        ("NoRotation", false),
        ("Freeform", true),
        ("Roadlike", true),
        ("Stairlike", true),
    ] {
        let mut document = Document::default();
        let layer = document.current_layer_id();
        let tolerance = document.tolerance();
        let source = document
            .add_geometry_with_attributes(
                Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(11.0, 0.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Source"),
            )
            .unwrap();
        document
            .add_geometry_with_attributes(
                Geometry::Arc(
                    CircularArc3::try_from_three_points(
                        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(5.0 * 2.0_f64.sqrt(), 5.0 * 2.0_f64.sqrt(), 0.0).unwrap(),
                        Point3::try_new(0.0, 10.0, 0.0).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(layer).with_name("Rail"),
            )
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut document,
                &format!("ArrayCrv 2 Orientation={orientation} PathName=Rail"),
            )
            .unwrap();
        let copy = document
            .objects()
            .filter(|object| object.attributes().name() == Some("Source"))
            .find(|object| object.id() != source)
            .unwrap();
        let Geometry::Line(line) = copy.geometry() else {
            panic!("expected an oriented line copy")
        };
        assert!(
            line.start()
                .is_near(Point3::try_new(0.0, 10.0, 0.0).unwrap(), tolerance)
        );
        let expected_end = if rotates {
            Point3::try_new(0.0, 11.0, 0.0).unwrap()
        } else {
            Point3::try_new(1.0, 10.0, 0.0).unwrap()
        };
        assert!(
            line.end().is_near(expected_end, tolerance),
            "{orientation} ended at {:?}, expected {expected_end:?}",
            line.end()
        );
    }
}

#[test]
fn curve_array_uses_last_selected_path_and_rejects_bad_inputs_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let tolerance = document.tolerance();
    let source = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Source"),
        )
        .unwrap();
    let path = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(4.0, 0.0, 0.0).unwrap(),
                    tolerance,
                )
                .unwrap(),
            ),
            ObjectAttributes::on_layer(layer).with_name("Rail"),
        )
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    document.select_object(path, SelectionMode::Add).unwrap();
    registry
        .execute(&mut document, "ArrayCrv 2 Orientation=NoRotation")
        .unwrap();
    assert_eq!(named_curve_array_points(&document, "Source").len(), 2);
    assert!(document.is_selected(source));
    assert!(!document.is_selected(path));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let object_count = document.objects().len();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ArrayCrv 0 PathName=Rail"),
        Err(CommandError::InvalidCurveArrayItemCount(value)) if value == "0"
    ));
    assert!(matches!(
        registry.execute(&mut document, "ArrayCrv Distance=0 PathName=Rail"),
        Err(CommandError::InvalidCurveArrayDistance(value)) if value == "0"
    ));
    assert!(matches!(
        registry.execute(&mut document, "ArrayCrv 2 PathName=Missing"),
        Err(CommandError::CurveArrayPathNotFound(name)) if name == "Missing"
    ));
    assert!(matches!(
        registry.execute(
            &mut document,
            "ArrayCrv 1000002 Orientation=NoRotation PathName=Rail"
        ),
        Err(CommandError::TooManyArrayObjects { .. })
    ));
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(document.is_selected(source));
}
