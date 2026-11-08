//! Native open-curve contacts, including oversized cuts and actual history.
use viboceros_command::{CommandRegistry, chamfer};
#[path = "support/chamfer_getter.rs"]
#[allow(dead_code)]
mod getters;
#[path = "support/fillet_getter.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;

#[test]
fn open_contacts_match_native_geometry_metadata_identity_history_and_archives() {
    let mut checked = 0;
    for family in [
        "chamfer_open_pilot",
        "chamfer_open_holdouts",
        "chamfer_open_curves",
        "chamfer_open_choices",
        "chamfer_open_overlaps",
    ] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            println!("CHAMFER_OPEN_CASE {id}");
            checked += 1;
            if value["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["name"] == "Chamfer" && e["result"] == "Cancel")
            {
                continue;
            }
            let mut doc = inputs::setup(value);
            let source_error = inputs::check(&doc, &value["before"], false);
            let state = inputs::State::capture(&doc);
            let picks = inputs::picks(&doc, value);
            let options = getters::final_options(value);
            let before = format!("{doc:?}");
            let point =
                |p: viboceros_geometry::Point3| p.to_array().map(|v| v.to_string()).join(",");
            let command = format!(
                "Chamfer Sources={},{} Pick1={} Pick2={} {}",
                picks[0].0,
                picks[1].0,
                point(picks[0].1),
                point(picks[1].1),
                options.arguments()
            );
            let invalid = value["after"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["native_valid"] == false);
            if !inputs::success(value) || invalid {
                assert!(chamfer::prepare(&doc, picks, options).is_err(), "{id}");
                assert_eq!(format!("{doc:?}"), before);
                assert!(
                    CommandRegistry::with_builtins()
                        .execute(&mut doc, &command)
                        .is_err(),
                    "{id}"
                );
                assert_eq!(format!("{doc:?}"), before);
                assert!(doc.can_redo());
                println!(
                    "CHAMFER_OPEN_REJECTION {}",
                    serde_json::json!({"id":id,"redo_preserved":true,"native_invalid":invalid})
                );
            } else {
                let prepared = chamfer::prepare(&doc, picks, options).unwrap();
                assert_eq!(format!("{doc:?}"), before);
                let mut cached = doc.clone();
                prepared.commit(&mut cached).unwrap();
                inputs::check(&cached, &value["after"], false);
                CommandRegistry::with_builtins()
                    .execute(&mut doc, &command)
                    .unwrap();
                let mut result = state.verify(&mut doc, value);
                result["maximum_geometry_error"] = serde_json::json!(
                    source_error.max(result["maximum_geometry_error"].as_f64().unwrap())
                );
                result["id"] = serde_json::json!(id);
                println!("CHAMFER_OPEN_COMMAND {result}");
            }
        }
    }
    assert_eq!(checked, 69);
}
