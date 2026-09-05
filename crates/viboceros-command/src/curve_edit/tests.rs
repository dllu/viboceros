use super::*;

#[test]
fn join_uses_selection_order_for_seed_direction_and_attributes() {
    let mut document = Document::default();
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let first = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(point(-1.0, 0.0), point(0.0, 0.0), Tolerance::DEFAULT)
                    .unwrap(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("first-created"),
        )
        .unwrap();
    let second = document
        .add_geometry_with_attributes(
            Geometry::Line(
                LineSegment::try_new(point(0.0, 1.0), point(0.0, 0.0), Tolerance::DEFAULT).unwrap(),
            ),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("first-selected"),
        )
        .unwrap();
    document
        .select_object(second, SelectionMode::Replace)
        .unwrap();
    document.select_object(first, SelectionMode::Add).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, "Join")
        .unwrap();
    let object = document.objects().next().unwrap();
    assert_eq!(object.attributes().name(), Some("first-selected"));
    assert_eq!(
        object
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap(),
        point(0.0, 1.0)
    );
}

#[test]
fn mixed_join_preserves_seed_attributes_groups_and_atomic_history() {
    let mut document = Document::new(Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap());
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let curve =
        NurbsCurve::try_clamped_uniform(2, vec![point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)])
            .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("seed")
        .with_object_color(ColorRgb::new(12, 34, 56));
    let first = document
        .add_geometry_with_attributes(Geometry::NurbsCurve(curve), attributes.clone())
        .unwrap();
    let second = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(1.004, 1.0), point(1.0, 3.0), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let first_group = document.add_group(Some("first".into()), [first]).unwrap();
    let second_group = document.add_group(Some("second".into()), [second]).unwrap();
    let shared = document
        .add_group(Some("shared".into()), [first, second])
        .unwrap();
    document
        .select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Join").unwrap();
    let object = document.objects().next().unwrap();
    let output = object.id();
    let joined_geometry = object.geometry().clone();
    assert!(output != first && output != second);
    assert_eq!(object.attributes(), &attributes);
    let Geometry::PolyCurve(curve) = object.geometry() else {
        panic!("expected mixed composite")
    };
    assert_eq!(curve.segments().len(), 2);
    assert_eq!(
        curve.segments()[0].control_points().last().unwrap().point(),
        point(1.002, 1.0)
    );
    assert_eq!(
        document
            .group(first_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![output]
    );
    assert_eq!(
        document
            .group(shared)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![output]
    );
    assert!(document.group(second_group).is_none());
    assert!(document.is_selected(output));
    assert_eq!(document.undo_label(), Some("Join"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        document
            .group(shared)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document.object(output).unwrap().geometry(),
        &joined_geometry
    );
    document
        .select_object(output, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "CloseCrv").unwrap();
    assert!(
        document
            .object(output)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .is_closed()
            .unwrap()
    );
    assert_eq!(document.object(output).unwrap().attributes(), &attributes);
    assert_eq!(
        document
            .group(shared)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![output]
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        !document
            .object(output)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .is_closed()
            .unwrap()
    );
}
