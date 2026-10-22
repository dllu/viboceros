//! Full command replay from immutable native inputs, including metadata/history.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{Object, SelectionMode};
#[path = "support/control_polygon.rs"]
mod native;
use native::compare;

#[test]
fn native_extraction_replays_geometry_attributes_mixed_selection_and_history() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = CommandRegistry::with_builtins();
    let (mut cases, mut successes) = (0, 0);
    for family in [
        "control_polygon",
        "control_polygon_options",
        "control_polygon_priority",
    ] {
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in native["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["history"].as_str().unwrap().contains("Unknown") {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            let context = format!("{family}/{id}");
            let (mut document, sources) = native::setup(family, id, v, &context);
            if v["input"]["preselected"] == false {
                let filter = registry
                    .object_selection_prompt("ExtractControlPolygon")
                    .unwrap()
                    .unwrap()
                    .filter;
                let selected = document
                    .objects()
                    .filter(|o| filter.accepts_object(o))
                    .map(Object::id)
                    .collect::<Vec<_>>();
                document
                    .select_objects_direct(selected, SelectionMode::Replace)
                    .unwrap();
            }
            document.clear_history().unwrap();
            let original = document
                .objects()
                .map(|o| {
                    (
                        o.id(),
                        o.geometry_snapshot().clone(),
                        o.attributes().clone(),
                        o.group_ids().to_vec(),
                    )
                })
                .collect::<Vec<_>>();
            let command = format!(
                "ExtractControlPolygon OutputLayer={} SubDPriority={}",
                v["input"]["layer"].as_str().unwrap(),
                v["input"]["priority"].as_str().unwrap_or("Geometry")
            );
            let result = if v["input"]["preselected"] == false {
                registry.execute_postselected(&mut document, &command, CommandContext::default())
            } else {
                registry.execute(&mut document, &command)
            };
            assert_eq!(
                result.is_ok(),
                v["success"].as_bool().unwrap(),
                "{context}: {result:?}"
            );
            let error = compare(&document, &sources, &v["after"], &context);
            for (id, geometry, attrs, groups) in &original {
                let o = document.object(*id).unwrap();
                assert_eq!(o.geometry_snapshot(), geometry);
                assert_eq!(o.attributes(), attrs);
                assert_eq!(o.group_ids(), groups);
            }
            if result.is_ok() {
                assert_eq!(document.undo_label(), Some("ExtractControlPolygon"));
                registry.execute(&mut document, "Undo").unwrap();
                compare(&document, &sources, &v["undo"], &context);
                registry.execute(&mut document, "Redo").unwrap();
                compare(&document, &sources, &v["redo"], &context);
                successes += 1;
            } else {
                assert!(document.undo_label().is_none());
            }
            println!(
                "CONTROL_POLYGON_NATIVE {}",
                json!({"family":family,"id":id,"success":result.is_ok(),"maximum_error":error,"geometry_unchanged":true,"metadata_exact":true,"selection_exact":true,"history_exact":true,"qualified":true})
            );
            cases += 1;
        }
    }
    assert_eq!((cases, successes), (42, 32));
}
