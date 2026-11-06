#[path = "support/plane_surface.rs"]
mod native;
use viboceros_command::{CommandContext, CommandRegistry};

#[test]
fn plane_scripts_match_native_basis_attributes_selection_and_history() {
    let capture = native::read("plane_surface_terminal");
    let guides = native::read("plane_surface_guides");
    let vertical = native::read("plane_surface_vertical_audit");
    assert_eq!(capture["results"].as_array().unwrap().len(), 51);
    let mut completed = 0;
    assert_eq!(guides["results"].as_array().unwrap().len(), 5);
    assert_eq!(vertical["results"].as_array().unwrap().len(), 7);
    for row in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .chain(guides["results"].as_array().unwrap())
        .chain(vertical["results"].as_array().unwrap())
    {
        let v = &row["value"];
        let rejected_option = [
            "deform_linear",
            "audit_degree_one",
            "audit_degree_twelve",
            "audit_low_count",
            "audit_degree_then_count",
        ]
        .contains(&row["id"].as_str().unwrap());
        assert_eq!(
            v["history"].as_str().unwrap().contains("Unknown command"),
            rejected_option
        );
        if v["success"] != true || rejected_option {
            continue;
        }
        let mut doc = native::setup(v);
        let parents = doc.objects().cloned().collect::<Vec<_>>();
        let command = format!("Plane {}", native::tokens(&doc, v).join(" "));
        CommandRegistry::with_builtins()
            .execute_in_context(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: native::frame(v),
                },
            )
            .unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
        let mut error = native::check(&doc, v, "after");
        assert_eq!(doc.undo_label(), Some("Plane"));
        doc.undo().unwrap();
        error = error.max(native::check(&doc, v, "undo"));
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        error = error.max(native::check(&doc, v, "redo"));
        for p in parents {
            assert_eq!(doc.object(p.id()), Some(&p));
            assert!(
                p.geometry_snapshot()
                    .shares_storage_with(doc.object(p.id()).unwrap().geometry_snapshot())
            );
        }
        println!(
            "PLANE_SCRIPT {}",
            serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error})
        );
        assert!(error <= 1e-6, "{} {error:e}", row["id"]);
        completed += 1;
    }
    assert_eq!(completed, 48);
}
