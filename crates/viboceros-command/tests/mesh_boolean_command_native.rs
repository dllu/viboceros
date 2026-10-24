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
use viboceros_command::{CommandContext, CommandRegistry};

#[test]
fn mesh_boolean_commands_replay_native_metadata_geometry_and_history() {
    let mut registry = CommandRegistry::with_builtins();
    let mut family = serde_json::Value::Null;
    let mut count = 0;
    let mut gaps = vec![];
    for row in native::rows() {
        if row["family"] != family {
            registry = CommandRegistry::with_builtins();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        if input["cancel"] == true
            || input["cancel_second"] == true
            || row["id"] == "boolean_lifecycle_union_one"
        {
            // Native options are accepted even if the getter is then cancelled.
            let scene = native::setup(&row);
            registry
                .accept_object_selection_input(&native::command(&scene, input))
                .unwrap();
            continue;
        }
        let mut scene = native::setup(&row);
        let before = native::check(&scene, value, "before");
        assert_eq!(before["metadata"], true, "{}", row["id"]);
        assert_eq!(before["geometry"], true, "{}", row["id"]);
        let (first, second) = native::sets(&scene, input);
        if input["preselect"] == false {
            native::pick(&mut scene, first.into_iter().chain(second));
        }
        let command = native::command(&scene, input);
        eprintln!("Mesh Boolean command {}: {command}", row["id"]);
        let result = if input["preselect"] == false {
            registry.execute_postselected(&mut scene.doc, &command, CommandContext::default())
        } else {
            registry.execute(&mut scene.doc, &command)
        };
        let success = result.is_ok();
        let prompt = registry
            .object_selection_prompt(command.split_whitespace().next().unwrap())
            .unwrap()
            .unwrap();
        if let Some(expected) = native::expected_tolerance(value) {
            assert_eq!(prompt.choices[0].value, expected, "{}", row["id"]);
        }
        let after = native::check(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = native::check(&scene, value, "undo");
        let restored = native::sources_restored(&scene);
        assert!(restored, "{}", row["id"]);
        scene.doc.redo().unwrap();
        let redo = native::check(&scene, value, "redo");
        let matched = success == (value["success"] == true)
            && [&after, &undo, &redo]
                .iter()
                .all(|v| v["metadata"] == true && v["geometry"] == true);
        if !matched {
            gaps.push(row["id"].clone());
        }
        println!(
            "MESH_BOOLEAN_COMMAND {}",
            json!({"id":row["id"],"success":success,"error":result.err().map(|e|e.to_string()),
            "after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":restored})
        );
        count += 1;
    }
    eprintln!("Mesh Boolean command replay: {count} cases; gaps {gaps:?}");
    assert_eq!(count, 79);
    gaps.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    assert_eq!(
        gaps,
        json!(native::COMPATIBILITY_GAPS)
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
