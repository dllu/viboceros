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
            for word in ["IP", "3Point", "2,0,0", "2,1,0", "2,0,1"] {
                enter(app, word);
            }
        }
        enter(app, "");
    }
    assert!(app.picking_mesh_trim_target(), "{:?}", app.command_log);
}
#[test]
fn mesh_trim_getters_replay_native_physical_picks_cancel_channels_and_history() {
    let mut gaps = vec![];
    let mut count = 0;
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(&row);
        let mut app = test_app();
        app.document = std::mem::take(&mut scene.doc);
        start(&mut app, &row, &scene.ids, false);
        for point in native::picks(&row) {
            let target = native::target(&app.document, point);
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
        println!("MESH_TRIM_GETTER {measured}");
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
    assert_eq!(count, 14);
    assert_eq!(gaps, native::GAPS);
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
