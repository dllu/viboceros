//! Same-source command results are compared against independently captured Rhino.
use viboceros_command::{
    CommandRegistry,
    chamfer::{self},
};
#[path = "support/fillet_getter.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;

#[path = "support/chamfer_getter.rs"]
#[allow(dead_code)]
mod getters;

#[test]
fn same_source_matches_native_geometry_identity_metadata_and_history() {
    let mut count = 0;
    for family in [
        "chamfer_self_pilot",
        "chamfer_self_discovery",
        "chamfer_self_holdouts",
        "chamfer_self_spans",
    ] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let options = getters::final_options(value);
            assert!(!inputs::diagnostic(value));
            if !inputs::success(value) {
                continue;
            }
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            println!("CHAMFER_SELF_CASE {id}");
            let mut doc = inputs::setup(value);
            let state = inputs::State::capture(&doc);
            let picks = inputs::picks(&doc, value);
            inputs::check(&doc, &value["before"], false);
            let before = format!("{doc:?}");
            let prepared = chamfer::prepare(&doc, picks, options).unwrap();
            assert_eq!(format!("{doc:?}"), before);
            let mut cached = doc.clone();
            prepared.commit(&mut cached).unwrap();
            inputs::check(&cached, &value["after"], false);
            let point =
                |p: viboceros_geometry::Point3| p.to_array().map(|v| v.to_string()).join(",");
            let mut selected = doc.clone();
            selected.set_transient_selection([picks[0].0]).unwrap();
            CommandRegistry::with_builtins()
                .execute(
                    &mut selected,
                    &format!(
                        "Chamfer Pick1={} Pick2={} {}",
                        point(picks[0].1),
                        point(picks[1].1),
                        options.arguments()
                    ),
                )
                .unwrap();
            inputs::check(&selected, &value["after"], false);
            CommandRegistry::with_builtins()
                .execute(
                    &mut doc,
                    &format!(
                        "Chamfer Sources={},{} Pick1={} Pick2={} {}",
                        picks[0].0,
                        picks[1].0,
                        point(picks[0].1),
                        point(picks[1].1),
                        options.arguments()
                    ),
                )
                .unwrap();
            let mut result = state.verify(&mut doc, value);
            result["id"] = serde_json::json!(id);
            println!("CHAMFER_SELF_COMMAND {result}");
            count += 1;
        }
    }
    assert_eq!(count, 35);
}

#[test]
fn native_rejections_are_atomic_and_preserve_pending_redo() {
    let mut rejected = 0;
    for family in [
        "chamfer_self_pilot",
        "chamfer_self_discovery",
        "chamfer_self_holdouts",
        "chamfer_self_spans",
    ] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let event = value["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == "Chamfer")
                .unwrap();
            if event["result"] != "Failure" {
                continue;
            }
            assert_eq!(value["history_changed"], false);
            let mut doc = inputs::setup(value);
            let picks = inputs::picks(&doc, value);
            let options = getters::final_options(value);
            let registry = CommandRegistry::with_builtins();
            if !doc.can_redo() {
                registry.execute(&mut doc, "Point 90,91,92").unwrap();
                doc.undo().unwrap();
            }
            let before = format!("{doc:?}");
            assert!(
                chamfer::prepare(&doc, picks, options).is_err(),
                "{family}/{}",
                row["id"]
            );
            assert_eq!(format!("{doc:?}"), before);
            let point =
                |p: viboceros_geometry::Point3| p.to_array().map(|v| v.to_string()).join(",");
            assert!(
                registry
                    .execute(
                        &mut doc,
                        &format!(
                            "Chamfer Sources={},{} Pick1={} Pick2={} {}",
                            picks[0].0,
                            picks[1].0,
                            point(picks[0].1),
                            point(picks[1].1),
                            options.arguments()
                        )
                    )
                    .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
            assert!(doc.can_redo());
            println!(
                "CHAMFER_SELF_REJECTION {}",
                serde_json::json!({"id":format!("{family}/{}",row["id"].as_str().unwrap()),"redo_preserved":true})
            );
            rejected += 1;
        }
    }
    assert_eq!(rejected, 10);
}
