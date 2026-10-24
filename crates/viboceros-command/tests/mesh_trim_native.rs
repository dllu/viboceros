#![cfg(feature = "native-smlib")]
#[allow(dead_code)]
#[path = "support/mesh_boolean.rs"]
mod boolean;
#[allow(dead_code)]
#[path = "support/frozen_scene.rs"]
mod frozen_scene;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod mesh_reference;
#[path = "support/mesh_trim.rs"]
#[allow(dead_code)]
mod native;
#[allow(dead_code)]
#[path = "support/mesh_split.rs"]
mod split;
use serde_json::json;
use viboceros_command::{CommandContext, CommandRegistry};

#[test]
fn mesh_trim_commands_replay_native_picked_regions_channels_and_history() {
    let (count, gaps) = replay(native::rows(), "MESH_TRIM_COMMAND", false);
    assert_eq!(count, 13);
    assert_eq!(gaps, native::GAPS);
}
#[test]
fn mesh_trim_captured_cutters_replay_native_continuations() {
    let (count, gaps) = replay(
        native::rows_for(&["continuation"]),
        "MESH_TRIM_CONTINUATION_COMMAND",
        true,
    );
    assert_eq!(count, 5);
    assert!(gaps.is_empty(), "{gaps:?}");
}
#[test]
fn mesh_trim_original_geometry_replays_native_cutter_policy() {
    let (count, gaps) = replay(
        native::rows_for(&["cutter_policy"]),
        "MESH_TRIM_CUTTER_POLICY_COMMAND",
        true,
    );
    assert_eq!(count, 2);
    assert!(gaps.is_empty(), "{gaps:?}");
}
#[test]
fn mesh_trim_captured_cutters_expose_perspective_curve_surface_difference() {
    let (count, gaps) = replay(
        native::rows_for(&["snapshot_policy"]),
        "MESH_TRIM_SNAPSHOT_POLICY_COMMAND",
        true,
    );
    assert_eq!(count, 2);
    assert_eq!(gaps, native::SNAPSHOT_GAPS);
}
fn replay(rows: Vec<serde_json::Value>, prefix: &str, frozen: bool) -> (usize, Vec<String>) {
    let registry = CommandRegistry::with_builtins();
    let mut gaps = vec![];
    let mut count = 0;
    for row in rows {
        // The backend performs accepted picks; cancellation is a getter action.
        if row["value"]["input"]["finish"] == "Cancel" {
            continue;
        }
        let value = &row["value"];
        let mut scene = native::setup(&row);
        let cutters = native::cutters(&row, &scene.ids);
        let mut captured = frozen
            .then(|| viboceros_command::MeshTrimCutters::capture(&scene.doc, &cutters).unwrap());
        let mut history = scene.doc.begin_history_group("MeshTrim").unwrap();
        for (index, point) in native::picks(&row).into_iter().enumerate() {
            let target = native::target_for_pick(&scene.doc, &row, index, point);
            let command = native::command(&row, &cutters, target, point);
            if let Some(captured) = &mut captured {
                captured
                    .trim(
                        &mut scene.doc,
                        target,
                        point,
                        &[],
                        CommandContext::default(),
                        &mut history,
                    )
                    .unwrap();
            } else {
                registry
                    .execute_in_history_group(
                        &mut scene.doc,
                        &command,
                        CommandContext::default(),
                        &mut history,
                    )
                    .unwrap();
            }
        }
        let after = split::check_with_channels(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = split::check_with_channels(&scene, value, "undo");
        let ownership = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = split::check_with_channels(&scene, value, "redo");
        let measured = json!({"id":row["id"],"success":true,"error":null,"after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":ownership});
        println!("{prefix} {measured}");
        assert!(ownership, "{}", row["id"]);
        if ["after", "undo", "redo"].iter().any(|s| {
            ["geometry", "metadata", "ngons"]
                .iter()
                .any(|k| measured[s][k] != true)
        }) {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        count += 1;
    }
    eprintln!("MeshTrim commands {count}; gaps {gaps:?}");
    gaps.sort();
    (count, gaps)
}

#[test]
fn mesh_trim_invalid_late_cutter_preserves_sources_selection_and_redo() {
    use viboceros_document::{Geometry, SelectionMode};
    use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh_trim_controls_sheet_left_post")
        .unwrap();
    let mut scene = native::setup(&row);
    let invalid = TriangleMesh::try_from_face_records(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let invalid = scene.doc.add_geometry(Geometry::Mesh(invalid)).unwrap();
    scene
        .doc
        .add_geometry(Geometry::Point(Point3::try_new(8., 9., 10.).unwrap()))
        .unwrap();
    scene.doc.undo().unwrap();
    scene
        .doc
        .select_objects_direct([scene.ids[0], scene.ids[1]], SelectionMode::Replace)
        .unwrap();
    let before = boolean::snapshot(&scene);
    let undo = scene.doc.undo_label().map(str::to_owned);
    let redo = scene.doc.redo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    let point = native::picks(&row)[0];
    let command = native::command(&row, &[scene.ids[1], invalid], scene.ids[0], point);
    assert!(registry.execute(&mut scene.doc, &command).is_err());
    assert_eq!(boolean::snapshot(&scene), before);
    assert_eq!(scene.doc.undo_label(), undo.as_deref());
    assert_eq!(scene.doc.redo_label(), redo.as_deref());
    assert!(boolean::sources_restored(&scene));
}
