//! Saved native actions run through the application's command and selection paths.
use super::*;
use crate::app::subd_native_tests as subd_native;
use serde_json::{Value, json};
#[path = "../../../../crates/viboceros-command/tests/support/soft_crease.rs"]
mod fixture;

#[test]
fn soft_crease_getters_replay_all_native_profiles_before_and_after_edge_selection() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for family in [
        "soft_crease_chains",
        "soft_crease_followup",
        "soft_crease_anchors",
    ] {
        let native: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in native["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let value = &row["value"];
            for preselected in [true, false] {
                let (document, objects, targets) = fixture::setup(root, family, id, value);
                let mut app = test_app();
                app.document = document;
                let original = app
                    .document
                    .objects()
                    .map(|o| (o.id(), o.geometry().clone()))
                    .collect::<Vec<_>>();
                app.document.clear_history().unwrap();
                if !preselected {
                    enter(&mut app, "SubDCrease");
                }
                for (oid, edge) in targets {
                    pick(&mut app, oid, &[edge], preselected);
                }
                enter(&mut app, if preselected { "SubDCrease" } else { "" });
                assert!(
                    !app.soft_crease_prompt.as_ref().unwrap().selecting(),
                    "{family}/{id}: {:?}",
                    app.command_log
                );
                for action in value["input"]["actions"].as_array().unwrap() {
                    if let Some(vertex) = action["vertex"].as_u64() {
                        let name = format!("Input {}", action["source"].as_u64().unwrap());
                        let oid = objects.iter().find(|(_, n, _)| *n == name).unwrap().0;
                        let label = *app
                            .soft_crease_prompt
                            .as_ref()
                            .unwrap()
                            .labels
                            .iter()
                            .find(|l| l.object == oid && l.vertex == vertex as usize)
                            .unwrap();
                        assert!(app.handle_viewport_action(ViewportOutput {
                            soft_crease_vertex: Some((label.chain, label.vertex)),
                            ..Default::default()
                        }));
                    } else {
                        let token = action
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| action.as_f64().unwrap().to_string());
                        enter(&mut app, &token);
                    }
                    if app.soft_crease_prompt.is_some() {
                        for (oid, geometry) in &original {
                            assert_eq!(app.document.object(*oid).unwrap().geometry(), geometry);
                        }
                        assert!(!app.document.can_undo());
                    }
                }
                assert!(
                    app.soft_crease_prompt.is_none(),
                    "{family}/{id}: {:?}",
                    app.command_log
                );
                assert!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .is_empty()
                );
                for (oid, name, _) in &objects {
                    let expected = value["after"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| o["name"] == *name)
                        .unwrap();
                    let Geometry::SubD(actual) = app.document.object(*oid).unwrap().geometry()
                    else {
                        panic!()
                    };
                    let check = subd_native::compare(actual, &expected["subd"]);
                    println!(
                        "SOFT_UI {}",
                        json!({"family":family,"id":id,"object":name,"preselected":preselected,"check":check,"staged":true,"selection_released":true})
                    );
                }
                let changed = original.iter().any(|(oid, geometry)| {
                    app.document.object(*oid).unwrap().geometry() != geometry
                });
                assert_eq!(app.document.undo_label() == Some("SubDCrease"), changed);
                if changed {
                    enter(&mut app, "Undo");
                    for (oid, geometry) in &original {
                        assert_eq!(app.document.object(*oid).unwrap().geometry(), geometry);
                    }
                    assert_eq!(app.document.selected_control_points().count(), 0);
                    enter(&mut app, "Redo");
                    for (oid, name, _) in &objects {
                        let expected = value["redo"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|o| o["name"] == *name)
                            .unwrap();
                        let Geometry::SubD(actual) = app.document.object(*oid).unwrap().geometry()
                        else {
                            panic!()
                        };
                        subd_native::compare(actual, &expected["subd"]);
                    }
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 96);
}
