use super::*;

#[test]
fn split_length_remainder_uses_source_intervals_attributes_and_one_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 9,0,0").unwrap();
    let source = doc.objects().next().unwrap().clone();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(
            &mut doc,
            "Divide Length 2.5 Split=Yes DeleteRemainder=Yes GroupOutput=Yes",
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert!(doc.object(source.id()).is_none());
    for (i, object) in doc.objects().enumerate() {
        assert_eq!(object.attributes(), source.attributes());
        let curve = object.geometry().curve_ref().unwrap();
        assert_eq!(curve.domain(), i as Real * 2.5..=(i + 1) as Real * 2.5);
    }
    assert_eq!(doc.groups().count(), 0);
    doc.undo().unwrap();
    assert_eq!(doc.objects().next().unwrap(), &source);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 3);
}

#[test]
fn chord_mode_crosses_corners_and_groups_outputs_per_source_on_current_layer() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "Polyline 0,0,0 3,0,0 3,4,0 8,4,0")
        .unwrap();
    registry.execute(&mut doc, "Line 0,10,0 10,10,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(&mut doc, "Divide EqualChordLength 2.5 GroupOutput=Yes")
        .unwrap();
    assert_eq!(doc.groups().count(), 2);
    assert_eq!(doc.selected_object_count(), 10);
    assert!(
        doc.selected_objects()
            .all(|o| o.attributes().layer_id() == doc.current_layer_id())
    );
}

#[test]
fn invalid_options_and_mixed_geometry_fail_before_source_or_output_mutation() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,0,0 10,0,0").unwrap();
    registry.execute(&mut doc, "Point 0,1,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for command in [
        "Divide 4 Split=Yes",
        "Divide 4 MarkEnds=bad",
        "Divide 4 Split=Yes Split=No",
        "Divide Length 0",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn weighted_polycurve_points_remain_accurate_and_unrepresentable_split_is_atomic() {
    use viboceros_geometry::{CurveSegment3, LineSegment, NurbsCurve, PolyCurve3, WeightedPoint3};
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let weighted = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 0.), 2_f64.powi(100)).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let curve = PolyCurve3::try_new(vec![
        CurveSegment3::Line(
            LineSegment::try_new(p(-2., 0.), p(0., 0.), Tolerance::DEFAULT).unwrap(),
        ),
        CurveSegment3::NurbsCurve(weighted),
        CurveSegment3::Line(
            LineSegment::try_new(p(2., 0.), p(2., 3.), Tolerance::DEFAULT).unwrap(),
        ),
    ])
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::PolyCurve(curve)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let source = doc.object(id).unwrap().clone();
    registry.execute(&mut doc, "Divide 4 MarkEnds=Yes").unwrap();
    let points = doc
        .selected_objects()
        .map(|o| match o.geometry() {
            Geometry::Point(p) => *p,
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(points.len(), 5);
    for (actual, expected) in
        points
            .iter()
            .zip([p(-2., 0.), p(-0.25, 0.), p(1.5, 0.), p(2., 1.25), p(2., 3.)])
    {
        assert!(actual.distance_to(expected).unwrap() < 1e-12);
    }
    doc.undo().unwrap();
    doc.clear_history().unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let result = registry.execute(&mut doc, "Divide 4 Split=Yes");
    assert!(
        matches!(
            &result,
            Err(CommandError::Geometry(
                GeometryError::UnrepresentableCurveDivisionParameter
            ))
        ),
        "{result:?}"
    );
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.object(id), Some(&source));
    assert!(!doc.can_undo());
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), vec![id]);
}
