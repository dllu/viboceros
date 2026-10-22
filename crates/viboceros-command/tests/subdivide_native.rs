#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod geometry_native;
use viboceros_command::{CommandRegistry, subdivide::SubdivisionSelection};
#[allow(dead_code)]
#[path = "support/subdivide.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;

#[test]
fn staged_subdivision_matches_native_geometry_attributes_selection_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let v = &row["value"];
        let mut scene = native::setup(v);
        native::check(&scene, &v["before"]);
        let mut targets = native::targets(&scene, v);
        for step in v["steps"].as_array().unwrap() {
            let prepared = SubdivisionSelection::prepare(&scene.doc, targets.clone())
                .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
            let out = prepared
                .commit(&mut scene.doc, v["input"]["preselected"] != true)
                .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
            native::check(&scene, &step["after"]);
            native::check_children(&scene, &out.selected_faces, &step["after"]);
            if v["input"]["reselect_children"] == true {
                targets = out
                    .selected_faces
                    .iter()
                    .map(|&(id, f)| viboceros_command::subdivide::SubdivisionTarget::Face(id, f))
                    .collect();
            }
        }
        scene.doc.clear_selection();
        let check = native::check(&scene, &v["after_script"]);
        assert_eq!(scene.doc.undo_label(), Some("SubDivide"));
        scene.doc.undo().unwrap();
        native::check(&scene, &v["undo"]);
        scene.doc.redo().unwrap();
        native::check(&scene, &v["redo"]);
        println!(
            "SUBDIVIDE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn command_registration_and_indexed_input_use_the_shared_operation() {
    let registry = CommandRegistry::with_builtins();
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "subdivide_subd_strip_local")
        .unwrap();
    let v = &row["value"];
    let mut scene = native::setup(v);
    scene
        .doc
        .select_object(scene.ids[0], viboceros_document::SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut scene.doc, "SubDivide Faces=0")
        .unwrap();
    scene.doc.clear_selection();
    native::check(&scene, &v["after_script"]);
    for name in ["SubDivide", "SelSubD", "SelOpenSubD", "SelClosedSubD"] {
        assert!(registry.command_names().contains(&name));
    }
}

#[test]
fn invalid_subdivision_scripts_and_prepared_stale_sources_preserve_model_and_redo() {
    use viboceros_command::subdivide::SubdivisionTarget;
    use viboceros_document::{Geometry, SelectionMode};
    let registry = CommandRegistry::with_builtins();
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "subdivide_subd_strip_local")
        .unwrap();
    let mut scene = native::setup(&row["value"]);
    scene
        .doc
        .select_objects_direct(scene.ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut scene.doc, "Point 99,99,99").unwrap();
    scene.doc.undo().unwrap();
    let before = format!("{:?}", scene.doc);
    for command in [
        "SubDivide 2",
        "SubDivide DeleteInput=No",
        "SubDivide Faces=99",
        "SubDivide Faces=0,99",
        "SubDivide Faces=0 trailing",
        "SubDivide Faces=All Faces=0",
    ] {
        assert!(
            registry.execute(&mut scene.doc, command).is_err(),
            "{command}"
        );
        assert_eq!(format!("{:?}", scene.doc), before, "{command}");
    }
    let invalid = SubdivisionSelection::prepare(
        &scene.doc,
        [
            SubdivisionTarget::Whole(scene.ids[0]),
            SubdivisionTarget::Face(scene.ids[0], usize::MAX),
        ],
    );
    assert!(invalid.is_err());
    assert_eq!(format!("{:?}", scene.doc), before);
    let prepared =
        SubdivisionSelection::prepare(&scene.doc, [SubdivisionTarget::Face(scene.ids[0], 0)])
            .unwrap();
    let Geometry::SubD(source) = scene.doc.object(scene.ids[0]).unwrap().geometry() else {
        panic!()
    };
    let changed = source.subdivided(1).unwrap();
    scene
        .doc
        .replace_object_geometries([(scene.ids[0], Geometry::SubD(changed))])
        .unwrap();
    let before = format!("{:?}", scene.doc);
    assert!(prepared.commit(&mut scene.doc, false).is_err());
    assert_eq!(format!("{:?}", scene.doc), before);
}
