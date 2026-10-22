//! transforms basic command regressions.
use super::*;

#[test]
fn move_and_copy_transform_the_selection_as_atomic_commands() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(registry.execute(&mut document, "Move 0,0 1,1").is_err());
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    let original = document.objects().next().unwrap().id();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Move 0,0,0 4,-2,1")
        .unwrap();
    assert_eq!(document.undo_label(), Some("Move"));
    assert!(matches!(
        document.object(original).unwrap().geometry(),
        Geometry::Point(point) if *point == Point3::try_new(5.0, 0.0, 4.0).unwrap()
    ));
    registry.execute(&mut document, "Undo").unwrap();
    assert!(matches!(
        document.object(original).unwrap().geometry(),
        Geometry::Point(point) if *point == Point3::try_new(1.0, 2.0, 3.0).unwrap()
    ));
    registry.execute(&mut document, "Redo").unwrap();

    registry.execute(&mut document, "Copy 5,0,4 8,1,4").unwrap();
    assert_eq!(document.undo_label(), Some("Copy"));
    assert_eq!(document.objects().len(), 2);
    assert!(document.is_selected(original));
    assert_eq!(document.selected_object_count(), 1);
    let copy = document
        .objects()
        .find(|object| object.id() != original)
        .unwrap()
        .id();
    assert!(!document.is_selected(copy));
    assert_ne!(copy, original);
    assert!(matches!(
        document.object(copy).unwrap().geometry(),
        Geometry::Point(point) if *point == Point3::try_new(8.0, 1.0, 4.0).unwrap()
    ));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.object(original).is_some());
}

