#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn extract_points_selected_grips_replay_native_locations_metadata_and_history() {
    let registry = CommandRegistry::with_builtins();
    let rows = native::rows();
    assert_eq!(rows.len(), 63);
    for row in rows {
        let label = row["id"].as_str().unwrap();
        eprintln!("Replay {label}");
        let (mut doc, sources) = native::setup(&row);
        let storage = sources
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        if row["path"] == "picked" {
            registry
                .execute_postselected(&mut doc, &native::command(&row), Default::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &native::command(&row)).unwrap();
        }
        native::compare(&doc, &sources, &row["value"]["after"]);
        let after = native::snapshot(&doc, &sources);
        assert_eq!(doc.undo_label(), Some("ExtractPt"), "{label}");
        registry.execute(&mut doc, "Undo").unwrap();
        native::compare(&doc, &sources, &row["value"]["undo"]);
        let undo = native::snapshot(&doc, &sources);
        registry.execute(&mut doc, "Redo").unwrap();
        native::compare(&doc, &sources, &row["value"]["redo"]);
        let redo = native::snapshot(&doc, &sources);
        for (id, original) in sources.iter().zip(&storage) {
            assert!(
                doc.object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(original),
                "{label}"
            );
        }
        println!(
            "EXTRACT_POINTS_GRIPS_COMMAND {}",
            serde_json::json!({"id":label,"path":row["path"],"success":true,"source_storage_unchanged":true,"after":after,"undo":undo,"redo":redo})
        );
    }
}
