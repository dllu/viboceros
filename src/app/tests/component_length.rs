use super::*;
use serde_json::{Value, json};
use viboceros_command::{ComponentPick, ComponentSelectionKind as Kind, chain_selection::Target};
#[path = "../../../crates/viboceros-oracle/tests/support/component_length.rs"]
mod native;
use super::oracle_observation as observation;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
struct Source {
    app: VibocerosApp,
    owner: ObjectId,
    curve: Option<ObjectId>,
    edges: Vec<[usize; 2]>,
    native: Vec<usize>,
    kind: Kind,
}
fn fixture(kind: &str, scene: &str, flat: bool, extra: bool) -> Source {
    let mut app = test_app();
    app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let (geometry, points) =
        viboceros_oracle::component_length::source(kind, scene, app.document.tolerance()).unwrap();
    let edges = viboceros_oracle::component_length::edges(&geometry, &points);
    let native = if let Geometry::SubD(s) = &geometry {
        s.component_ids()
            .unwrap()
            .edges
            .into_iter()
            .map(|i| i as usize)
            .collect()
    } else {
        (0..edges.len()).collect()
    };
    let kind = match geometry {
        Geometry::Mesh(_) => Kind::MeshEdge,
        Geometry::SubD(_) => Kind::SubDEdge,
        _ => Kind::BrepEdge,
    };
    let owner = app.document.add_geometry(geometry).unwrap();
    let curve = if extra {
        enter(&mut app, "Line 30,0,0 33,0,0");
        Some(app.document.objects().last().unwrap().id())
    } else {
        None
    };
    if flat {
        enter(&mut app, "SubDDisplayToggle");
    }
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    Source {
        app,
        owner,
        curve,
        edges,
        native,
        kind,
    }
}
fn snapshot(
    app: &VibocerosApp,
    owner: ObjectId,
    curve: Option<ObjectId>,
    native: &[usize],
    kind: Kind,
) -> Value {
    let kind = match kind {
        Kind::MeshEdge => "MeshTopologyEdge",
        Kind::SubDEdge => "SubdEdge",
        _ => "BrepEdge",
    };
    let mut components = app
        .commands
        .component_selection()
        .checked_picks(&app.document)
        .unwrap()
        .into_iter()
        .map(|p| json!({"index":native[p.index],"kind":kind}))
        .collect::<Vec<_>>();
    components.sort_by_key(|p| p["index"].as_u64().unwrap());
    json!({"whole":app.document.is_selected(owner),"curve":curve.is_some_and(|id|app.document.is_selected(id)),"components":components,"objects":app.document.objects().len()})
}
fn length(app: &VibocerosApp) -> Option<f64> {
    app.command_log.iter().rev().find_map(|s| {
        s.split_once("total length ")
            .and_then(|(_, s)| s.split_whitespace().next())
            .and_then(|s| s.parse().ok())
    })
}
#[test]
fn native_component_lengths_preselection_cleanup_and_picked_inputs_replay_through_the_app() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "component_length_discovery",
        "component_length_followups",
        "component_length_workflows",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let expected = observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["results"].as_array().unwrap())
        {
            count += 1;
            let v = &row["value"];
            let Source {
                mut app,
                owner,
                curve,
                edges,
                native: native_ids,
                kind,
            } = fixture(
                op["kind"].as_str().unwrap(),
                op["scene"].as_str().unwrap(),
                op["flat"].as_bool().unwrap(),
                op["curve"].as_bool().unwrap(),
            );
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            let originals = app.document.objects().cloned().collect::<Vec<_>>();
            let picks = op["pairs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = [
                        p[0].as_u64().unwrap() as usize,
                        p[1].as_u64().unwrap() as usize,
                    ];
                    ComponentPick {
                        object: owner,
                        index: edges
                            .iter()
                            .position(|e| *e == p || *e == [p[1], p[0]])
                            .unwrap(),
                        kind,
                    }
                })
                .collect::<Vec<_>>();
            let post = op.get("steps").is_some();
            if !post {
                app.commands
                    .component_selection()
                    .replace(&app.document, picks.clone())
                    .unwrap();
                if let Some(id) = curve {
                    app.document
                        .select_objects_direct([id], SelectionMode::Add)
                        .unwrap();
                }
            }
            let check = |app: &VibocerosApp, state: &Value| {
                assert_eq!(
                    snapshot(app, owner, curve, &native_ids, kind),
                    *state,
                    "{}",
                    row["id"]
                )
            };
            check(&app, &v["before"]);
            enter(&mut app, "Length");
            if post {
                for (step, state) in op["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(v["trace"].as_array().unwrap())
                {
                    let target = if step["target"] == "curve" {
                        Target::Curve(curve.unwrap())
                    } else {
                        Target::Edge(owner, picks[0].index)
                    };
                    app.handle_viewport_action(crate::viewport::ViewportOutput {
                        chain_click: Some(vec![target]),
                        length_mode: Some(if step["modifiers"] == "ctrl" {
                            SelectionMode::Remove
                        } else {
                            SelectionMode::Add
                        }),
                        ..Default::default()
                    });
                    check(&app, state);
                }
                if op["finish"] == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            check(&app, &v["after"]);
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                originals
            );
            assert!(app.document.can_redo());
            assert!(!app.document.can_undo());
            if let Some((value, epsilon)) = native::measurement(v) {
                assert!(
                    (length(&app).unwrap() - value).abs() <= epsilon,
                    "{}",
                    row["id"]
                );
            } else {
                assert!(length(&app).is_none());
            }
            assert!(app.object_prompt.is_none());
            enter(&mut app, "Redo");
            check(&app, &v["redo"]);
            enter(&mut app, "Undo");
            check(&app, &v["undo"]);
        }
    }
    assert_eq!(count, 42);
}
#[test]
fn mesh_and_subd_length_pointer_inputs_follow_appearance_in_every_display_mode() {
    use super::chain_selection::viewport::Pointer;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let data = observation::read(&base, "component_length_discovery");
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        for (kind, flat, id) in [
            ("mesh", false, "mesh_bent_grid_6_0"),
            ("subd", false, "subd_bent_grid_6_0"),
            ("subd", true, "subd_bent_grid_6_1"),
        ] {
            let Source { mut app, .. } = fixture(kind, "bent_grid", flat, false);
            enter(&mut app, "Length");
            Pointer::new(mode).click(&mut app, point(4.5, 3., 0.));
            assert_eq!(app.commands.component_selection().len(), 1);
            enter(&mut app, "");
            let reference = &data["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == id)
                .unwrap()["value"];
            let (expected, epsilon) = native::measurement(reference).unwrap();
            assert!(
                (length(&app).unwrap() - expected).abs() <= epsilon,
                "{kind} flat={flat} {mode:?}"
            );
            assert!(app.commands.component_selection().is_empty());
            assert!(app.object_prompt.is_none());
        }
    }
}