#[test]
fn scale_rotate_and_mirror_support_numeric_and_reference_forms() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 2,1,0").unwrap();
    let object = document.objects().next().unwrap().id();
    document
        .select_object(object, SelectionMode::Replace)
        .unwrap();
    let position = |document: &Document| match document.object(object).unwrap().geometry() {
        Geometry::Point(point) => *point,
        _ => panic!("expected a point"),
    };

    let history_before = document.undo_label().map(str::to_owned);
    registry.execute(&mut document, "Scale 1,1 -1").unwrap();
    // Native Scale uses the factor's magnitude, so -1 is an identity edit.
    assert_eq!(position(&document), Point3::try_new(2.0, 1.0, 0.0).unwrap());
    assert_eq!(document.undo_label(), history_before.as_deref());
    registry.execute(&mut document, "Scale 1,1 2").unwrap();
    assert_eq!(position(&document), Point3::try_new(3.0, 1.0, 0.0).unwrap());
    assert_eq!(document.undo_label(), Some("Scale"));
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Scale 1,1 2,1 3,1")
        .unwrap();
    assert_eq!(position(&document), Point3::try_new(3.0, 1.0, 0.0).unwrap());
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "Rotate 1,1 90").unwrap();
    assert!(position(&document).is_near(
        Point3::try_new(1.0, 2.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert_eq!(document.undo_label(), Some("Rotate"));
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Rotate 1,1 2,1 1,2")
        .unwrap();
    assert!(position(&document).is_near(
        Point3::try_new(1.0, 2.0, 0.0).unwrap(),
        document.tolerance()
    ));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Mirror 0,0 0,1 Copy=No")
        .unwrap();
    assert_eq!(
        position(&document),
        Point3::try_new(-2.0, 1.0, 0.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Mirror"));
    registry.execute(&mut document, "Undo").unwrap();

    let history_label = document.undo_label().map(str::to_owned);
    assert!(registry.execute(&mut document, "Scale 1,1 0").is_err());
    assert!(registry.execute(&mut document, "Mirror 0,0 0,0").is_err());
    assert_eq!(position(&document), Point3::try_new(2.0, 1.0, 0.0).unwrap());
    assert_eq!(document.undo_label(), history_label.as_deref());
}

#[test]
fn nonuniform_scale_commands_support_axes_references_flattening_and_copy() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 2,3,4").unwrap();
    let original = document.objects().next().unwrap().id();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();
    let position = |document: &Document, id: ObjectId| {
        let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
            panic!("expected a point")
        };
        *point
    };

    registry
        .execute(&mut document, "Scale1D 0,0,0 2 1,1,0")
        .unwrap();
    assert!(position(&document, original).is_near(
        Point3::try_new(4.5, 5.5, 4.0).unwrap(),
        document.tolerance()
    ));
    assert_eq!(document.undo_label(), Some("Scale1D"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Scale1D 0,0,0 1,0,0 3,0,0")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(6.0, 3.0, 4.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Scale1D 0,0,0 0 1,0,0")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(0.0, 3.0, 4.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry.execute(&mut document, "Scale2D 1,1,1 2").unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(3.0, 5.0, 4.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Scale2D 0,0,0 2,0,10 4,0,-20")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(4.0, 6.0, 4.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "ScaleNU 1,1,1 2 -1 .5 Copy=Yes")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 3.0, 4.0).unwrap()
    );
    let copy = document.objects().nth(1).unwrap().id();
    assert_eq!(
        position(&document, copy),
        Point3::try_new(3.0, -1.0, 2.5).unwrap()
    );
    assert!(document.is_selected(original));
    assert!(!document.is_selected(copy));
    assert_eq!(document.undo_label(), Some("ScaleNU"));

    registry.execute(&mut document, "Undo").unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Scale1D 0,0,0 2 0,0,0",
        "Scale2D 0,0,0 0",
        "ScaleNU 0,0,0 1 NaN 1",
        "ScaleNU 0,0,0 1 2 3 Copy=Yes Copy=No",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 3.0, 4.0).unwrap()
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn shear_matches_rhino_top_view_basis_angles_copy_and_atomic_errors() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 2,3,4").unwrap();
    let original = document.objects().next().unwrap().id();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();
    let position = |document: &Document, id: ObjectId| {
        let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
            panic!("expected a point")
        };
        *point
    };

    registry
        .execute(&mut document, "Shear 0,0,0 1,0,0 45")
        .unwrap();
    assert!(position(&document, original).is_near(
        Point3::try_new(2.0, 5.0, 4.0).unwrap(),
        document.tolerance()
    ));
    assert_eq!(document.undo_label(), Some("Shear"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Shear 0,0,0 2,0,0 -30")
        .unwrap();
    assert!(position(&document, original).is_near(
        Point3::try_new(2.0, 1.845_299_461_620_748_5, 4.0).unwrap(),
        document.tolerance()
    ));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Shear 1,2,3 1,5,3 30")
        .unwrap();
    assert!(position(&document, original).is_near(
        Point3::try_new(1.422_649_730_810_374_3, 3.0, 4.0).unwrap(),
        document.tolerance()
    ));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Shear 0,0,0 2,0,0 2,2,0 Copy=Yes")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 3.0, 4.0).unwrap()
    );
    let copy = document.objects().nth(1).unwrap().id();
    assert!(position(&document, copy).is_near(
        Point3::try_new(2.0, 5.0, 4.0).unwrap(),
        document.tolerance()
    ));
    assert!(document.is_selected(original));
    assert!(!document.is_selected(copy));
    registry.execute(&mut document, "Undo").unwrap();

    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Shear 0,0,0 0,0,5 45",
        "Shear 0,0,0 1,0,0 0,0,0",
        "Shear 0,0,0 1,0,0 45 Copy=Yes Copy=No",
        "Shear 0,0,0 1,0,0 45 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 3.0, 4.0).unwrap()
    );
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn project_to_cplane_matches_rhino_delete_input_groups_and_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 1,2,3 4,5,6").unwrap();
    registry.execute(&mut document, "Point -2,7,-4").unwrap();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .add_group(
            Some("Projection pair".to_owned()),
            source_ids.iter().copied(),
        )
        .unwrap();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();

    registry.execute(&mut document, "ProjectToCPlane").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.groups().len(), 2);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    let copies = document
        .objects()
        .filter(|object| !source_ids.contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    assert!(
        copies
            .iter()
            .all(|object| !document.is_selected(object.id()))
    );
    let Geometry::Line(projected_line) = copies[0].geometry() else {
        panic!("expected a projected line")
    };
    assert_eq!(
        projected_line.start(),
        Point3::try_new(1.0, 2.0, 0.0).unwrap()
    );
    assert_eq!(
        projected_line.end(),
        Point3::try_new(4.0, 5.0, 0.0).unwrap()
    );
    let Geometry::Point(projected_point) = copies[1].geometry() else {
        panic!("expected a projected point")
    };
    assert_eq!(*projected_point, Point3::try_new(-2.0, 7.0, 0.0).unwrap());
    assert_eq!(document.undo_label(), Some("ProjectToCPlane"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "ProjectToCPlane DeleteInput=Yes")
        .unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    let Geometry::Line(projected_line) = document.object(source_ids[0]).unwrap().geometry() else {
        panic!("expected the source line identity")
    };
    assert_eq!(projected_line.start().z(), 0.0);
    assert_eq!(projected_line.end().z(), 0.0);
    let Geometry::Point(projected_point) = document.object(source_ids[1]).unwrap().geometry()
    else {
        panic!("expected the source point identity")
    };
    assert_eq!(projected_point.z(), 0.0);
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "ProjectToCPlane _Yes")
        .unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ProjectToCPlane DeleteInput=Maybe",
        "ProjectToCPlane Other=Yes",
        "ProjectToCPlane Yes No",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn rotate_three_dimensional_and_transform_copy_options_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 2,0,1").unwrap();
    let original = document.objects().next().unwrap().id();
    document
        .select_object(original, SelectionMode::Replace)
        .unwrap();
    let position = |document: &Document, id: ObjectId| {
        let Geometry::Point(point) = document.object(id).unwrap().geometry() else {
            panic!("expected a point")
        };
        *point
    };

    registry
        .execute(&mut document, "Rotate3D 0,0,0 0,0,2 90")
        .unwrap();
    assert!(position(&document, original).is_near(
        Point3::try_new(0.0, 2.0, 1.0).unwrap(),
        document.tolerance()
    ));
    assert_eq!(document.undo_label(), Some("Rotate3D"));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Rotate3D 0,0,0 0,0,2 1,0,5 0,1,-2 Copy=Yes")
        .unwrap();
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 0.0, 1.0).unwrap()
    );
    let copy = document.objects().nth(1).unwrap().id();
    assert!(position(&document, copy).is_near(
        Point3::try_new(0.0, 2.0, 1.0).unwrap(),
        document.tolerance()
    ));
    assert!(document.is_selected(original));
    assert!(!document.is_selected(copy));
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "Rotate 0,0 90 Copy=Yes")
        .unwrap();
    assert_eq!(document.objects().len(), 2);
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Mirror 0,0 0,1 Copy=Yes")
        .unwrap();
    let mirrored = document.objects().nth(1).unwrap().id();
    assert_eq!(
        position(&document, mirrored),
        Point3::try_new(-2.0, 0.0, 1.0).unwrap()
    );
    registry.execute(&mut document, "Undo").unwrap();

    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Rotate3D 0,0,0 0,0,0 90",
        "Rotate3D 0,0,0 0,0,2 0,0,1 1,0,0",
        "Rotate3D 0,0,0 0,0,2 90 Copy=Yes Copy=No",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        position(&document, original),
        Point3::try_new(2.0, 0.0, 1.0).unwrap()
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
