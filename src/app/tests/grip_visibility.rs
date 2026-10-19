use super::*;
use serde_json::Value;
use viboceros_document::ControlPointId;
use viboceros_oracle::grip_visibility::snapshot_with_geometry;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-8,
            "{path}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, a) in a {
                close(a, &b[k], &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
fn check(doc: &Document, ids: &[ObjectId], peer: ObjectId, expected: &Value, name: &str) {
    let actual = snapshot_with_geometry(
        doc,
        ids,
        peer,
        expected["sources"][0].get("curve").is_some(),
    );
    close(&actual, expected, name);
}
fn seed(s: &str, n: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    match s {
        "first" => vec![0],
        "last" => vec![n - 1],
        "middle" => vec![n / 2],
        "before_middle" => vec![(n / 2).saturating_sub(1)],
        "ends" => vec![0, n - 1],
        "all" => (0..n).collect(),
        "empty" => Vec::new(),
        _ => panic!("invalid seed"),
    }
}
#[test]
fn complete_native_grip_visibility_application_states_and_getters() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/grip_visibility.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/grip_visibility.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 39);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let name = row["id"].as_str().unwrap();
        assert_eq!(op["id"], row["id"]);
        let (doc, _, _, _) = viboceros_command::open_3dm_with_views(
            root.join(format!("fixtures/grip_visibility/{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let mut app = test_app();
        app.document = doc;
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        for (id, s) in ids.iter().zip(op["sources"].as_array().unwrap()) {
            if s["mode"] == "edit" {
                app.document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
                enter(&mut app, "EditPtOn");
            } else {
                app.document.enable_control_points([*id]).unwrap();
            }
        }
        let peer = app
            .document
            .add_geometry(Geometry::Point(point(20., 20., 20.)))
            .unwrap();
        let marker = app
            .document
            .add_geometry(Geometry::Point(point(30., 30., 30.)))
            .unwrap();
        app.document.undo().unwrap();
        for (id, s) in ids.iter().zip(op["sources"].as_array().unwrap()) {
            if s["parent"] == true {
                app.document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
            }
        }
        app.document
            .select_objects_direct([peer], SelectionMode::Add)
            .unwrap();
        let edited = op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| matches!(a["command"].as_str(), Some("Move" | "Undo" | "Redo")));
        let originals = ids
            .iter()
            .map(|id| app.document.object(*id).unwrap().clone())
            .collect::<Vec<_>>();
        check(&app.document, &ids, peer, &row["value"]["before"], name);
        for (action, step) in op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["value"]["steps"].as_array().unwrap())
        {
            if let Some(parent) = action["parents"].as_bool() {
                app.document
                    .select_objects_direct(
                        ids.iter().copied(),
                        if parent {
                            SelectionMode::Add
                        } else {
                            SelectionMode::Remove
                        },
                    )
                    .unwrap();
            }
            if let Some(s) = action["seed"].as_str() {
                let old = app
                    .document
                    .selected_logical_control_points()
                    .map(|(g, _)| g)
                    .collect::<std::collections::BTreeSet<_>>();
                let mut picks = Vec::new();
                for id in &ids {
                    let n = app
                        .document
                        .control_point_locations(*id)
                        .map_or(0, |p| p.len());
                    picks.extend(
                        seed(s, n)
                            .into_iter()
                            .map(|index| ControlPointId { object: *id, index }),
                    );
                }
                picks.retain(|g| app.document.is_control_point_selectable(*g) || old.contains(g));
                app.document.clear_control_point_selection();
                app.document
                    .select_control_points(picks, SelectionMode::Add)
                    .unwrap();
            }
            let label = format!("{name}: {}", step["command"]);
            check(&app.document, &ids, peer, &step["before"], &label);
            let command = action["command"].as_str().unwrap();
            if command == "SelUV" {
                let p = app.document.control_point_locations(ids[0]).unwrap();
                let point = p[p.len() / 2];
                enter(&mut app, "SelUV Direction=Both HideOthers=No");
                if app.select_uv_prompt.as_ref().unwrap().selecting() {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(ids[0]),
                        mode: SelectionMode::Replace,
                    });
                }
                assert!(app.select_uv_point(point));
                enter(&mut app, "");
            } else {
                enter(&mut app, command);
                if step["getter"]["opened"] == true {
                    assert_eq!(
                        app.object_prompt.as_ref().unwrap().description.command,
                        if command == "Move" { "Move" } else { "HidePt" },
                        "{label}: {:?}",
                        app.command_log
                    );
                    if action["pick"] == "all" {
                        enter(&mut app, "SelPt");
                    }
                    if action["cancel"] == true || action["pick"] != "all" {
                        app.cancel_current_prompt_or_selection();
                    } else {
                        enter(&mut app, "");
                    }
                } else if command == "Move" {
                    enter(&mut app, "w0,0,0");
                    enter(&mut app, "w0,0,1");
                }
            }
            assert!(app.command_line_idle(), "{label}: {:?}", app.command_log);
            check(&app.document, &ids, peer, &step["after_script"], &label);
            for (id, original) in ids.iter().zip(&originals) {
                let object = app.document.object(*id).unwrap();
                if !edited {
                    assert_eq!(object, original, "{label}");
                }
                assert_eq!(object.attributes(), original.attributes(), "{label}");
                assert_eq!(object.group_ids(), original.group_ids(), "{label}");
            }
            if !edited {
                assert!(app.document.can_redo(), "{label}");
            }
        }
        if app.document.can_redo() {
            app.document.redo().unwrap();
        }
        assert_eq!(
            app.document.object(marker).is_some(),
            row["value"]["redo_retained"] == true
        );
    }
}
#[test]
fn hiding_inside_transform_getters_and_stale_hidden_mouse_events() {
    let mut app = test_app();
    enter(&mut app, "Curve 0,0,0 1,2,0 2,-1,0 3,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.enable_control_points([id]).unwrap();
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    enter(&mut app, "HidePt");
    enter(&mut app, "SetPt XSet=No YSet=No ZSet=Yes");
    enter(&mut app, "SelPt");
    let before = format!("{:?}", app.document);
    enter(&mut app, "HidePt invalid");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "HidePt");
    assert_eq!(app.document.control_points().count(), 0);
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.command,
        "SetPt"
    );
    enter(&mut app, "ShowPt");
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    enter(&mut app, "HidePt");
    enter(&mut app, "SelPt");
    assert!(
        !app.document
            .selected_control_points()
            .any(|(g, _)| g.index == 0)
    );
    enter(&mut app, "");
    enter(&mut app, "0,0,5");
    let points = app.document.control_point_locations(id).unwrap();
    assert_eq!(points[0].z(), 0.);
    assert!(points[1..].iter().all(|p| p.z() == 5.));
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(8., 0., 0.)))
        .unwrap();
    app.document
        .select_objects_direct([peer], SelectionMode::Add)
        .unwrap();
    let output = ViewportOutput {
        control_point_selection: Some(crate::viewport::ControlPointSelection {
            picks: vec![ControlPointId {
                object: id,
                index: 0,
            }],
            mode: SelectionMode::Replace,
        }),
        ..Default::default()
    };
    assert!(app.handle_viewport_action(output));
    assert!(app.document.is_selected(peer));
    assert_eq!(
        app.document.is_control_point_hidden(ControlPointId {
            object: id,
            index: 0
        }),
        Some(true)
    );
    enter(&mut app, "ShowPt");
    assert_eq!(
        app.document
            .control_points()
            .filter(|(g, _, _)| g.object == id)
            .count(),
        4
    );
}
