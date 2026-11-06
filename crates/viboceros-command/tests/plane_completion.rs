#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod baseline;
#[path = "support/plane_completion.rs"]
#[allow(dead_code, unused_imports)]
mod native;
use viboceros_command::{CommandContext, CommandRegistry};

#[test]
fn plane_completion_scripts_use_recorded_pointer_rays_and_native_defaults() {
    let data = native::read("plane_completion_terminal");
    let mut count = 0;
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["success"] != true || v["history"].as_str().unwrap().contains("Unknown command") {
            continue;
        }
        let mut doc = native::setup(v);
        let parents = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        assert!(registry.remember_plane_preferences(native::preferences(v)));
        let command = format!("Plane {}", native::tokens(&doc, v).join(" "));
        registry
            .execute_with_pick_projection(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: native::frame(v),
                },
                false,
                native::projection(v),
            )
            .unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
        let mut error = native::check(&doc, v, "after");
        assert_eq!(doc.undo_label(), Some("Plane"));
        doc.undo().unwrap();
        error = error.max(native::check(&doc, v, "undo"));
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        error = error.max(native::check(&doc, v, "redo"));
        for parent in parents {
            assert_eq!(doc.object(parent.id()), Some(&parent));
            assert!(
                parent
                    .geometry_snapshot()
                    .shares_storage_with(doc.object(parent.id()).unwrap().geometry_snapshot())
            );
        }
        println!(
            "PLANE_COMPLETION_SCRIPT {}",
            serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error})
        );
        assert!(error <= 1e-6, "{} {error:e}", row["id"]);
        count += 1;
    }
    assert_eq!(count, 39);
}
