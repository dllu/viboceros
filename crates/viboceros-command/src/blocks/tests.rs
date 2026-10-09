use super::*;

#[test]
fn cardinal_insertion_rotation_does_not_amplify_trigonometric_noise_at_extreme_scales() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,0,0").unwrap();
    doc.select_all();
    registry.execute(&mut doc, "Block 0,0,0 axis").unwrap();
    registry
        .execute(&mut doc, "Insert axis 0,0,0 Scale=1e308 Rotation=90")
        .unwrap();
    let Geometry::BlockInstance(instance) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert_eq!(point.to_array(), [0., 1e308, 0.]);
}

#[test]
fn create_and_insert_scaled_rotated_shared_instances_with_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 11,22,33").unwrap();
    let source = doc.objects().next().unwrap().id();
    doc.select_object(source, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut doc, "Block 10,20,30 \"Part A\"")
        .unwrap();
    let original = doc.objects().next().unwrap().id();
    registry
        .execute(
            &mut doc,
            "Insert \"part a\" 10,20,30 Scale=2,3,4 Rotation=90",
        )
        .unwrap();
    let inserted = doc.objects().last().unwrap().id();
    let Geometry::BlockInstance(instance) = doc.object(inserted).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert!(
        point
            .distance_to(Point3::try_new(4., 22., 42.).unwrap())
            .unwrap()
            < 1e-12
    );
    let Geometry::BlockInstance(first) = doc.object(original).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        first.reference().definition(),
        instance.reference().definition()
    );
    assert_eq!(doc.undo_label(), Some("Insert"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert!(doc.object(inserted).is_none());
    registry.execute(&mut doc, "Redo").unwrap();
    assert!(doc.object(inserted).is_some());
}

#[test]
fn malformed_names_options_missing_definitions_and_late_placement_failures_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,0,0").unwrap();
    doc.select_all();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    registry.execute(&mut doc, "Point 9,0,0").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    for input in [
        "Insert missing 0,0,0",
        "Insert part 0,0,0 Scale=0",
        "Insert part 0,0,0 Axis=0,0,0",
        "Insert part 0,0,0 Scale=2 Scale=3",
        "Insert part 0,0,0 Rotation=1 Rotate=2",
        "Insert part 0,0,0 File=Yes",
        "Insert \"unfinished 0,0,0",
        "Insert part 0,0,0 Scale=NaN",
        "Insert part 1e308,0,0 Scale=1e308",
    ] {
        let before = format!("{doc:?}");
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before, "{input}");
    }
}

#[test]
fn quoted_tokenizer_preserves_names_and_rejects_ambiguous_boundaries() {
    assert_eq!(
        tokenize("  \"Part  A\" 1,2,3 Scale=2 ").unwrap(),
        ["Part  A", "1,2,3", "Scale=2"]
    );
    for input in ["\"unfinished", "\"\"", "\"name\"tail", "na\"me"] {
        assert!(tokenize(input).is_err());
    }
}
