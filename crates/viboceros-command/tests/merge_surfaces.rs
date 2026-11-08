#[path = "support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
use captured::*;
use serde_json::json;
use viboceros_command::merge_surfaces::merge;

#[test]
fn automatic_support_orientation_and_topology_match_public_sdk() {
    let rows = sdk_rows();
    assert_eq!(rows.len(), 40);
    for row in rows {
        let id = row["id"].as_str().unwrap();
        println!("MERGE_SURFACE_GEOMETRY_CASE {id}");
        let value = &row["value"];
        let recipe = &value["prescribed_case"];
        let first = input(&recipe["sources"][0], model_tolerance());
        let second = input(&recipe["sources"][1], model_tolerance());
        let before = (first.clone(), second.clone());
        let result = merge(&first, &second, options(recipe), model_tolerance());
        assert_eq!((first, second), before, "{id}");
        if value["success"] == false {
            assert!(result.is_err(), "{id}");
            println!(
                "MERGE_SURFACE_GEOMETRY {}",
                json!({"id":id,"success":false})
            );
            continue;
        }
        let result = result.unwrap_or_else(|e| panic!("{id}: {e}"));
        let error = compare(&result, &value["output"]);
        let restored = archive(&result);
        let error = error.max(compare(&restored, &value["output"]));
        println!(
            "MERGE_SURFACE_GEOMETRY {}",
            json!({"id":id,"success":true,"maximum_geometry_error":error,"archives":1})
        );
    }
}

#[path = "support/merge_surface_command.rs"]
mod workflow;

#[test]
fn registry_command_identity_metadata_history_and_archives_match_native() {
    use viboceros_command::{CommandContext, CommandRegistry, merge_surfaces::Options};
    use viboceros_document::SelectionMode;
    let mut count = 0;
    for family in workflow::families() {
        let registry = CommandRegistry::with_builtins();
        let mut defaults = Options::default();
        for row in family {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let mut d = workflow::setup(v);
            let state = workflow::State::capture(&d);
            defaults = workflow::options(v, defaults, &d);
            registry.remember_merge_surface_options(defaults).unwrap();
            if workflow::outcome(v) == "Cancel" {
                continue;
            }
            println!("MERGE_SURFACE_API_CASE {id}");
            workflow::check(&d, &v["before"], false);
            let postselected = v["prescribed_case"]["preselected"] == false;
            if postselected {
                let ids = v["prescribed_case"]["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|a| a["curve"].as_u64())
                    .map(|i| workflow::source(&d, i as usize))
                    .collect::<Vec<_>>();
                d.clear_selection();
                for id in ids {
                    d.select_objects_direct([id], SelectionMode::Add).unwrap();
                }
            }
            let result = if postselected {
                registry.execute_postselected(&mut d, "MergeSrf", CommandContext::default())
            } else {
                registry.execute(&mut d, "MergeSrf")
            };
            assert_eq!(result.is_ok(), v["success"] == true, "{id}: {result:?}");
            let mut record = state.verify(&mut d, v);
            record["id"] = id.into();
            println!("MERGE_SURFACE_API {record}");
            count += 1;
        }
    }
    assert_eq!(count, 15);
}

#[test]
fn prepared_merge_rejects_stale_inputs_without_consuming_redo() {
    use viboceros_command::merge_surfaces::{Options, prepare};
    use viboceros_document::Geometry;
    let row = &workflow::families()[0][0];
    let v = &row["value"];
    let base = workflow::setup(v);
    let original = base.objects().cloned().collect::<Vec<_>>();
    let staged = prepare(&base, Options::default(), false).unwrap();
    assert!(staged.is_current(&base));
    assert_eq!(base.objects().cloned().collect::<Vec<_>>(), original);
    assert!(!base.can_undo());
    assert!(base.can_redo());
    for edit in 0..4 {
        let mut d = base.clone();
        match edit {
            0 => {
                d.clear_selection();
            }
            1 => {
                d.set_tolerance(viboceros_geometry::Tolerance::DEFAULT);
            }
            2 => {
                let id = workflow::source(&d, 0);
                d.replace_object_geometries([(
                    id,
                    Geometry::NurbsSurface(surface(&v["prescribed_case"]["curves"][1])),
                )])
                .unwrap();
            }
            _ => {
                d.set_object_geometry_user_text(
                    [workflow::source(&d, 0)],
                    "revision",
                    Some("changed"),
                )
                .unwrap();
            }
        }
        let objects = d.objects().cloned().collect::<Vec<_>>();
        let history = (d.can_undo(), d.can_redo());
        assert!(!staged.is_current(&d));
        assert!(staged.commit(&mut d).is_err());
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!((d.can_undo(), d.can_redo()), history);
    }
    let mut d = base;
    staged.commit(&mut d).unwrap();
    assert_eq!(d.undo_label(), Some("MergeSrf"));
    d.undo().unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), original);
}
