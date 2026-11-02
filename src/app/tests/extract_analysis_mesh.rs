use super::*;
use serde_json::{Value, json};
use std::io::Read;
mod source;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn attrs(doc: &Document, o: &viboceros_document::Object) -> Value {
    let a = o.attributes();
    let c = a.object_color();
    json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
        "color_source":match a.color_source() {viboceros_document::ObjectColorSource::Layer=>"ColorFromLayer",viboceros_document::ObjectColorSource::Object=>"ColorFromObject",viboceros_document::ObjectColorSource::Parent=>"ColorFromParent",viboceros_document::ObjectColorSource::Material=>"ColorFromMaterial"},
        "color":[c.red,c.green,c.blue,255],"wire_density":a.wire_density(),
        "groups":o.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap_or("")).collect::<Vec<_>>(),
        "user_text":a.user_text().get("analysis-probe"),"geometry_text":o.geometry_user_text().get("geometry-probe"),"selected":doc.is_selected(o.id())})
}
fn state(app: &VibocerosApp, id: ObjectId) -> Value {
    let mut outputs = app
        .document
        .objects()
        .filter(|o| o.id() != id)
        .map(|o| {
            let Geometry::Mesh(m) = o.geometry() else {
                panic!("unexpected extraction output")
            };
            assert_eq!(m.vertex_normals().unwrap().len(), m.vertices().len());
            json!({"attributes":attrs(&app.document,o),"colored":m.vertex_colors().is_some()})
        })
        .collect::<Vec<_>>();
    outputs.sort_by_key(|o| o["attributes"].to_string());
    json!({"source":attrs(&app.document,app.document.object(id).unwrap()),"outputs":outputs})
}
fn expected(native: &Value) -> Value {
    let mut outputs=native["outputs"].as_array().unwrap().iter().map(|o|json!({"attributes":o["attributes"],"colored":o["mesh"]["colors"].as_u64().unwrap()>0})).collect::<Vec<_>>();
    outputs.sort_by_key(|o| o["attributes"].to_string());
    json!({"source":native["source"],"outputs":outputs})
}
#[test]
fn extract_analysis_mesh_matches_native_selection_attributes_colors_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/extract_analysis_mesh.json")).unwrap(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("observations/extract_analysis_mesh.json.gz")).unwrap(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    let n: Value = serde_json::from_slice(&bytes).unwrap();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let mut app = test_app();
        let id = source::install(&mut app, op["source"].as_str().unwrap());
        let mode = op["mode"].as_str().unwrap();
        if mode != "None" {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            if mode.starts_with("Draft") {
                enter(&mut app, "DraftAngleAnalysis Range=-5,7 Up=0,0,1");
            } else {
                enter(&mut app, "CurvatureAnalysis Style=Gaussian Range=-1,1");
            }
            if mode.ends_with("Off") {
                enter(
                    &mut app,
                    if mode.starts_with("Draft") {
                        "DraftAngleAnalysisOff"
                    } else {
                        "CurvatureAnalysisOff"
                    },
                );
            }
        }
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        let geometry = app.document.object(id).unwrap().geometry_snapshot().clone();
        assert_eq!(
            state(&app, id),
            expected(&row["value"]["states"][0]),
            "{} initial",
            op["id"]
        );
        for (i, step) in op["steps"].as_array().unwrap().iter().enumerate() {
            if step == "Extract" {
                app.document.clear_selection();
                if op["preselect"] == true {
                    app.document
                        .select_objects_direct([id], SelectionMode::Replace)
                        .unwrap();
                }
                enter(&mut app, "ExtractAnalysisMesh");
                if op["preselect"] == false {
                    assert!(
                        app.object_prompt.is_some(),
                        "{} {:?}",
                        op["id"],
                        app.command_log
                    );
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                    enter(&mut app, "");
                }
            } else {
                enter(&mut app, step.as_str().unwrap());
            }
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                op["id"],
                app.command_log
            );
            assert!(
                geometry.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot())
            );
            assert_eq!(
                state(&app, id),
                expected(&row["value"]["states"][i + 1]),
                "{} step {step}",
                op["id"]
            );
        }
        count += 1;
    }
    assert_eq!(count, 53);
}
