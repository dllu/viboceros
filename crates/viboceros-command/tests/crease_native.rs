#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use native::*;
use serde_json::json;
use viboceros_command::{
    CommandRegistry,
    crease::{CreaseSelection, CreaseTarget},
};
use viboceros_document::SelectionMode;

#[test]
fn staged_crease_edits_match_native_controls_facets_attributes_order_and_undo() {
    let rows = rows();
    assert_eq!(rows.len(), 110);
    let mut failures = Vec::new();
    for row in rows {
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            let (mut doc, ids) = setup(&row);
            let mut maximum = check(&doc, &v["before"], true);
            let original = ids
                .iter()
                .map(|&id| doc.object(id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            let prepared = CreaseSelection::prepare(&doc, mode(v), targets(&doc, &ids, v)).unwrap();
            let out = prepared.commit(&mut doc).unwrap();
            for (&id, attributes) in ids.iter().zip(&original) {
                assert_eq!(doc.object(id).unwrap().attributes(), attributes);
            }
            maximum = maximum.max(check(&doc, &v["after"], true));
            let native_changed = v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["name"] == "Undo witness");
            assert_eq!(!out.processed_objects.is_empty(), native_changed);
            assert_eq!(
                doc.undo_label(),
                Some(if native_changed {
                    mode(v).command()
                } else {
                    "Undo witness"
                })
            );
            // The producer appends Cancel after observing command completion.
            // Rhino turns grips off there; Undo restores the command's input
            // grips, while Redo restores this post-cleanup display state.
            if v["input"]["grips"] == true {
                doc.disable_control_points();
            }
            maximum = maximum.max(check(&doc, &v["after_script"], true));
            doc.undo().unwrap();
            maximum = maximum.max(check(&doc, &v["undo"], true));
            doc.redo().unwrap();
            maximum = maximum.max(check(&doc, &v["redo"], true));
            println!(
                "CREASE_COMMAND {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"processed_objects":out.processed_objects.len(),"native_empty_ngon_slots":v["after"].as_array().unwrap().iter().flat_map(|r|r["mesh"]["ngons"].as_array().into_iter().flatten()).filter(|n|n["vertices"].as_array().unwrap().is_empty()&&n["faces"].as_array().unwrap().is_empty()).count()})
            );
        }));
        if run.is_err() {
            failures.push(row["id"].as_str().unwrap().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "native crease differences: {failures:?}"
    );
}

#[test]
fn typed_crease_components_use_the_registered_commands_and_atomic_history() {
    for name in [
        "main_crease_subd_edge",
        "main_removecrease_subd_creased_edge",
        "main_crease_subd_vertex",
        "main_removecrease_subd_corner",
        "main_crease_mesh_edge",
        "main_removecrease_mesh_seam",
    ] {
        let row = rows().into_iter().find(|r| r["id"] == name).unwrap();
        let v = &row["value"];
        let (mut doc, ids) = setup(&row);
        let selected = targets(&doc, &ids, v);
        let edges = selected
            .iter()
            .filter_map(|t| match t {
                CreaseTarget::Edge(_, i) => Some(i.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let vertices = selected
            .iter()
            .filter_map(|t| match t {
                CreaseTarget::Vertex(_, i) => Some(i.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let mut command = mode(v).command().to_string();
        if !edges.is_empty() {
            command.push_str(&format!(" Edges={}", edges.join(",")));
        }
        if !vertices.is_empty() {
            command.push_str(&format!(" Vertices={}", vertices.join(",")));
        }
        CommandRegistry::with_builtins()
            .execute(&mut doc, &command)
            .unwrap();
        check(&doc, &v["after"], true);
        doc.undo().unwrap();
        check(&doc, &v["undo"], false);
        doc.redo().unwrap();
        check(&doc, &v["redo"], true);
    }
}
