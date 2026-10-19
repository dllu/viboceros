use super::*;
use serde_json::{Value, json};
use viboceros_command::open_3dm_with_views;
use viboceros_document::ControlPointId;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn snapshot(doc: &Document, ids: &[ObjectId], peer: ObjectId) -> Value {
    json!({"sources":ids.iter().map(|id|json!({"selected":doc.is_selected(*id),
        "grips":doc.control_points().filter(|(g,_,_)|g.object==*id).map(|(g,p,s)|
            json!({"index":g.index,"point":p.to_array(),"selected":s})).collect::<Vec<_>>()
    })).collect::<Vec<_>>(),"peer_selected":doc.is_selected(peer)})
}
fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-8,
                "{path}: {a} != {b}"
            );
        }
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

#[test]
fn directional_grip_commands_match_all_native_application_states() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("fixtures/grip_navigation.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("observations/grip_navigation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 25);
    let mut cases = 0;
    let mut steps = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let name = op["id"].as_str().unwrap();
        let path = folder
            .join("fixtures/grip_navigation")
            .join(format!("{name}.3dm"));
        let (doc, _, _, _) = open_3dm_with_views(path.to_str().unwrap()).unwrap();
        let mut app = test_app();
        app.document = doc;
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        for (id, source) in ids.iter().zip(op["sources"].as_array().unwrap()) {
            if source["mode"] == "edit" {
                app.document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
                enter(&mut app, "EditPtOn");
            } else {
                // Match the native fixture's direct GripsOn preparation; it
                // leaves earlier edit owners and their selections alone.
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
        for (id, source) in ids.iter().zip(op["sources"].as_array().unwrap()) {
            if source["parent"] == true {
                app.document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
            }
        }
        app.document
            .select_objects_direct([peer], SelectionMode::Add)
            .unwrap();
        close(
            &snapshot(&app.document, &ids, peer),
            &row["value"]["before"],
            name,
        );
        let originals = ids
            .iter()
            .map(|id| app.document.object(*id).unwrap().clone())
            .collect::<Vec<_>>();
        for (action, expected) in op["actions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["value"]["steps"].as_array().unwrap())
        {
            if let Some(seed) = action["seed"].as_str() {
                let mut picks = Vec::new();
                for id in &ids {
                    let n = app.document.control_point_locations(*id).unwrap().len();
                    let indices = match seed {
                        "first" => vec![0],
                        "last" => vec![n - 1],
                        "middle" => vec![n / 2],
                        "ends" => vec![0, n - 1],
                        "all" => (0..n).collect(),
                        "empty" => vec![],
                        _ => unreachable!(),
                    };
                    picks.extend(
                        indices
                            .into_iter()
                            .map(|index| ControlPointId { object: *id, index }),
                    );
                }
                app.document.clear_control_point_selection();
                app.document
                    .select_control_points(picks, SelectionMode::Add)
                    .unwrap();
            }
            let context = format!("{name}/{steps}/{}", action["command"]);
            close(
                &snapshot(&app.document, &ids, peer),
                &expected["before"],
                &context,
            );
            enter(&mut app, action["command"].as_str().unwrap());
            assert!(
                app.object_prompt.is_none() && app.active_command.is_none(),
                "{context}"
            );
            close(
                &snapshot(&app.document, &ids, peer),
                &expected["after"],
                &context,
            );
            for (id, old) in ids.iter().zip(&originals) {
                assert_eq!(app.document.object(*id), Some(old), "{context}");
            }
            steps += 1;
        }
        enter(&mut app, "Redo");
        assert!(app.document.object(marker).is_some(), "{name}");
        cases += 1;
    }
    eprintln!(
        "GRIP_NAVIGATION_UI_AUDIT {}",
        json!({"cases":cases,"commands":steps})
    );
}

#[test]
fn navigation_stays_inside_the_set_point_source_getter() {
    let mut app = test_app();
    enter(&mut app, "ControlPointCurve 2 0,0,0 2,3,0 4,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "PointsOn");
    let before = app.document.control_point_locations(id).unwrap().to_vec();
    enter(&mut app, "SetPt XSet=No YSet=No ZSet=Yes");
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    enter(&mut app, "NextV");
    assert!(app.object_prompt.is_some());
    assert_eq!(
        app.document
            .selected_control_points()
            .map(|(g, _)| g.index)
            .collect::<Vec<_>>(),
        [1]
    );
    let state = format!("{:?}", app.document);
    enter(&mut app, "AddNextU bad");
    assert_eq!(format!("{:?}", app.document), state);
    enter(&mut app, "AddNextU");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "");
    enter(&mut app, "0,0,5");
    let after = app.document.control_point_locations(id).unwrap();
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1].z(), 5.);
    assert_eq!(after[2].z(), 5.);
    enter(&mut app, "Undo");
    assert_eq!(app.document.control_point_locations(id).unwrap(), before);
}
