fn audit_matrix(root: &std::path::Path, matrix: &str, count: usize) {
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/insert_knot_{matrix}.json")))
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
                &root.join(format!("fixtures/insert_knot_{matrix}")),
            )
        })) {
            Ok(gap) => eprintln!(
                "INSERT_KNOT_UI_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            Err(_) => {
                eprintln!("INSERT_KNOT_UI_MISMATCH {matrix}/{name}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "Native UI mismatches: {failures:?}");
}
use super::insert_kink::comparison;
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
    app.document =
        viboceros_command::open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
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
            "InsertKnot"
        } else {
            "InsertKnot SubCrv"
        },
    );
    assert_eq!(
        app.refinement_prompt.as_ref().unwrap().command_name(),
        "InsertKnot"
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
    let mut gap = comparison::check(&app.document, &v["after"], id, name);
    if !v["undo"].is_null() {
        assert_eq!(app.document.undo_label(), Some("InsertKnot"));
        enter(&mut app, "Undo");
        gap = gap.max(comparison::check(&app.document, &v["undo"], id, name));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        gap = gap.max(comparison::check(&app.document, &v["redo"], id, name));
    }
    assert!(gap <= 1e-8, "{name}: {gap}");
    gap
}
#[test]
fn insert_knot_all_native_getters_numeric_automatic_local_undo_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "command", 156);
    audit_matrix(&root, "followup", 49);
}

#[test]
fn insert_knot_native_symmetric_complement_and_center_picks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "symmetry", 19);
}

#[test]
fn insert_knot_guides_local_undo_redo_retention_and_invalid_source() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    enter(&mut app, "Undo");
    assert!(app.document.can_redo());
    enter(&mut app, "InsertKnot Midpoints=Yes");
    let markers = app.refinement_prompt.as_ref().unwrap().markers().unwrap();
    assert_eq!(markers.len(), 1);
    assert!((markers[0].x() - 3.).abs() < 1e-12);
    enter(&mut app, "Automatic");
    assert_eq!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .markers()
            .unwrap()
            .len(),
        2
    );
    enter(&mut app, "Automatic");
    assert_eq!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .markers()
            .unwrap()
            .len(),
        4
    );
    enter(&mut app, "Undo");
    assert_eq!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .markers()
            .unwrap()
            .len(),
        2
    );
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    enter(&mut app, "Symmetrical=Yes");
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
    assert!(app.commands.insert_knot_options().symmetrical);
    assert!(!app.commands.insert_kink_options().symmetrical);
    enter(&mut app, "InsertKnot");
    app.document
        .replace_object_geometries([(id, Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))])
        .unwrap();
    app.validate_refinement();
    assert!(app.refinement_prompt.is_none());
}

#[test]
fn insert_knot_viewport_parameters_relative_points_and_filters_share_input() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "InsertKnot");
    assert!(app.handle_viewport_action(ViewportOutput {
        parametric_click: Some(vec![crate::viewport::ParametricPick {
            object: id,
            member: None,
            face: None
        }]),
        ..Default::default()
    }));
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(2.),
        ..Default::default()
    }));
    enter(&mut app, "End");
    enter(&mut app, "r1,0,0");
    assert!(app.one_shot_snap_label().is_none());
    enter(&mut app, ".x");
    enter(&mut app, "4,20,0");
    enter(&mut app, "0,0,0");
    enter(&mut app, "5");
    enter(&mut app, "99");
    assert!(app.command_log.back().unwrap().contains("Error"));
    assert!(app.handle_viewport_action(ViewportOutput {
        enter_pressed: true,
        ..Default::default()
    }));
    let Geometry::NurbsCurve(curve) = app.document.object(id).unwrap().geometry() else {
        panic!("nurbs")
    };
    assert_eq!(curve.knots(), &[0., 0., 2., 3., 4., 5., 6., 6.]);
    assert_eq!(app.document.undo_label(), Some("InsertKnot"));
}
