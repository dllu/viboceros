//! Temporary curve references retain native controls, attributes and history.
#[path = "support/extract_points.rs"]
mod native;
use viboceros_command::CommandRegistry;

#[test]
fn extract_point_ranges_replay_native_controls_attributes_and_history() {
    let registry = CommandRegistry::with_builtins();
    for row in native::range_rows() {
        let (mut doc, ids) = native::setup(&row);
        let storage = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        native::select_range_grips(&mut doc, &row);
        registry
            .execute_postselected(
                &mut doc,
                &native::range_command(&row, &ids),
                Default::default(),
            )
            .unwrap();
        let mut stages = Vec::new();
        for stage in ["after", "undo", "redo"] {
            eprintln!("Replay {} {stage}", row["id"]);
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
                        .shares_storage_with(original)
                );
            }
            stages.push(native::snapshot(&doc, &ids));
        }
        println!(
            "EXTRACT_POINTS_RANGE_COMMAND {}",
            serde_json::json!({"id":row["id"],"success":true,"source_storage_unchanged":true,
            "after":stages[0],"undo":stages[1],"redo":stages[2]})
        );
    }
}
