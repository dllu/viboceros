#![cfg(feature = "native-smlib")]
use super::frozen_scene;
use super::mesh_boolean_tests::native as boolean;
use super::mesh_split_tests::native as split;
use super::*;
use crate::app::tests::test_app;
use serde_json::json;
#[allow(dead_code)]
#[path = "../../../crates/viboceros-command/tests/support/mesh_trim.rs"]
mod native;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn start(app: &mut VibocerosApp, row: &serde_json::Value, ids: &[ObjectId], preselect: bool) {
    let cutters = native::cutters(row, ids);
    if preselect {
        app.document
            .select_objects_direct(cutters.clone(), SelectionMode::Replace)
            .unwrap();
    }
    enter(app, "MeshTrim");
    assert!(app.intersection_prompt.is_some(), "{:?}", app.command_log);
    if !preselect {
        app.select_intersection_prompt_objects(cutters, SelectionMode::Add);
        if row["value"]["input"]["infinite_plane"].is_string() {
            for word in row["value"]["input"]["cutting_arguments"]
                .as_str()
                .unwrap()
                .split_whitespace()
            {
                enter(app, word);
            }
        }
        enter(app, "");
    }
    assert!(app.picking_mesh_trim_target(), "{:?}", app.command_log);
}
#[test]
fn mesh_trim_getters_replay_native_physical_picks_cancel_channels_and_history() {
    let (count, gaps) = replay(native::rows(), "MESH_TRIM_GETTER");
    assert_eq!(count, 14);
    assert_eq!(gaps, native::GAPS);
}
#[test]
fn mesh_trim_getters_replay_native_captured_cutter_continuations() {
    let (count, gaps) = replay(
        native::rows_for(&["continuation"]),
        "MESH_TRIM_CONTINUATION_GETTER",
    );
    assert_eq!(count, 6);
    assert!(gaps.is_empty(), "{gaps:?}");
}
#[test]
fn mesh_trim_getters_replay_native_original_cutter_geometry() {
    let (count, gaps) = replay(
        native::rows_for(&["cutter_policy"]),
        "MESH_TRIM_CUTTER_POLICY_GETTER",
    );
    assert_eq!(count, 2);
    assert!(gaps.is_empty(), "{gaps:?}");
}
#[test]
fn mesh_trim_getters_replay_native_preselection_and_sdk_controls() {
    let (count, gaps) = replay(
        native::all_rows_for(&["getter_controls"]),
        "MESH_TRIM_GETTER_CONTROLS_GETTER",
    );
    assert_eq!(count, 5);
    assert_eq!(gaps, native::GETTER_CONTROL_GAPS);
}
#[test]
fn mesh_trim_getters_replay_default_native_preselection() {
    let (count, gaps) = replay(
        native::all_rows_for(&["verified_getters"]),
        "MESH_TRIM_VERIFIED_GETTERS_GETTER",
    );
    assert_eq!(count, 2);
    assert!(gaps.is_empty(), "{gaps:?}");
}
#[test]
fn mesh_trim_getters_replay_native_direct_plane_differences() {
    let (count, gaps) = replay(
        native::all_rows_for(&["plane_getters"]),
        "MESH_TRIM_PLANE_GETTERS_GETTER",
    );
    assert_eq!(count, 2);
    assert_eq!(gaps, native::PLANE_GETTER_GAPS);
}
#[test]
fn mesh_trim_getters_expose_native_perspective_curve_surface_difference() {
    let (count, gaps) = replay(
        native::rows_for(&["snapshot_policy"]),
        "MESH_TRIM_SNAPSHOT_POLICY_GETTER",
    );
    assert_eq!(count, 2);
    assert_eq!(gaps, native::SNAPSHOT_GAPS);
}
fn replay(rows: Vec<serde_json::Value>, prefix: &str) -> (usize, Vec<String>) {
    let mut gaps = vec![];
    let mut count = 0;
    for row in rows {
        let value = &row["value"];
        let mut scene = native::setup(&row);
        let mut app = test_app();
        app.document = std::mem::take(&mut scene.doc);
        start(
            &mut app,
            &row,
            &scene.ids,
            value["input"]["preselect"] == true,
        );
        for (index, point) in native::picks(&row).into_iter().enumerate() {
            let target = native::target_for_pick(&app.document, &row, index, point);
            app.accept_component_face_hit(target, 0, Some(point));
            assert!(app.picking_mesh_trim_target());
        }
        let success = value["input"]["finish"] != "Cancel";
        if success {
            enter(&mut app, "");
        } else {
            app.cancel_interactive_command(true);
        }
        assert!(app.intersection_prompt.is_none());
        let error = app
            .command_log
            .iter()
            .find_map(|s| s.strip_prefix("Error: "))
            .map(str::to_owned);
        assert!(error.is_none(), "{}: {:?}", row["id"], app.command_log);
        scene.doc = std::mem::take(&mut app.document);
        let after = split::check_with_channels(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = split::check_with_channels(&scene, value, "undo");
        let ownership = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = split::check_with_channels(&scene, value, "redo");
        let measured = json!({"id":row["id"],"success":success,"error":error,
            "after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":ownership});
        println!("{prefix} {measured}");
        assert!(ownership);
        assert_eq!(success, value["success"].as_bool().unwrap());
        if ["after", "undo", "redo"].iter().any(|s| {
            ["geometry", "metadata", "ngons"]
                .iter()
                .any(|k| measured[s][k] != true)
        }) {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        count += 1;
    }
    gaps.sort();
    (count, gaps)
}
#[test]
fn mesh_trim_preselected_cutters_and_local_undo_preserve_one_command_history() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh_trim_controls_both_sides")
        .unwrap();
    let mut scene = native::setup(&row);
    let mut app = test_app();
    app.document = std::mem::take(&mut scene.doc);
    start(&mut app, &row, &scene.ids, true);
    let picks = native::picks(&row);
    app.accept_component_face_hit(scene.ids[0], 0, Some(picks[0]));
    let retained = native::target(&app.document, picks[1]);
    assert_ne!(retained, scene.ids[0]);
    app.accept_component_face_hit(retained, 0, Some(picks[1]));
    assert!(
        app.document
            .objects()
            .all(|o| o.attributes().name() != Some("Source 0"))
    );
    enter(&mut app, "Undo");
    assert!(app.document.object(retained).is_some());
    enter(&mut app, "Undo");
    assert!(app.document.object(scene.ids[0]).is_some());
    // Another Undo belongs to the getter and must leave the earlier witness alone.
    enter(&mut app, "Undo");
    assert!(app.document.object(scene.witness).is_some());
    app.accept_component_face_hit(scene.ids[0], 0, Some(picks[0]));
    app.cancel_interactive_command(true);
    assert_eq!(app.document.undo_label(), Some("MeshTrim"));
    app.document.undo().unwrap();
    scene.doc = std::mem::take(&mut app.document);
    assert!(boolean::sources_restored(&scene));
    assert_eq!(scene.doc.selected_object_count(), 0);
    assert!(scene.doc.object(scene.witness).is_some());
}

#[test]
fn mesh_trim_cutter_descendants_survive_failed_pick_and_local_undo() {
    let row = native::rows_for(&["continuation"])
        .into_iter()
        .find(|r| r["id"] == "mesh_trim_continuation_shared_two_cutters")
        .unwrap();
    let mut scene = native::setup(&row);
    let mut app = test_app();
    app.document = std::mem::take(&mut scene.doc);
    start(&mut app, &row, &scene.ids, false);
    let points = native::picks(&row);
    app.accept_component_face_hit(scene.ids[0], 0, Some(points[0]));
    let retained = native::target(&app.document, points[1]);
    let storage = app
        .document
        .objects()
        .map(|o| (o.id(), o.geometry_snapshot().clone()))
        .collect::<Vec<_>>();
    // The old target ID is gone. A failed pick must leave the accepted step and
    // its input snapshots usable, without adding a local Undo checkpoint.
    app.accept_component_face_hit(scene.ids[0], 0, Some(points[1]));
    assert_eq!(app.document.objects().len(), storage.len());
    for (id, geometry) in storage {
        assert!(
            app.document
                .object(id)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&geometry)
        );
    }
    assert_eq!(
        app.command_log
            .iter()
            .filter(|s| s.starts_with("Error: "))
            .count(),
        1
    );
    app.accept_component_face_hit(retained, 0, Some(points[1]));
    enter(&mut app, "Undo");
    assert!(app.document.object(retained).is_some());
    app.accept_component_face_hit(retained, 0, Some(points[1]));
    app.cancel_interactive_command(true);
    scene.doc = std::mem::take(&mut app.document);
    let check = split::check_with_channels(&scene, &row["value"], "after");
    assert_eq!(check["geometry"], true);
    assert_eq!(check["metadata"], true);
    scene.doc.undo().unwrap();
    assert!(boolean::sources_restored(&scene));
    assert_eq!(scene.doc.undo_label(), Some("Undo witness"));
}

