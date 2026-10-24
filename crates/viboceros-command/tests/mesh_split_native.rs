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
#[path = "support/mesh_split.rs"]
mod native;
use serde_json::json;
use viboceros_command::CommandRegistry;
use viboceros_document::SelectionMode;
#[test]
fn mesh_split_commands_replay_native_geometry_metadata_and_history() {
    let mut registry = CommandRegistry::with_builtins();
    let mut family = serde_json::Value::Null;
    let mut gaps = Vec::new();
    let mut count = 0;
    for row in native::rows() {
        if family != row["family"] {
            registry = CommandRegistry::with_builtins();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        let name = row["id"].as_str().unwrap();
        if native::cancelled(&row) {
            if let Some(arguments) = input["arguments"]
                .as_str()
                .filter(|_| !input["menu"].is_string())
            {
                registry
                    .accept_object_selection_input(&format!(
                        "{} {}",
                        native::name(&row),
                        arguments.replace('_', "")
                    ))
                    .unwrap();
            }
            continue;
        }
        let mut scene = native::setup(&row);
        if input["preselect"] == false {
            let selected = input["first"]
                .as_array()
                .unwrap()
                .iter()
                .chain(input["second"].as_array().unwrap())
                .map(|i| scene.ids[i.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            scene
                .doc
                .select_objects_direct(selected, SelectionMode::Replace)
                .unwrap();
        }
        let command = native::command(&row, &scene.ids);
        let context = native::context(&row);
        let result = if input["preselect"] == true {
            registry.execute_in_context(&mut scene.doc, &command, context)
        } else {
            registry.execute_postselected(&mut scene.doc, &command, context)
        };
        let after = native::check(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = native::check(&scene, value, "undo");
        let ownership = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = native::check(&scene, value, "redo");
        let row = json!({"id":name,"success":result.is_ok(),"error":result.err().map(|e|e.to_string()),"after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":ownership});
        println!("MESH_SPLIT_COMMAND {row}");
        if row["success"] != value["success"]
            || ["after", "undo", "redo"].iter().any(|s| {
                row[s]["metadata"] != true || row[s]["geometry"] != true || row[s]["ngons"] != true
            })
        {
            gaps.push(name.to_owned());
        }
        count += 1;
    }
    eprintln!("MeshSplit commands {count}; gaps {gaps:?}");
    gaps.sort();
    assert_eq!(count, 58);
    assert_eq!(gaps, native::GAPS);
}

#[test]
fn late_invalid_split_target_preserves_geometry_selection_and_redo() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{MeshFace, Point3, TriangleMesh};
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "mesh_split_box_plane_pre")
        .unwrap();
    let mut scene = native::setup(&row);
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let bad = scene.doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    scene
        .doc
        .add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.).unwrap()))
        .unwrap();
    scene.doc.undo().unwrap();
    scene
        .doc
        .select_objects_direct([scene.ids[0], bad], SelectionMode::Replace)
        .unwrap();
    let before = boolean::snapshot(&scene);
    let undo = scene.doc.undo_label().map(str::to_owned);
    let redo = scene.doc.redo_label().map(str::to_owned);
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "MeshSplit FirstSet={},{} SecondSet={}",
        scene.ids[0], bad, scene.ids[1]
    );
    assert!(registry.execute(&mut scene.doc, &command).is_err());
    assert_eq!(boolean::snapshot(&scene), before);
    assert_eq!(scene.doc.undo_label(), undo.as_deref());
    assert_eq!(scene.doc.redo_label(), redo.as_deref());
    assert!(boolean::sources_restored(&scene));
}
