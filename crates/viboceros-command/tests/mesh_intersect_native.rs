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
#[path = "support/mesh_intersect.rs"]
mod native;
use serde_json::json;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::SelectionMode;

#[test]
fn mesh_intersect_commands_match_saved_native_scenes_and_history() {
    let mut gaps = Vec::new();
    let mut total = 0;
    let mut registry = CommandRegistry::with_builtins();
    let mut family = serde_json::Value::Null;
    for row in native::rows() {
        if family != row["family"] {
            registry = CommandRegistry::with_builtins();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        let name = row["id"].as_str().unwrap();
        if native::menu(&row) {
            continue;
        }
        let mut scene = native::setup(&row);
        let command_name = native::name(&row);
        let command = native::command(&row);
        if input["cancel"] == true {
            registry.accept_object_selection_input(&command).unwrap();
            continue;
        }
        if input["preselect"] == false {
            scene
                .doc
                .select_objects_direct(scene.ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        let result = if input["preselect"] == true {
            registry.execute(&mut scene.doc, &command)
        } else {
            registry.execute_postselected(&mut scene.doc, &command, CommandContext::default())
        };
        let after = boolean::check(&scene, value, "after");
        if command_name == "MeshIntersect" {
            assert!(boolean::sources_restored(&scene), "{name} changed a source");
        }
        scene.doc.undo().unwrap();
        let undo = boolean::check(&scene, value, "undo");
        let sources_restored_on_undo = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = boolean::check(&scene, value, "redo");
        let success = result.is_ok();
        let row = json!({"id":name,"success":success,"error":result.err().map(|e|e.to_string()),"after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":sources_restored_on_undo});
        println!("MESH_INTERSECT_COMMAND {row}");
        if success != value["success"].as_bool().unwrap()
            || ["after", "undo", "redo"]
                .iter()
                .any(|s| row[s]["metadata"] != true || row[s]["geometry"] != true)
        {
            gaps.push(name.to_owned());
        }
        total += 1;
    }
    gaps.sort();
    assert_eq!(total, 57);
    assert_eq!(gaps, native::GAPS);
    println!("MeshIntersect {total} commands; gaps {gaps:?}");
}

#[test]
fn late_invalid_mesh_preserves_geometry_selection_and_redo() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh_intersect_overlap_pre")
        .unwrap();
    let mut scene = native::setup(&row);
    let bad = TriangleMesh::try_from_face_records(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let bad = scene.doc.add_geometry(Geometry::Mesh(bad)).unwrap();
    scene
        .doc
        .add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.).unwrap()))
        .unwrap();
    scene.doc.undo().unwrap();
    scene
        .doc
        .select_objects_direct(
            scene.ids.iter().copied().chain([bad]),
            SelectionMode::Replace,
        )
        .unwrap();
    let before = boolean::snapshot(&scene);
    let undo = scene.doc.undo_label().map(str::to_owned);
    let redo = scene.doc.redo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    assert!(registry.execute(&mut scene.doc, "MeshIntersect").is_err());
    assert_eq!(boolean::snapshot(&scene), before);
    assert_eq!(scene.doc.undo_label(), undo.as_deref());
    assert_eq!(scene.doc.redo_label(), redo.as_deref());
    assert!(boolean::sources_restored(&scene));
}

#[test]
fn tolerance_and_coplanar_preferences_are_shared_without_resetting_cutter_retention() {
    let r = CommandRegistry::with_builtins();
    r.accept_object_selection_input("MeshBooleanDifference DeleteInput=No Tolerance=Zero")
        .unwrap();
    let p = r.object_selection_prompt("MeshIntersect").unwrap().unwrap();
    assert_eq!(p.choices[0].value, "Zero");
    r.accept_object_selection_input("MeshIntersect CoplanarFaces=Mesh Tolerance=Document")
        .unwrap();
    let p = r
        .object_selection_prompt("MeshBooleanDifference")
        .unwrap()
        .unwrap();
    assert_eq!(p.choices[0].value, "Document");
    assert!(!p.options[0].value);
    for input in [
        "MeshIntersect Tolerance=Literal",
        "MeshIntersect CoplanarFaces=Meshes",
        "MeshIntersect Tolerance=Auto Tolerance=Zero",
    ] {
        assert!(r.accept_object_selection_input(input).is_err());
        let p = r.object_selection_prompt("MeshIntersect").unwrap().unwrap();
        assert_eq!(p.choices[0].value, "Document");
        assert_eq!(p.choices[1].value, "Mesh");
    }
}
