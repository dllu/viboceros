//! Actual custom-data getters, disconnected picks and document history.
use super::*;
use crate::app::merge_faces::native_models;
use serde_json::Value;
use viboceros_geometry::Vector3;
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-io/tests/support/subd_face_custom.rs"]
mod custom;
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-io/tests/support/subd_insert_custom.rs"]
mod insertion;
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-io/tests/support/subd_control_comparison.rs"]
mod support;

fn check(
    app: &VibocerosApp,
    id: ObjectId,
    baseline: &SubD,
    archive: &Value,
    rows: &Value,
    name: &str,
) {
    let Geometry::SubD(actual) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    let expected = support::read(archive);
    let mut differences = Vec::new();
    insertion::compare(actual, &expected, rows, baseline, name, &mut differences);
    assert!(differences.is_empty(), "{name}: {differences:?}");
    assert_eq!(
        app.document.objects().len(),
        rows.as_array().unwrap().len(),
        "{name}"
    );
}
#[test]
fn custom_uvw_insertions_match_native_getters_rejections_cancel_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    for family in [
        "subd_insert_custom",
        "subd_insert_order",
        "subd_insert_order_repeat",
    ] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let recipes: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "fixtures/{}_recipes.json",
                family.replace("_repeat", "")
            )))
            .unwrap(),
        )
        .unwrap();
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let recipe = &recipes[name];
            let baseline = support::read(&v["initial_3dm"]);
            let mut models = native_models(&v["initial_3dm"]);
            let mut app = test_app();
            let (id, geometry) = models.remove("Input 0").unwrap();
            app.document
                .add_geometry_with_metadata_and_id(
                    geometry,
                    viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id()),
                    Default::default(),
                    Some(id),
                )
                .unwrap();
            app.document.clear_history().unwrap();
            let (witness, geometry) = models.remove("Undo witness").unwrap();
            app.document
                .add_geometry_with_metadata_and_id(
                    geometry,
                    viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id()),
                    Default::default(),
                    Some(witness),
                )
                .unwrap();
            app.document.clear_selection();
            if let Some(faces) = recipe["merge"].as_array() {
                for face in faces {
                    app.accept_component_click(ComponentClick {
                        picks: vec![ComponentPick {
                            object: id,
                            kind: ComponentSelectionKind::SubDFace,
                            index: face.as_u64().unwrap() as usize,
                        }],
                        preselection: true,
                        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    });
                }
                enter(&mut app, "MergeFaces");
            }
            check(&app, id, &baseline, &v["source_3dm"], &v["before"], name);
            app.component_selection.clear();
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "InsertPoint");
            let source = support::read(&v["source_3dm"]);
            let document_source = app.document.object(id).unwrap().geometry().clone();
            for (i, action) in recipe["actions"].as_array().unwrap().iter().enumerate() {
                if action.get("click").is_some() {
                    let click = v["trace"]["clicks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["action"] == i)
                        .unwrap();
                    app.pick_insert_point(
                        insertion::pick(&source, &click["projected_point"]),
                        Vector3::try_new(0., 0., 1.)
                            .unwrap()
                            .normalized_nonzero()
                            .unwrap(),
                    );
                    // Preview and rejected picks must not edit the document.
                    assert_eq!(
                        app.document.object(id).unwrap().geometry(),
                        &document_source,
                        "{name}"
                    );
                } else {
                    enter(&mut app, action.as_str().unwrap());
                }
            }
            assert!(app.insert_prompt().is_none(), "{name}");
            check(&app, id, &baseline, &v["after_3dm"], &v["after"], name);
            for phase in ["undone", "redone"] {
                for state in v[phase].as_array().unwrap() {
                    enter(&mut app, if phase == "undone" { "Undo" } else { "Redo" });
                    check(
                        &app,
                        id,
                        &baseline,
                        &state["archive"],
                        &state["state"],
                        name,
                    );
                }
            }
        }
    }
}
