#[path = "support/surface_interpolation.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn surface_interpolation_invalid_inputs_preserve_geometry_selection_and_redo() {
    let (_, v) = native::rows().remove(0);
    let (mut doc, target) = native::setup(&v);
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "InterpCrvOnSrf 0,0,0 1,1,0")
        .unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for input in [
        "InterpCrvOnSrf 0,0,0 0,0,0",
        "InterpCrvOnSrf 0,0,0 1,1,0 Close",
        "InterpCrvOnSrf 0,0,0 1,1,0 AutoClose=Maybe",
        "InterpCrvOnSrf 0,0,0 1,1,0 Face=4",
        "InterpCrvOnSrf 0,0,0 1,1,0 Surface=invalid",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(format!("{doc:?}"), before);
        assert!(doc.is_selected(target));
    }
    assert_eq!(doc.redo_label(), Some("InterpCrvOnSrf"));
}

#[test]
fn surface_interpolation_scripts_match_native_geometry_metadata_and_history() {
    for (op, v) in native::rows() {
        // This reference build reports both newer options as unknown commands.
        if matches!(op["case"].as_str(), Some("persistent" | "autoclose_no")) {
            println!(
                "SURFACE_INTERPOLATION_COMMAND {}",
                serde_json::json!({"id":op["id"],"native_supported":false,"qualified":false})
            );
            continue;
        }
        let (mut doc, target) = native::setup(&v);
        let parent = doc.object(target).unwrap().geometry_snapshot().clone();
        let mut points = Vec::new();
        let mut close = false;
        let mut cancel = false;
        for action in op["actions"].as_array().unwrap() {
            if let Some(p) = action.as_array().or_else(|| action["click"].as_array()) {
                points.push(
                    p.iter()
                        .map(|x| x.as_f64().unwrap().to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            } else {
                match action.as_str().unwrap() {
                    "Undo" => {
                        points.pop();
                    }
                    "Close" => close = true,
                    "Cancel" => cancel = true,
                    "Enter" => (),
                    other => panic!("unexpected native action {other}"),
                }
            }
        }
        let mut maximum = 0_f64;
        if cancel {
            maximum = maximum.max(native::check(&doc, target, &v, "after_script"));
            assert!(!doc.can_undo());
        } else {
            CommandRegistry::with_builtins()
                .execute(
                    &mut doc,
                    &format!(
                        "InterpCrvOnSrf {} {}",
                        points.join(" "),
                        if close { "Close" } else { "" }
                    ),
                )
                .unwrap();
            maximum = maximum.max(native::check(&doc, target, &v, "after_script"));
            assert!(parent.shares_storage_with(doc.object(target).unwrap().geometry_snapshot()));
            assert_eq!(doc.undo_label(), Some("InterpCrvOnSrf"));
            doc.undo().unwrap();
            maximum = maximum.max(native::check(&doc, target, &v, "undo"));
            doc.redo().unwrap();
            maximum = maximum.max(native::check(&doc, target, &v, "redo"));
        }
        println!(
            "SURFACE_INTERPOLATION_COMMAND {}",
            serde_json::json!({"id":op["id"],"native_supported":true,"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert!(maximum < 1e-6, "{} native error {maximum:e}", op["id"]);
    }
}
