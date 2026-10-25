//! Whole SubD locations come from independent Rhino command output.
#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn extract_points_whole_subds_replay_native_locations_metadata_and_history() {
    let registry = CommandRegistry::with_builtins();
    for row in native::whole_subd_rows() {
        let label = row["id"].as_str().unwrap();
        eprintln!("Replay {label}");
        let (mut doc, ids) = native::setup_whole_subd(&row);
        let storage = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let input = native::whole_subd_command(&row);
        if row["path"] == "picked" {
            registry
                .execute_postselected(&mut doc, &input, Default::default())
                .unwrap();
        } else {
            registry.execute(&mut doc, &input).unwrap();
        }
        assert_eq!(doc.undo_label(), Some("ExtractPt"));
        let mut stages = Vec::new();
        for stage in ["after", "undo", "redo"] {
            if stage != "after" {
                registry
                    .execute(&mut doc, if stage == "undo" { "Undo" } else { "Redo" })
                    .unwrap();
            }
            native::compare(&doc, &ids, &row["value"][stage]);
            for (id, original) in ids.iter().zip(&storage) {
                assert!(
                    doc.object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(original),
                    "{label} {stage}"
                );
            }
            stages.push(native::snapshot(&doc, &ids));
        }
        println!(
            "EXTRACT_POINTS_SUBD_COMMAND {}",
            serde_json::json!({"id":label,"path":row["path"],
            "success":true,"source_storage_unchanged":true,"after":stages[0],"undo":stages[1],"redo":stages[2]})
        );
    }
}
