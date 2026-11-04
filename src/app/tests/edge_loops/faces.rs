use super::*;
use std::collections::BTreeSet;
pub(super) fn data(family: &str) -> (Value, Value) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let bytes = std::fs::read(base.join(format!("observations/{family}.json.gz"))).unwrap();
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    (request, serde_json::from_slice(&raw).unwrap())
}
fn ordinal(v: &Value, key: &str, id: usize) -> usize {
    v[key]
        .as_array()
        .unwrap()
        .iter()
        .position(|n| n.as_u64() == Some(id as u64))
        .unwrap()
}
fn snapshot(app: &VibocerosApp, id: ObjectId, v: &Value, state: &Value) {
    let actual = app
        .component_selection
        .checked_picks(&app.document)
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected = state["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let n = c["index"].as_u64().unwrap() as usize;
            let (kind, index) = match c["kind"].as_str().unwrap() {
                "MeshTopologyEdge" => (Kind::MeshEdge, ordinal(v, "native", n)),
                "SubdEdge" => (Kind::SubDEdge, ordinal(v, "native", n)),
                "MeshFace" => (Kind::MeshFace, ordinal(v, "face_native", n)),
                "SubdFace" => (Kind::SubDFace, ordinal(v, "face_native", n)),
                "MeshVertex" => (Kind::MeshVertex, n),
                "SubdVertex" => {
                    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
                        panic!()
                    };
                    (
                        Kind::SubDVertex,
                        s.component_ids()
                            .unwrap()
                            .vertices
                            .iter()
                            .position(|&i| i as usize == n)
                            .unwrap(),
                    )
                }
                other => panic!("unexpected native component {other}"),
            };
            ComponentPick {
                object: id,
                kind,
                index,
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        app.document.is_selected(id),
        state["whole"].as_bool().unwrap()
    );
    assert_eq!(
        app.document.objects().len(),
        state["objects"].as_u64().unwrap() as usize
    );
}
#[test]
fn face_loops_and_mixed_preselection_replay_all_native_states_without_model_edits() {
    let mut count = 0;
    for family in [
        "face_loop_discovery",
        "face_loop_commands",
        "component_pattern_preselection",
        "component_pattern_holdouts",
    ] {
        let (request, observed) = data(family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            count += 1;
            let v = &row["value"];
            let mesh = op["kind"] == "mesh";
            let (mut app, id) = fixture(v);
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            let source = app.document.object(id).unwrap().clone();
            if op["whole"] == true {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
            if let Some(component) = op["component"].as_str() {
                let kind = match (component, mesh) {
                    ("face", true) => Kind::MeshFace,
                    ("face", false) => Kind::SubDFace,
                    (_, true) => Kind::MeshVertex,
                    (_, false) => Kind::SubDVertex,
                };
                seed(
                    &mut app,
                    vec![ComponentPick {
                        object: id,
                        index: 0,
                        kind,
                    }],
                );
            }
            seed(
                &mut app,
                indices(&v["before"]["ordinals"])
                    .into_iter()
                    .map(|i| pick(id, mesh, i))
                    .collect(),
            );
            snapshot(&app, id, v, &v["before"]);
            enter(&mut app, op["command"].as_str().unwrap_or("SelFaceLoop"));
            for (step, state) in op["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(v["trace"].as_array().unwrap())
            {
                let pair = indices(&step["pair"]);
                let edge = v["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .position(|e| {
                        let e = indices(e);
                        e == pair || e == [pair[1], pair[0]]
                    })
                    .unwrap();
                app.accept_component_click(ComponentClick {
                    picks: vec![pick(id, mesh, edge)],
                    preselection: false,
                    modifiers: if step["modifiers"] == "ctrl" {
                        egui::Modifiers::CTRL
                    } else {
                        egui::Modifiers::NONE
                    },
                });
                snapshot(&app, id, v, state);
            }
            if app.edge_pattern_prompt.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            assert!(app.edge_pattern_prompt.is_none(), "{}", row["id"]);
            snapshot(&app, id, v, &v["after"]);
            assert_eq!(app.document.object(id).unwrap(), &source);
            assert!(app.document.can_redo());
            enter(&mut app, "Redo");
            snapshot(&app, id, v, &v["redo"]);
            enter(&mut app, "Undo");
            snapshot(&app, id, v, &v["undo"]);
        }
    }
    assert_eq!(count, 131);
}
