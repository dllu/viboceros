#[allow(dead_code)]
#[path = "support/grip_transform.rs"]
mod grip_fixtures;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/splop_guided.rs"]
mod splop_guided_test_support;
#[path = "support/splop_tangent.rs"]
mod support;
#[test]
fn native_splop_tangent_reference_clouds_selection_and_real_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let (mut doc, sources) = support::guided::setup(&value);
            let id = op["id"].as_str().unwrap();
            support::check(&sources, &doc, &op, &value, "before", id);
            support::execute(&mut doc, &sources, &op, &value);
            let mut gap = support::check(&sources, &doc, &op, &value, "after_script", id);
            if op["finish"] == "CancelReference" {
                assert!(doc.undo_label().is_none());
                assert!(doc.redo_label().is_none());
            } else {
                assert_eq!(doc.undo_label(), Some("Splop"));
                assert!(doc.undo().unwrap().is_some());
                gap = gap.max(support::check(&sources, &doc, &op, &value, "undo", id));
                assert!(doc.redo().unwrap().is_some());
                gap = gap.max(support::check(&sources, &doc, &op, &value, "redo", id));
            }
            println!(
                "SPLOP_TANGENT_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn rational_arc_tangent_reference_satisfies_the_exact_three_four_five_construction() {
    use viboceros_geometry::Point3;
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "tangent_92")
        .unwrap();
    let (doc, sources) = support::guided::setup(&value);
    let def = support::reference(&doc, &sources, &op, &value).unwrap();
    assert!(
        def.frame
            .origin()
            .distance_to(Point3::try_from([3., 4., 0.]).unwrap())
            .unwrap()
            < 1e-9
    );
    assert!((def.radius - 2.).abs() < 1e-10);
    assert!(
        Point3::try_from(def.frame.x_axis().as_vector().to_array())
            .unwrap()
            .distance_to(Point3::try_from([-0.6, -0.8, 0.]).unwrap())
            .unwrap()
            < 1e-9
    );
}
#[test]
fn sphere_and_circle_getters_retain_their_distinct_unconstrained_circle_line_rules() {
    use viboceros_command::circle_tangent_input::Input;
    use viboceros_geometry::Point3;
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "tangent_48")
        .unwrap();
    let (doc, sources) = support::guided::setup(&value);
    let sphere = support::reference(&doc, &sources, &op, &value).unwrap();
    assert!(
        sphere
            .frame
            .origin()
            .distance_to(Point3::try_from([0., 5., 0.]).unwrap())
            .unwrap()
            < 1e-9
    );
    assert!((sphere.radius - 3.).abs() < 1e-9);
    let mut circle = Input::new(support::plane(&op), doc.tolerance(), Default::default());
    for (i, id) in sources.guides.iter().enumerate() {
        assert!(
            circle
                .curve(&doc, *id, None, support::contact(&value, i + 1))
                .unwrap()
                .is_none()
        );
    }
    let circle = circle.enter().unwrap().unwrap();
    assert!((circle.center().x() - support::contact(&value, 2).x()).abs() < 1e-9);
    assert!(circle.center().distance_to(sphere.frame.origin()).unwrap() > 0.1);
}
