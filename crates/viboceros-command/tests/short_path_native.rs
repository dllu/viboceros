#[path = "support/short_path.rs"]
mod native;
use viboceros_command::CommandRegistry;
#[test]
fn short_path_command_keeps_native_selection_layers_and_history() {
    for (op, v) in native::rows()
        .into_iter()
        .filter(|(op, _)| op["case"] != "cancel")
    {
        let (mut doc, target) = native::setup(&v);
        let parent = doc.object(target).unwrap().geometry_snapshot().clone();
        let point = |i: usize| {
            op["actions"][i]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("ShortPath {} {}", point(0), point(1)))
            .unwrap();
        let mut maximum = native::check(&doc, target, &v, "after_script");
        assert!(parent.shares_storage_with(doc.object(target).unwrap().geometry_snapshot()));
        assert_eq!(doc.undo_label(), Some("ShortPath"));
        doc.undo().unwrap();
        maximum = maximum.max(native::check(&doc, target, &v, "undo"));
        doc.redo().unwrap();
        maximum = maximum.max(native::check(&doc, target, &v, "redo"));
        println!(
            "SHORT_PATH_COMMAND {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert_eq!(maximum < 1e-6, op["case"] == "plane");
        assert!(maximum < 0.001);
    }
}

#[test]
fn short_path_trim_boundaries_match_the_native_underlying_surface_policy() {
    for (op, v) in native::boundary_rows() {
        let (mut doc, target) = native::setup_boundary(&op, &v);
        let parent = doc.object(target).unwrap().geometry_snapshot().clone();
        let point = |i: usize| {
            op["actions"][i]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        CommandRegistry::with_builtins()
            .execute(&mut doc, &format!("ShortPath {} {}", point(0), point(1)))
            .unwrap();
        let mut maximum = native::check(&doc, target, &v, "after_script");
        assert!(parent.shares_storage_with(doc.object(target).unwrap().geometry_snapshot()));
        assert_eq!(doc.undo_label(), Some("ShortPath"));
        doc.undo().unwrap();
        maximum = maximum.max(native::check(&doc, target, &v, "undo"));
        doc.redo().unwrap();
        maximum = maximum.max(native::check(&doc, target, &v, "redo"));
        println!(
            "SHORT_PATH_BOUNDARY_COMMAND {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert!(maximum < 1e-6);
    }
}
