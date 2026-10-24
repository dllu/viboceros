#![cfg(feature = "native-smlib")]
#[allow(dead_code)]
#[path = "support/frozen_scene.rs"]
mod frozen_scene;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod mesh_reference;
#[path = "support/mesh_boolean.rs"]
#[allow(dead_code)]
mod native;
use serde_json::json;
use viboceros_command::CommandRegistry;

#[path = "support/mesh_boolean_replay.rs"]
mod replay;

#[test]
fn empty_mesh_split_commands_match_native_storage_and_history() {
    let (rows, gaps) = replay::replay(
        native::family_rows(&["empty"]),
        "MESH_BOOLEAN_EMPTY_COMMAND",
    );
    assert_eq!(rows.len(), 2);
    assert!(gaps.is_empty(), "{gaps:?}");
}

#[test]
fn mesh_boolean_commands_replay_native_metadata_geometry_and_history() {
    let (rows, gaps) = replay::replay(native::rows(), "MESH_BOOLEAN_COMMAND");
    assert_eq!(rows.len(), 79);
    assert_eq!(
        gaps,
        json!(native::COMPATIBILITY_GAPS)
            .as_array()
            .unwrap()
            .clone()
    );
}

#[test]
fn mesh_boolean_sets_commands_replay_native_metadata_geometry_and_history() {
    let (rows, gaps) = replay::replay(
        native::family_rows(&["sets", "split", "split_controls", "split_stages"]),
        "MESH_BOOLEAN_SETS_COMMAND",
    );
    assert_eq!(rows.len(), 64);
    assert_eq!(
        gaps,
        json!(native::SET_COMPATIBILITY_GAPS)
            .as_array()
            .unwrap()
            .clone()
    );
}

#[test]
fn mesh_boolean_failures_preserve_sources_selection_and_redo() {
    use viboceros_document::{Geometry, SelectionMode};
    use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
    let registry = CommandRegistry::with_builtins();
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "boolean_lifecycle_union_overlap_pre")
        .unwrap();
    let mut scene = native::setup(&row);
    let invalid = TriangleMesh::try_from_face_records(
        [0., 1., 2.]
            .into_iter()
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .collect(),
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    scene
        .doc
        .replace_object_geometries([(scene.ids[1], Geometry::Mesh(invalid))])
        .unwrap();
    scene.doc.begin_transaction("Redo witness").unwrap();
    scene
        .doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    scene.doc.commit_transaction().unwrap();
    scene.doc.undo().unwrap();
    scene
        .doc
        .select_objects_direct(scene.ids.clone(), SelectionMode::Replace)
        .unwrap();
    let before = native::snapshot(&scene);
    let undo = scene.doc.undo_label().map(str::to_owned);
    let redo = scene.doc.redo_label().map(str::to_owned);
    for command in [
        "MeshBooleanUnion",
        "MeshBooleanUnion Tolerance=Literal",
        "MeshBooleanUnion Tolerance=0.001",
        "MeshBooleanUnion Tolerance=Auto Tolerance=Zero",
    ] {
        assert!(registry.execute(&mut scene.doc, command).is_err());
        assert_eq!(native::snapshot(&scene), before);
        assert_eq!(scene.doc.undo_label(), undo.as_deref());
        assert_eq!(scene.doc.redo_label(), redo.as_deref());
        assert!(
            scene
                .doc
                .object(scene.ids[0])
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&scene.sources[0])
        );
    }
}

#[test]
fn mesh_boolean_tolerance_preferences_cross_commands_and_survive_cancel_answers() {
    let registry = CommandRegistry::with_builtins();
    registry
        .accept_object_selection_input("MeshBooleanUnion Tolerance=Zero")
        .unwrap();
    for name in [
        "MeshBooleanUnion",
        "MeshBooleanIntersection",
        "MeshBooleanDifference",
    ] {
        let prompt = registry.object_selection_prompt(name).unwrap().unwrap();
        assert_eq!(prompt.choices[0].value, "Zero");
    }
    registry
        .accept_object_selection_input("MeshBooleanDifference DeleteInput=No Tolerance=Document")
        .unwrap();
    let prompt = registry
        .object_selection_prompt("MeshBooleanUnion")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.choices[0].value, "Document");
    assert!(
        registry
            .accept_object_selection_input("MeshBooleanUnion Tolerance=Literal")
            .is_err()
    );
    assert_eq!(
        registry
            .object_selection_prompt("MeshBooleanUnion")
            .unwrap()
            .unwrap()
            .choices[0]
            .value,
        "Document"
    );
    assert!(
        !registry
            .object_selection_prompt("MeshBooleanDifference")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
    assert_eq!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("MeshBooleanUnion")
            .unwrap()
            .unwrap()
            .choices[0]
            .value,
        "Auto"
    );
}

#[test]
fn mesh_boolean_auto_and_zero_resolve_slivers_below_document_tolerance() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "boolean_options_difference_auto")
        .unwrap();
    for mode in ["Auto", "Zero"] {
        let registry = CommandRegistry::with_builtins();
        let mut scene = native::setup(&row);
        assert!((scene.doc.tolerance().absolute() - 1e-3).abs() < 1e-15);
        let mut input = row["value"]["input"].clone();
        input["arguments"] = json!(format!("Tolerance={mode}"));
        let command = native::command(&scene, &input);
        registry.execute(&mut scene.doc, &command).unwrap();
        let viboceros_document::Geometry::Mesh(mesh) =
            scene.doc.object(scene.ids[0]).unwrap().geometry()
        else {
            panic!("mesh Boolean result expected")
        };
        // A 0.0001-wide cut through a 3x2 cross section removes 0.0006.
        assert!((mesh.signed_volume().unwrap() - (24. - 0.0006)).abs() < 1e-9);
    }
}

#[test]
fn mesh_boolean_split_retains_cutter_stages_and_preserves_all_cutters() {
    use viboceros_document::Geometry;
    for (family, case, lengths, volumes) in [
        (
            "split",
            "boolean_split_two_cutters",
            vec![2, 3],
            vec![24., 24.],
        ),
        (
            "split_stages",
            "boolean_split_stages_three_cutters",
            vec![2, 3, 7],
            vec![24., 24., 48.],
        ),
    ] {
        let row = native::family_rows(&[family])
            .into_iter()
            .find(|r| r["id"] == case)
            .unwrap();
        let mut scene = native::setup(&row);
        let command = native::command(&scene, &row["value"]["input"]);
        CommandRegistry::with_builtins()
            .execute(&mut scene.doc, &command)
            .unwrap();
        for index in 1..scene.ids.len() {
            assert!(
                scene
                    .doc
                    .object(scene.ids[index])
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(&scene.sources[index])
            );
        }
        let pieces = scene
            .doc
            .objects()
            .filter(|o| o.id() != scene.witness && !scene.ids[1..].contains(&o.id()))
            .map(|o| {
                let Geometry::Mesh(mesh) = o.geometry() else {
                    panic!("split mesh expected")
                };
                mesh
            })
            .collect::<Vec<_>>();
        assert_eq!(pieces.len(), lengths.iter().sum::<usize>());
        assert!(pieces.iter().all(|m| m.topology().is_solid()));
        let mut offset = 0;
        for (length, volume) in lengths.into_iter().zip(volumes) {
            let stage = &pieces[offset..offset + length];
            assert!(
                (stage
                    .iter()
                    .map(|m| m.signed_volume().unwrap())
                    .sum::<f64>()
                    - volume)
                    .abs()
                    < 1e-9
            );
            offset += length;
        }
    }
}
