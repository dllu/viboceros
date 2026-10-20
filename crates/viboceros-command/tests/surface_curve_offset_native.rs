#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/surface_curve_offset.rs"]
mod support;
#[test]
fn native_through_point_selected_objects_order_projection_and_history() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    for (op, value) in support::select_rows() {
        if value["success"] != true {
            continue;
        }
        let (mut doc, scene) = support::setup(&op, &value);
        support::check(&doc, &scene, &value, "before");
        let ids = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["curve"].as_u64())
            .filter(|i| *i >= 2)
            .map(|i| scene.references[i as usize - 2].to_string())
            .collect::<Vec<_>>()
            .join(",");
        registry.execute(&mut doc, &format!("OffsetCrvOnSrf ThroughPoint Points={ids} Source={} Surface={} Number=3 Flip=Yes", scene.source, scene.target)).unwrap();
        let mut maximum = support::check(&doc, &scene, &value, "after_script");
        if !value["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["role"] == "output")
        {
            assert!(doc.undo_label().is_none());
        } else {
            assert_eq!(doc.undo_label(), Some("OffsetCrvOnSrf"));
            doc.undo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "undo"));
            assert!(doc.undo().unwrap().is_none());
            doc.redo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "redo"));
        }
        println!(
            "SURFACE_OFFSET_SELECT_POINT {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":value["after_script"].as_array().unwrap().len()-value["before"].as_array().unwrap().len()})
        );
        support::qualify(&op, maximum);
    }
}
#[test]
fn independent_sdk_queries_confirm_native_through_point_discrepancies() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_offset_through_verify.json"
    ))
    .unwrap();
    let r: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_offset_through_verify.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 9);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        assert!(row["error"].is_null());
        let curve = morph_fixtures::curve(op);
        let point = morph_fixtures::p(&op["target"]);
        let t = curve
            .closest_parameter(
                point,
                viboceros_geometry::Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap(),
            )
            .unwrap();
        let distance = point.distance_to(curve.evaluate(t).unwrap()).unwrap();
        assert!(
            (distance - row["value"]["distance"].as_f64().unwrap()).abs() < 1e-10,
            "{}",
            op["id"]
        );
    }
}
#[test]
fn native_surface_offset_command_extensions_selection_layers_and_history() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    for (op, v) in support::rows() {
        let (mut doc, scene) = support::setup(&op, &v);
        support::check(&doc, &scene, &v, "before");
        let flip = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "Flip");
        let distance = op["actions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_f64()
            .unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "OffsetCrvOnSrf {distance} Source={} Surface={} Number={} Flip={}",
                    scene.source,
                    scene.target,
                    support::number(&v),
                    if flip { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        let mut maximum = support::check(&doc, &scene, &v, "after_script");
        assert_eq!(doc.undo_label(), Some("OffsetCrvOnSrf"));
        doc.undo().unwrap();
        maximum = maximum.max(support::check(&doc, &scene, &v, "undo"));
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        maximum = maximum.max(support::check(&doc, &scene, &v, "redo"));
        println!(
            "SURFACE_OFFSET_COMMAND {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":v["after_script"].as_array().unwrap().len()-2})
        );
        support::qualify(&op, maximum);
    }
}

#[test]
fn native_through_point_repeats_projected_locations_and_preserves_accepted_history() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    for (op, value) in support::through_rows() {
        let (mut doc, scene) = support::setup(&op, &value);
        let mut group = doc.begin_history_group("OffsetCrvOnSrf").unwrap();
        support::check(&doc, &scene, &value, "before");
        for point in op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a.is_array())
        {
            let [x, y, z]: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
            let before = format!("{doc:?}");
            let result = registry.execute_in_history_group(&mut doc,&format!("OffsetCrvOnSrf ThroughPoint={x},{y},{z} Source={} Surface={} Number=3 Flip=Yes",scene.source,scene.target),viboceros_command::CommandContext::default(),&mut group);
            if op["case"] == "through_on_source" {
                assert!(result.is_err());
                assert_eq!(format!("{doc:?}"), before);
            } else {
                result.unwrap();
            }
        }
        let mut maximum = support::check(&doc, &scene, &value, "after_script");
        let count = value["after_script"].as_array().unwrap().len() - 2;
        if count > 0 {
            assert_eq!(doc.undo_label(), Some("OffsetCrvOnSrf"));
            doc.undo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "undo"));
            assert!(doc.undo().unwrap().is_none());
            doc.redo().unwrap();
            maximum = maximum.max(support::check(&doc, &scene, &value, "redo"));
        } else {
            assert!(doc.undo_label().is_none());
        }
        println!(
            "SURFACE_OFFSET_THROUGH_POINT {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":count})
        );
        support::qualify(&op, maximum);
    }
}
