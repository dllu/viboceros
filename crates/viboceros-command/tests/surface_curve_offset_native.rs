#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/surface_curve_offset.rs"]
mod support;
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
