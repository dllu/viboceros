fn audit_matrix(root: &std::path::Path, matrix: &str, count: usize) {
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/remove_knot_{matrix}.json")))
            .unwrap(),
    )
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(
                &row["value"],
                &root.join(format!("fixtures/remove_knot_{matrix}")),
            )
        })) {
            Ok(gap) => eprintln!(
                "REMOVE_KNOT_UI_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            Err(_) => {
                eprintln!("REMOVE_KNOT_UI_MISMATCH {matrix}/{name}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "Native UI mismatches: {failures:?}");
}
use super::insert_kink::comparison;
#[path = "../../../crates/viboceros-command/tests/support/remove_knot.rs"]
mod topology;
use super::*;
use serde_json::Value;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn replay(v: &Value, root: &std::path::Path) -> f64 {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let mut app = test_app();
    let (document, _, _, views) =
        viboceros_command::open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap();
    app.document = document;
    app.restore_file_viewports(views);
    let id = v["source_id"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    app.document.clear_selection();
    if v["before"][0]["selected"] == true {
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
    }
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(
        &mut app,
        if r["subcurve"].is_null() {
            "RemoveKnot"
        } else {
            "RemoveKnot SubCrv"
        },
    );
    assert_eq!(
        app.refinement_prompt.as_ref().unwrap().command_name(),
        "RemoveKnot"
    );
    if app.refinement_prompt.as_ref().unwrap().selecting() {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    if let Some(range) = r["subcurve"].as_array() {
        let curve = before[0].geometry().curve_ref().unwrap();
        for fraction in range {
            let p = curve
                .evaluate(curve.parameter_at(fraction.as_f64().unwrap()).unwrap())
                .unwrap();
            enter(&mut app, &format!("w{}", format_model_point(p)));
        }
    }
    for token in v["tokens"].as_array().unwrap() {
        let token = token.as_str().unwrap();
        if token == "!" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if token == "_Enter" { "" } else { token });
        }
        if app.refinement_prompt.is_some() {
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: staged edit"
            );
            assert!(!app.document.can_undo());
        }
    }
    if app.refinement_prompt.is_some() && !v["success"].as_bool().unwrap() {
        app.cancel_current_prompt_or_selection();
    }
    assert!(app.refinement_prompt.is_none(), "{name}: command ended");
    let mut gap = topology::check(&app.document, &v["after"], id, name);
    if !v["undo"].is_null() {
        assert_eq!(app.document.undo_label(), Some("RemoveKnot"));
        enter(&mut app, "Undo");
        gap = gap.max(topology::check(&app.document, &v["undo"], id, name));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        gap = gap.max(topology::check(&app.document, &v["redo"], id, name));
    }
    assert!(gap <= 1e-8, "{name}: {gap}");
    gap
}
#[test]
fn remove_knot_all_native_getters_numeric_local_undo_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "command", 149);
}

fn source(name: &str) -> (VibocerosApp, ObjectId, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/remove_knot_command.json")).unwrap(),
    )
    .unwrap();
    let value = capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == name)
        .unwrap()["value"]
        .clone();
    let mut app = test_app();
    let (document, _, _, views) = viboceros_command::open_3dm_with_views(
        root.join("fixtures/remove_knot_command")
            .join(format!("{name}.3dm"))
            .to_str()
            .unwrap(),
    )
    .unwrap();
    app.document = document;
    app.restore_file_viewports(views);
    let id = value["source_id"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, id, value)
}
#[test]
fn remove_knot_previews_deformation_updates_picking_and_retains_redo_on_cancel() {
    for name in ["cubic_deformed", "surface_U_deformed"] {
        let (mut app, id, v) = source(name);
        app.execute_command("SetObjectName Future");
        enter(&mut app, "Undo");
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
        let original = app.document.object(id).unwrap().clone();
        assert!(app.document.can_redo());
        enter(&mut app, "RemoveKnot");
        enter(
            &mut app,
            v["tokens"][if name.starts_with("surface") { 1 } else { 0 }]
                .as_str()
                .unwrap(),
        );
        let prompt = app.refinement_prompt.as_ref().unwrap();
        let scene = prompt.scene().unwrap();
        assert_ne!(scene.object(id).unwrap().geometry(), original.geometry());
        if let Some(cursor) = prompt.cursor_curve() {
            assert_eq!(
                cursor,
                &scene
                    .object(id)
                    .unwrap()
                    .geometry()
                    .curve_ref()
                    .unwrap()
                    .to_nurbs()
                    .unwrap()
            );
        }
        assert_eq!(app.document.object(id), Some(&original));
        assert!(!app.document.can_undo());
        assert!(app.document.can_redo());
        enter(&mut app, "Undo");
        assert_eq!(
            app.refinement_prompt
                .as_ref()
                .unwrap()
                .scene()
                .unwrap()
                .object(id)
                .unwrap()
                .geometry(),
            original.geometry()
        );
        app.cancel_current_prompt_or_selection();
        assert!(!app.document.is_selected(id));
        assert!(app.document.can_redo());
        assert_eq!(app.document.object(id), Some(&original));
    }
}
#[test]
fn remove_knot_scene_refreshes_background_and_rejects_stale_sources() {
    let (mut app, id, v) = source("cubic_deformed");
    enter(&mut app, "RemoveKnot");
    enter(&mut app, v["tokens"][0].as_str().unwrap());
    let other = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(19., 2., 3.).unwrap()))
        .unwrap();
    app.validate_refinement();
    assert!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .object(other)
            .is_some()
    );
    app.document
        .replace_object_geometries([(id, Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))])
        .unwrap();
    app.validate_refinement();
    assert!(app.refinement_prompt.is_none());
}
#[test]
fn remove_knot_options_stay_independent_and_local_undo_keeps_the_getter() {
    let (mut app, id, v) = source("cubic");
    enter(&mut app, "RemoveKnot");
    enter(&mut app, "Undo");
    assert!(app.refinement_prompt.is_some());
    enter(&mut app, "Direction=V");
    assert_eq!(
        app.commands.remove_knot_options().direction,
        viboceros_geometry::SurfaceKnotDirection::V
    );
    assert_eq!(
        app.commands.insert_knot_options().direction,
        viboceros_geometry::SurfaceKnotDirection::U
    );
    enter(&mut app, "Direction=Both");
    assert!(app.command_log.back().unwrap().contains("Error"));
    assert_eq!(
        app.commands.remove_knot_options().direction,
        viboceros_geometry::SurfaceKnotDirection::V
    );
    enter(&mut app, v["tokens"][0].as_str().unwrap());
    enter(&mut app, "Undo");
    enter(&mut app, "");
    assert!(app.refinement_prompt.is_none());
    assert!(!app.document.is_selected(id));
    assert!(
        app.document
            .object(id)
            .unwrap()
            .geometry_user_text()
            .is_empty()
    );
}

#[test]
fn followup_merged_polycurve_clocks_repeated_picks_and_deformed_periodic_tensors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "followup", 6);
}

#[test]
fn smooth_periodic_surface_deformation_repeated_picks_and_undo() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "smooth_periodic", 2);
}
