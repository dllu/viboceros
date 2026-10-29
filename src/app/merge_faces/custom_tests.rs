//! Native material/UVW states through the application command and full history.
use super::face_properties_tests::setup;
use super::tests::support;
use super::*;
use crate::viewport::ComponentClick;
use serde_json::Value;
use viboceros_command::ComponentSelectionKind;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command_input();
}
fn check(app: &VibocerosApp, id: ObjectId, archive: &Value, rows: &Value, name: &str) {
    let models = support::models(archive);
    let Geometry::SubD(expected) = &models["Input 0"].1 else {
        panic!("native SubD");
    };
    let actual = app.document.object(id).unwrap().geometry();
    let Geometry::SubD(s) = actual else {
        panic!("application SubD");
    };
    assert_eq!(
        s.face_properties(),
        expected.face_properties(),
        "{name} materials/colors"
    );
    assert_eq!(
        s.face_texture_points(),
        expected.face_texture_points(),
        "{name} UVWs"
    );
    assert_eq!(
        s.texture_coordinate_type(),
        expected.texture_coordinate_type(),
        "{name} texture source"
    );
    support::compare(
        &support::geometry_value(actual),
        &support::geometry_value(&models["Input 0"].1),
        name,
    );
    assert_eq!(
        app.document.objects().len(),
        rows.as_array().unwrap().len(),
        "{name} owner count"
    );
}
#[test]
fn native_materials_and_custom_uvws_survive_merge_getters_undo_and_redo() {
    for family in ["subd_face_custom", "subd_face_custom_holdouts"] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(
                support::root().join(format!("tools/rhino_oracle/observations/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let (mut app, id, _) = setup(&v["source_3dm"]);
            check(&app, id, &v["source_3dm"], &v["before"], name);
            for step in v["steps"].as_array().unwrap() {
                app.component_selection.clear();
                for c in step["selected"].as_array().unwrap() {
                    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
                        panic!("source SubD");
                    };
                    let (kind, ids) = match c["kind"].as_str().unwrap() {
                        "face" => (ComponentSelectionKind::SubDFace, s.face_ids()),
                        "edge" => (ComponentSelectionKind::SubDEdge, s.edge_ids()),
                        "vertex" => (ComponentSelectionKind::SubDVertex, s.vertex_ids()),
                        _ => unreachable!(),
                    };
                    let index = ids
                        .binary_search(&(c["id"].as_u64().unwrap() as u32))
                        .unwrap();
                    app.accept_component_click(ComponentClick {
                        picks: vec![ComponentPick {
                            object: id,
                            kind,
                            index,
                        }],
                        preselection: true,
                        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    });
                }
                enter(&mut app, "MergeFaces");
                check(&app, id, &step["after_3dm"], &step["after"], name);
            }
            for phase in ["undone", "redone"] {
                for state in v[phase].as_array().unwrap() {
                    enter(&mut app, if phase == "undone" { "Undo" } else { "Redo" });
                    check(&app, id, &state["archive"], &state["state"], name);
                }
            }
        }
    }
}
