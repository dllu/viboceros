use super::tests::support;
use super::*;
use crate::app::tests::test_app;
use crate::viewport::ComponentClick;
use serde_json::{Value, json};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command_input();
}
fn check(app: &VibocerosApp, id: ObjectId, rows: &Value) {
    let d = &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap()["subd"];
    let mut expected = d["face_ids"]
        .as_array()
        .unwrap()
        .iter()
        .zip(d["face_colors"].as_array().unwrap())
        .map(|(id, c)| {
            (
                id.as_u64().unwrap(),
                if c.is_null() {
                    Value::Null
                } else {
                    json!([c[0], c[1], c[2], 255 - c[3].as_u64().unwrap()])
                },
            )
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|r| r.0);
    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
        panic!("owned SubD")
    };
    assert_eq!(
        s.face_ids()
            .iter()
            .zip(s.face_properties())
            .map(|(id, p)| (*id as u64, json!(p.color)))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(app.document.objects().len(), rows.as_array().unwrap().len());
}

pub(super) fn setup(encoded: &Value) -> (VibocerosApp, ObjectId, Geometry) {
    let mut models = support::models(encoded);
    let mut app = test_app();
    let (id, geometry) = models.remove("Input 0").unwrap();
    app.document
        .add_geometry_with_metadata_and_id(
            geometry.clone(),
            viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                .with_name("Input 0"),
            std::collections::BTreeMap::new(),
            Some(id),
        )
        .unwrap();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let (witness, point) = models.remove("Undo witness").unwrap();
    app.document
        .add_geometry_with_metadata_and_id(
            point,
            viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                .with_name("Undo witness"),
            std::collections::BTreeMap::new(),
            Some(witness),
        )
        .unwrap();
    app.document.clear_selection();
    (app, id, geometry)
}

#[test]
fn native_colored_subd_getters_sequential_history_and_noops_restore_face_properties() {
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(
            support::root().join("tools/rhino_oracle/observations/subd_merge_properties.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        let (mut app, id, geometry) = setup(&v["source_3dm"]);
        check(&app, id, &v["before"]);
        for step in v["steps"].as_array().unwrap() {
            app.component_selection.clear();
            let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
                panic!("owned SubD")
            };
            let targets = step["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    let value = c["id"].as_u64().unwrap() as u32;
                    let (kind, ids) = match c["kind"].as_str().unwrap() {
                        "face" => (
                            viboceros_command::ComponentSelectionKind::SubDFace,
                            s.face_ids(),
                        ),
                        "vertex" => (
                            viboceros_command::ComponentSelectionKind::SubDVertex,
                            s.vertex_ids(),
                        ),
                        "edge" => (
                            viboceros_command::ComponentSelectionKind::SubDEdge,
                            s.edge_ids(),
                        ),
                        _ => unreachable!(),
                    };
                    ComponentPick {
                        object: id,
                        kind,
                        index: ids.binary_search(&value).unwrap(),
                    }
                })
                .collect::<Vec<_>>();
            for p in targets {
                app.accept_component_click(ComponentClick {
                    picks: vec![p],
                    preselection: true,
                    modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                });
            }
            enter(&mut app, "MergeFaces");
            check(&app, id, &step["after"]);
        }
        for rows in v["undone"].as_array().unwrap() {
            enter(&mut app, "Undo");
            check(&app, id, rows);
        }
        assert_eq!(app.document.object(id).unwrap().geometry(), &geometry);
        for rows in v["redone"].as_array().unwrap() {
            enter(&mut app, "Redo");
            check(&app, id, rows);
        }
    }
}