#[test]
fn mesh_trim_explicit_script_picks_bypass_interactive_selection() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh_trim_controls_sheet_left_post")
        .unwrap();
    let mut scene = native::setup(&row);
    let mut app = test_app();
    app.document = std::mem::take(&mut scene.doc);
    let command = native::command(
        &row,
        &native::cutters(&row, &scene.ids),
        scene.ids[0],
        native::picks(&row)[0],
    );
    enter(&mut app, &command);
    assert!(app.intersection_prompt.is_none() && app.object_prompt.is_none());
    assert!(
        !app.command_log.iter().any(|s| s.starts_with("Error: ")),
        "{:?}",
        app.command_log
    );
    scene.doc = std::mem::take(&mut app.document);
    let check = split::check_with_channels(&scene, &row["value"], "after");
    assert_eq!(check["metadata"], true);
    assert_eq!(check["geometry"], true);
}

#[test]
fn mesh_trim_empty_cancel_getters_match_native_selection_and_history() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/observations/mesh_trim_discovery.json");
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for name in ["mesh_trim_cancel_first", "mesh_trim_cancel_second"] {
        let mut row = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == name)
            .unwrap()
            .clone();
        row["family"] = "discovery".into();
        let value = &row["value"];
        let mut scene = native::setup(&row);
        let mut app = test_app();
        app.document = std::mem::take(&mut scene.doc);
        enter(&mut app, "MeshTrim");
        app.select_intersection_prompt_objects(
            native::cutters(&row, &scene.ids),
            SelectionMode::Add,
        );
        if name.ends_with("second") {
            enter(&mut app, "");
        }
        app.cancel_interactive_command(true);
        assert!(app.intersection_prompt.is_none());
        scene.doc = std::mem::take(&mut app.document);
        for stage in ["after", "undo", "redo"] {
            if stage == "undo" {
                scene.doc.undo().unwrap();
            }
            if stage == "redo" {
                scene.doc.redo().unwrap();
            }
            let check = split::check_with_channels(&scene, value, stage);
            assert_eq!(check["metadata"], true, "{name}: {stage}");
            assert_eq!(check["geometry"], true, "{name}: {stage}");
        }
    }
}
