use super::*;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use serde_json::Value;
use std::io::Read;
use viboceros_command::ComponentSelectionKind as Kind;
mod command;
mod viewport;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn capture() -> Value {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/observations/edge_loop_modified_gestures.json.gz"
    ))
    .unwrap();
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    serde_json::from_slice(&raw).unwrap()
}
fn fixture(value: &Value) -> (VibocerosApp, ObjectId) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.3dm");
    let bytes = value["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let mut app = test_app();
    app.document = viboceros_command::open_3dm_with_views(path.to_str().unwrap())
        .unwrap()
        .0;
    let id = app.document.objects().next().unwrap().id();
    app.document.set_subd_control_net(true);
    app.document.clear_history().unwrap();
    (app, id)
}
fn pick(id: ObjectId, mesh: bool, index: usize) -> ComponentPick {
    ComponentPick {
        object: id,
        index,
        kind: if mesh { Kind::MeshEdge } else { Kind::SubDEdge },
    }
}
fn indices(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_u64().unwrap() as usize)
        .collect()
}
fn selected(app: &VibocerosApp) -> Vec<usize> {
    let mut ids = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap()
        .iter()
        .map(|p| p.index)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}
fn seed(app: &mut VibocerosApp, picks: Vec<ComponentPick>) {
    app.accept_component_window(ComponentWindow {
        picks,
        preselection: true,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
        crossing: true,
        inverted: false,
    });
}
fn double(app: &mut VibocerosApp, p: ComponentPick) {
    let click = ComponentClick {
        picks: vec![p],
        preselection: true,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    };
    app.accept_component_click(click.clone());
    app.accept_component_loop_click(click);
}

#[test]
fn native_loop_gestures_match_sets_and_conversion_order_without_editing_sources() {
    let data = capture();
    assert_eq!(data["results"].as_array().unwrap().len(), 16);
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        let mesh = row["id"].as_str().unwrap().starts_with("mesh_");
        for pre in [false, true] {
            let (mut app, id) = fixture(v);
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            assert!(app.document.can_redo());
            let source = app.document.object(id).unwrap().clone();
            if !pre {
                enter(&mut app, "ToNURBS");
            }
            seed(
                &mut app,
                indices(&v["before"]["ordinals"])
                    .into_iter()
                    .map(|i| pick(id, mesh, i))
                    .collect(),
            );
            double(
                &mut app,
                pick(id, mesh, v["seed"].as_u64().unwrap() as usize),
            );
            assert_eq!(
                selected(&app),
                indices(&v["after"]["ordinals"]),
                "{} pre={pre}",
                row["id"]
            );
            assert_eq!(app.document.object(id), Some(&source));
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
            let order = indices(&v["to_nurbs_order"]);
            if order.is_empty() {
                if !pre {
                    enter(&mut app, "");
                    assert_eq!(
                        app.object_prompt.as_ref().unwrap().phase,
                        crate::app::object_selection::ObjectPromptPhase::Selecting
                    );
                    app.cancel_interactive_command(true);
                    assert!(app.object_prompt.is_none());
                }
                continue;
            }
            if pre {
                enter(&mut app, "ToNURBS");
            } else {
                enter(&mut app, "");
            }
            enter(&mut app, "");
            assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
            assert_eq!(app.document.object(id), Some(&source));
            let outputs = app.document.objects().skip(1).collect::<Vec<_>>();
            assert_eq!(outputs.len(), order.len());
            for (o, i) in outputs.into_iter().zip(order) {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!()
                };
                let edge = &v["edges"][i];
                let points = indices(edge)
                    .into_iter()
                    .map(|i| {
                        Point3::try_from(std::array::from_fn(|a| {
                            v["input"]["points"][i][a].as_f64().unwrap()
                        }))
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(c.degree(), 1);
                assert_eq!(
                    c.control_points()
                        .iter()
                        .map(|p| p.point())
                        .collect::<Vec<_>>(),
                    points,
                    "{} pre={pre}",
                    row["id"]
                );
            }
            let after = app.document.objects().cloned().collect::<Vec<_>>();
            enter(&mut app, "Undo");
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                [source]
            );
            assert!(selected(&app).is_empty());
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn double_clicks_keep_ambiguity_explicit_and_reject_changed_sources() {
    let data = capture();
    let v = &data["results"][0]["value"];
    let (mut app, id) = fixture(v);
    let geometry = app.document.object(id).unwrap().geometry().clone();
    let other = app.document.add_geometry(geometry).unwrap();
    let index = v["seed"].as_u64().unwrap() as usize;
    app.accept_component_loop_click(ComponentClick {
        picks: vec![pick(id, true, index), pick(other, true, index)],
        preselection: true,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    });
    assert!(app.component_selection.has_choices());
    assert!(selected(&app).is_empty());
    enter(&mut app, "2");
    assert!(!app.component_selection.has_choices());
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .iter()
            .all(|p| p.object == other)
    );
    assert!(selected(&app).len() > 1);
    app.component_selection.clear();
    let click = ComponentClick {
        picks: vec![pick(id, true, index)],
        preselection: true,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    };
    app.accept_component_click(click.clone());
    app.document
        .replace_object_geometries(vec![(id, Geometry::Point(point(9., 9., 9.)))])
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.accept_component_loop_click(click);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.command_log.back().unwrap().contains("changed"));
}
