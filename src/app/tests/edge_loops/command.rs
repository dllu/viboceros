use super::*;

fn snapshot(app: &VibocerosApp, id: ObjectId, value: &Value) {
    assert_eq!(
        app.document.is_selected(id),
        value["whole"].as_bool().unwrap()
    );
    assert_eq!(selected(app), indices(&value["ordinals"]));
    assert_eq!(
        app.document.objects().len(),
        value["objects"].as_u64().unwrap() as usize
    );
}
#[test]
fn standalone_native_workflows_keep_seeds_preselection_cancel_and_redo() {
    let mut count = 0;
    for family in ["edge_loop_workflow_discovery", "edge_loop_workflows"] {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
        let query: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let raw = std::fs::read(base.join(format!("observations/{family}.json.gz"))).unwrap();
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(raw.as_slice())
            .read_to_end(&mut data)
            .unwrap();
        let data: Value = serde_json::from_slice(&data).unwrap();
        for (row, op) in data["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(query["operations"].as_array().unwrap())
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
            let before = indices(&v["before"]["ordinals"]);
            seed(
                &mut app,
                before.into_iter().map(|i| pick(id, mesh, i)).collect(),
            );
            if let Some(kind) = op["component"].as_str() {
                let kind = match (kind, mesh) {
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
            enter(&mut app, "_SelEdgeLoop");
            if app.edge_loop_prompt.is_some() {
                assert!(!app.command_line_idle());
                assert!(app.viewport_object_filter().is_none());
            }
            for (step, expected) in op["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(v["trace"].as_array().unwrap())
            {
                let pair = indices(&step["pair"]);
                let index = v["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .position(|e| {
                        let e = indices(e);
                        e == pair || e == [pair[1], pair[0]]
                    })
                    .unwrap();
                app.accept_component_click(ComponentClick {
                    picks: vec![pick(id, mesh, index)],
                    preselection: false,
                    modifiers: if step["modifiers"] == "ctrl" {
                        egui::Modifiers::CTRL
                    } else {
                        egui::Modifiers::NONE
                    },
                });
                snapshot(&app, id, expected);
            }
            if app.edge_loop_prompt.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            assert!(app.edge_loop_prompt.is_none(), "{}", row["id"]);
            snapshot(&app, id, &v["after"]);
            assert_eq!(app.document.object(id).unwrap(), &source);
            assert!(app.document.can_redo());
            enter(&mut app, "Redo");
            snapshot(&app, id, &v["redo"]);
            enter(&mut app, "Undo");
            snapshot(&app, id, &v["undo"]);
            if !selected(&app).is_empty() {
                enter(&mut app, "ToNURBS");
                for _ in 0..3 {
                    if app.object_prompt.is_some() {
                        enter(&mut app, "");
                    }
                }
                assert!(app.object_prompt.is_none());
                assert!(selected(&app).is_empty());
                assert!(app.document.can_undo());
            }
        }
    }
    assert_eq!(count, 44);
}

#[test]
fn standalone_loop_choices_and_stale_sources_do_not_silently_commit() {
    let data = capture();
    let (mut app, id) = fixture(&data["results"][0]["value"]);
    let mesh = matches!(
        app.document.object(id).unwrap().geometry(),
        Geometry::Mesh(_)
    );
    enter(&mut app, "SelEdgeLoop");
    app.accept_component_click(ComponentClick {
        picks: vec![pick(id, mesh, 11), pick(id, mesh, 12)],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(selected(&app).is_empty());
    enter(&mut app, "2");
    assert_eq!(selected(&app), [12]);
    let expected = app
        .edge_loop_prompt
        .as_ref()
        .unwrap()
        .preview()
        .iter()
        .map(|p| p.index)
        .collect::<Vec<_>>();
    assert!(expected.len() > 1);
    app.document
        .select_objects_direct([id], SelectionMode::Add)
        .unwrap();
    app.execute_command("Hide");
    app.validate_edge_loop();
    assert!(app.edge_loop_prompt.is_none());
    assert!(selected(&app).is_empty());
}
