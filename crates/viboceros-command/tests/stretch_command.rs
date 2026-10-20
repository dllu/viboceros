use viboceros_command::CommandRegistry;
#[path = "support/stretch.rs"]
mod support;
#[test]
fn native_stretch_segment_definitions() {
    use viboceros_geometry::PointMorph;
    let observed: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/stretch_details.json"
    ))
    .unwrap();
    for row in observed["results"].as_array().unwrap().iter().take(2) {
        let curve = support::morph::curve(&row["value"]["curve"]);
        let (start, end) = (
            support::point(&serde_json::json!([-1., 0., 0.])),
            support::point(&serde_json::json!([1., 0., 0.])),
        );
        let result = viboceros_command::stretch::Target::Factor(2.)
            .morph(start, end)
            .unwrap()
            .rebuild_grip_curve(curve, viboceros_geometry::Tolerance::DEFAULT)
            .unwrap();
        let viboceros_geometry::Curve3::PolyCurve(poly) = result else {
            panic!("{}", row["id"]);
        };
        assert_eq!(
            poly.segments().len(),
            row["value"]["segments"].as_array().unwrap().len()
        );
        for (a, b) in poly
            .segments()
            .iter()
            .zip(row["value"]["segments"].as_array().unwrap())
        {
            let actual = a.to_nurbs().unwrap();
            let expected = support::morph::curve(&b["curve"]);
            assert_eq!(actual.degree(), expected.degree());
            assert_eq!(actual.knots(), expected.knots());
            assert_eq!(
                actual.control_points().len(),
                expected.control_points().len()
            );
            for (p, q) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                assert!(p.point().distance_to(q.point()).unwrap() < 1e-12);
                assert_eq!(p.weight(), q.weight());
            }
        }
    }
}
#[test]
fn native_stretch_points_and_postselection() {
    assert_native_workflows("stretch_command", 16);
}
#[test]
fn native_stretch_fitted_geometry_and_rigid_groups() {
    assert_native_workflows("stretch_geometry_command", 57);
}
#[test]
fn native_stretch_grips_and_repeated_copy_display() {
    assert_native_workflows("stretch_grips", 46);
}
fn assert_native_workflows(kind: &str, expected_count: usize) {
    let mut failures = Vec::new();
    let mut count = 0;
    for (op, value) in support::rows() {
        if op["op"] != kind {
            continue;
        }
        let label = op["id"].as_str().unwrap().to_owned();
        let result = std::panic::catch_unwind(|| {
            let (mut doc, sources) = support::setup(&op, &value);
            sources.check(&doc, &op, &value, "before");
            let registry = CommandRegistry::with_builtins();
            let (start, end) = support::axis(&op);
            let options = support::options(&op);
            let words = sources.arguments(&op);
            let mut group = doc.begin_history_group("Stretch").unwrap();
            for target in support::targets(&op) {
                let input = if op["early_factor"] == true {
                    format!(
                        "Stretch {} {} {} {} {words}",
                        support::token(start),
                        target.argument(),
                        support::token(end),
                        options.arguments()
                    )
                } else {
                    format!(
                        "Stretch {} {} {} {} {words}",
                        support::token(start),
                        support::token(end),
                        target.argument(),
                        options.arguments()
                    )
                };
                registry
                    .execute_in_history_group(&mut doc, &input, Default::default(), &mut group)
                    .unwrap_or_else(|e| panic!("{label}: {e}"));
            }
            let mut gap = sources.check(&doc, &op, &value, "after");
            let undo = &value["undo"];
            let undo_objects = if undo.is_array() {
                undo.as_array().unwrap().len()
            } else {
                undo["objects"].as_array().map_or(0, Vec::len)
            };
            if !undo.is_null() && undo_objects > 0 {
                assert_eq!(doc.undo_label(), Some("Stretch"));
                registry.execute(&mut doc, "Undo").unwrap();
                gap = gap.max(sources.check(&doc, &op, &value, "undo"));
                registry.execute(&mut doc, "Redo").unwrap();
                gap = gap.max(sources.check(&doc, &op, &value, "redo"));
            } else {
                assert!(doc.undo_label().is_none(), "{label}: identity history");
            }
            eprintln!(
                "STRETCH_API_AUDIT {}",
                serde_json::json!({"id":label,"kind":op["op"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(label);
        }
        count += 1;
    }
    assert_eq!(count, expected_count);
    assert!(failures.is_empty(), "Stretch differences: {failures:?}");
}
#[test]
fn failed_stretch_batches_preserve_geometry_selection_preferences_and_redo() {
    use viboceros_document::{Document, Geometry, SelectionMode};
    use viboceros_geometry::Point3;
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 5.).unwrap()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry
        .execute(&mut doc, "Stretch 0,0,0 0,0,10 2")
        .unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let factor = registry.stretch_factor_default();
    for input in [
        "Stretch 0,0,0 0,0,0 2",
        "Stretch 0,0,0 0,0,10 NaN",
        "Stretch 0,0,0 0,0,10 Length=NaN",
        "Stretch 0,0,0 0,0,10 2 Copy=Maybe",
        "Stretch 0,0,0 0,0,10 2 Rigid=Yes Rigid=No",
        "Stretch 0,0,0 0,0,10 2 trailing",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
        assert_eq!(registry.stretch_factor_default(), factor);
    }
    registry
        .execute(&mut doc, "Stretch 0,0,0 0,0,10 1 Copy=Yes")
        .unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert!(doc.undo_label().is_none());
}
