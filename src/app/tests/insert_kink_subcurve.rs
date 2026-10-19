use super::insert_kink::comparison;
use super::*;
use serde_json::Value;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn reference_length(v: &Value) -> f64 {
    if v["type"] == "polycurve" {
        return v["segments"]
            .as_array()
            .unwrap()
            .iter()
            .map(reference_length)
            .sum();
    }
    assert_eq!(v["type"], "nurbs");
    let d = &v["definition"];
    let curve = NurbsCurve::try_new_rational(
        d["degree"].as_u64().unwrap() as usize,
        d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                viboceros_geometry::WeightedPoint3::try_new(
                    point(&p["point"]),
                    p["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        d["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect(),
    )
    .unwrap();
    curve
        .length(viboceros_geometry::Tolerance::try_new(1e-12, 1e-13, 1e-10).unwrap())
        .unwrap()
}

fn case(v: &Value, root: &std::path::Path) -> Value {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let g = &r["getter"];
    let mut app = test_app();
    app.document =
        viboceros_command::open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
    app.viewports[0].set_world_view(ViewKind::Top).unwrap();
    let id = v["source_id"]
        .as_str()
        .unwrap()
        .parse::<ObjectId>()
        .unwrap();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let source = app
        .document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .to_owned();
    enter(&mut app, "InsertKink SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    let start = source
        .as_ref()
        .evaluate(
            source
                .as_ref()
                .parameter_at(g["start"].as_f64().unwrap())
                .unwrap(),
        )
        .unwrap();
    enter(&mut app, &format!("w{}", format_model_point(start)));
    if !g["lock_hover"].is_null() {
        app.handle_viewport_action(ViewportOutput {
            drafting_hover: Some(point(&v["input_trace"]["hover"])),
            ..Default::default()
        });
        assert!(app.subcurve_preview_hover().is_some());
    }
    let owned = v["input_trace"]["typed"].as_array().unwrap();
    let tokens = if owned.is_empty() {
        v["input_trace"]["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect::<Vec<_>>()
    } else {
        owned.iter().map(|x| x["token"].as_str().unwrap()).collect()
    };
    for token in tokens {
        if app.refinement_prompt.is_none() {
            break;
        }
        if token == "!" || token == "Escape" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if token == "_Enter" { "" } else { token });
        }
        if app.refinement_prompt.is_some() {
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: immutable draft"
            );
            assert!(!app.document.can_undo());
        }
    }
    if app.refinement_prompt.is_some() && !v["success"].as_bool().unwrap() {
        app.cancel_current_prompt_or_selection();
    }
    assert!(app.refinement_prompt.is_none(), "{name}: completed input");
    let native_output = v["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"].as_str() != Some(&id.to_string()));
    if name == "ellipse_whole" {
        assert_eq!(native_output.unwrap()["geometry"]["type"], "invalid_curve");
        assert_eq!(native_output.unwrap()["geometry"]["valid"], false);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        assert!(!app.document.can_redo());
        return serde_json::json!({"id":name,"status":"invalid_native_curve_guard","native_equivalent":false});
    }
    let mut maximum = comparison::check(&app.document, &v["after"], id, name);
    let mut native_length_error = 0.;
    let circle_deviation = matches!(name, "circle_lock_numeric" | "circle_lock_back_numeric");
    if circle_deviation {
        // These retained scripts return a 3.7699-unit piece for length 1.
        // Keep the requested exact length and record the native discrepancy.
        let curve = app
            .document
            .objects()
            .find(|o| o.id() != id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!((curve.length(app.document.tolerance()).unwrap() - 1.).abs() < 1e-10);
        let d = &native_output.unwrap()["geometry"]["domain"];
        assert!(
            (d[1].as_f64().unwrap() - d[0].as_f64().unwrap() - 3.7699111843077517).abs() < 1e-10
        );
        assert!(maximum > 1.);
    } else if let Some(output) = native_output
        && matches!(
            r["family"].as_str().unwrap(),
            "rational" | "ellipse" | "periodic"
        )
        && g["lengths"].as_array().is_some_and(|a| !a.is_empty())
    {
        let desired = v["input_trace"]["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str()?.parse::<f64>().ok())
            .next_back()
            .unwrap()
            .abs();
        let source_ref = source.as_ref();
        // An overlong number is rejected. The following point then supplies
        // the ordinary endpoint, so there is no accepted numeric length.
        if desired <= source_ref.length(app.document.tolerance()).unwrap() * (1. + 1e-8) {
            let mut intended = desired;
            if !source_ref.is_closed().unwrap() {
                let domain = &output["geometry"]["domain"];
                let anchor = source_ref
                    .parameter_at(g["start"].as_f64().unwrap())
                    .unwrap();
                let source_domain = source_ref.domain();
                let boundary = if domain[0].as_f64() == Some(*source_domain.start()) {
                    Some(*source_domain.start())
                } else if domain[1].as_f64() == Some(*source_domain.end()) {
                    Some(*source_domain.end())
                } else {
                    None
                };
                if let Some(end) = boundary {
                    intended = intended.min(
                        source
                            .try_subcurve(anchor, end)
                            .unwrap()
                            .as_ref()
                            .length(app.document.tolerance())
                            .unwrap(),
                    );
                }
            }
            native_length_error = (reference_length(&output["geometry"]) - intended).abs();
            assert!(
                native_length_error < 1e-6,
                "{name}: bounded native inverse-length residual {native_length_error}"
            );
            let curve = app
                .document
                .objects()
                .find(|o| o.id() != id)
                .unwrap()
                .geometry()
                .curve_ref()
                .unwrap();
            assert!(
                (curve.length(app.document.tolerance()).unwrap() - intended).abs() < 1e-8,
                "{name}: exact requested length"
            );
        }
    }
    if !v["undo"].is_null() {
        assert_eq!(app.document.undo_label(), Some("InsertKink"));
        enter(&mut app, "Undo");
        maximum = maximum.max(comparison::check(&app.document, &v["undo"], id, name));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        maximum = maximum.max(comparison::check(&app.document, &v["redo"], id, name));
    } else {
        assert!(!app.document.can_undo());
    }
    if !circle_deviation {
        assert!(
            maximum <= 1e-8 + native_length_error,
            "{name}: {maximum}, native inverse-length residual {native_length_error}"
        );
    }
    serde_json::json!({"id":name,"maximum_error":maximum,"native_length_error":native_length_error,"native_equivalent":maximum<=1e-8,"status":if circle_deviation {"native_circle_length_deviation"}else if maximum>1e-8 {"native_inverse_length_precision"}else {"match"}})
}
#[test]
fn insert_kink_subcurve_native_lengths_locks_correction_and_cancellation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("tools/rhino_oracle/observations/insert_kink_subcurve.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let folder = root.join("tools/rhino_oracle/fixtures/insert_kink_subcurve");
    let mut failures = Vec::new();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), 128);
    let mut outcomes = std::collections::BTreeMap::<String, usize>::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            case(&row["value"], &folder)
        })) {
            Ok(audit) => {
                *outcomes
                    .entry(audit["status"].as_str().unwrap().into())
                    .or_default() += 1;
                eprintln!("KINK_GETTER_AUDIT {audit}");
            }
            Err(e) => {
                let error = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                eprintln!(
                    "KINK_GETTER_MISMATCH {}",
                    serde_json::json!({"id":name,"error":error})
                );
                failures.push(name);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Native getter mismatches: {failures:?}"
    );
    assert_eq!(
        outcomes,
        std::collections::BTreeMap::from([
            ("match".into(), 112),
            ("native_inverse_length_precision".into(), 13),
            ("native_circle_length_deviation".into(), 2),
            ("invalid_native_curve_guard".into(), 1)
        ])
    );
}

#[test]
fn insert_kink_subcurve_replays_literal_direction_toggle_and_escape() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/insert_kink_subcurve_keys.json")).unwrap(),
    )
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let audit = case(
            &row["value"],
            &root.join("fixtures/insert_kink_subcurve_keys"),
        );
        assert_eq!(audit["status"], "match");
        eprintln!("KINK_GETTER_KEYS {audit}");
    }
}
#[test]
fn insert_kink_subcurve_previews_are_cached_read_only_and_return_to_parent() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "InsertKink SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "1,0,0");
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(Point3::try_new(5., 0., 0.).unwrap()),
        ..Default::default()
    });
    let first = app.subcurve_draft_preview().unwrap();
    let again = app.subcurve_draft_preview().unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.geometry, &again.geometry));
    assert_eq!(
        first.geometry.endpoints,
        [
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(5., 0., 0.).unwrap()
        ]
    );
    enter(&mut app, "D");
    assert!(
        app.refinement_prompt
            .as_ref()
            .unwrap()
            .hint()
            .contains("Locked")
    );
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(Point3::try_new(0., 0., 0.).unwrap()),
        ..Default::default()
    });
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, "Direction=Free");
    enter(&mut app, "-2");
    let preview = app.subcurve_draft_preview().unwrap();
    assert!(
        preview.geometry.endpoints[1]
            .distance_to(Point3::try_new(1., 0., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.refinement_prompt.as_ref().unwrap().selecting());
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, "SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "1,0,0");
    enter(&mut app, "2ft");
    assert!(app.refinement_prompt.is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.refinement_prompt.as_ref().unwrap().selecting());
    assert!(app.command_log.back().unwrap().contains("Select one curve"));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn insert_kink_subcurve_closed_locked_preview_matches_the_accepted_chart() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut app = test_app();
    app.document = viboceros_command::open_3dm_with_views(
        root.join("tools/rhino_oracle/fixtures/insert_kink_subcurve/circle_lock_wrong_point.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .0;
    let id = app.document.objects().next().unwrap().id();
    let source = app
        .document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .to_owned();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let at = |f| {
        source
            .as_ref()
            .evaluate(source.as_ref().parameter_at(f).unwrap())
            .unwrap()
    };
    enter(&mut app, "InsertKink SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, &format!("w{}", format_model_point(at(0.5))));
    app.update_subcurve_hover(at(0.8));
    enter(&mut app, "D");
    app.update_subcurve_hover(at(0.2));
    let preview = app.subcurve_draft_preview().unwrap();
    let again = app.subcurve_draft_preview().unwrap();
    assert!(std::sync::Arc::ptr_eq(&preview.geometry, &again.geometry));
    for (actual, expected) in preview.geometry.endpoints.iter().zip([at(0.5), at(1.)]) {
        assert!(actual.distance_to(expected).unwrap() < 1e-10);
    }
    enter(&mut app, &format!("w{}", format_model_point(at(0.2))));
    let accepted = app
        .refinement_prompt
        .as_ref()
        .unwrap()
        .cursor_curve()
        .unwrap();
    assert!(
        accepted
            .evaluate(*accepted.domain().end())
            .unwrap()
            .distance_to(at(1.))
            .unwrap()
            < 1e-10
    );
    app.cancel_refinement();
    enter(&mut app, "InsertKink SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, &format!("w{}", format_model_point(at(0.2))));
    app.update_subcurve_hover(at(0.8));
    let whole = source
        .as_ref()
        .length(app.document.tolerance())
        .unwrap()
        .to_string();
    enter(&mut app, &whole);
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, &format!("w{}", format_model_point(at(0.8))));
    assert!(app.refinement_prompt.as_ref().unwrap().selecting());
    app.cancel_refinement();
    assert!(!app.document.can_undo());
}

#[test]
fn insert_kink_subcurve_length_uses_model_units_and_keeps_redo_until_commit() {
    let mut app = test_app();
    app.execute_command("Units Millimeters");
    app.execute_command("Line 0,0,0 600,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    enter(&mut app, "Undo");
    app.document.clear_selection();
    enter(&mut app, "InsertKink");
    enter(&mut app, "SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "w100,0,0");
    enter(&mut app, "1ft");
    enter(&mut app, "w500,0,0");
    let curve = app
        .refinement_prompt
        .as_ref()
        .unwrap()
        .cursor_curve()
        .unwrap();
    assert!((curve.evaluate(*curve.domain().start()).unwrap().x() - 100.).abs() < 1e-10);
    assert!((curve.evaluate(*curve.domain().end()).unwrap().x() - 404.8).abs() < 1e-10);
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    enter(&mut app, "w200,0,0");
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_redo());
    assert_eq!(app.document.undo_label(), Some("InsertKink"));
}

#[test]
fn insert_kink_subcurve_rational_off_curve_picks_keep_exact_source_locus() {
    // The retained native input session did not complete for these picks.
    // Check independent geometric invariants without calling that a match.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut app = test_app();
    app.document=viboceros_command::open_3dm_with_views(root.join("tools/rhino_oracle/observations/insert_kink_subcurve_off_curve/rational_lock_back_numeric.3dm").to_str().unwrap()).unwrap().0;
    let id = app.document.objects().next().unwrap().id();
    let source = app
        .document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .to_owned();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    let at = |fraction| {
        source
            .as_ref()
            .evaluate(source.as_ref().parameter_at(fraction).unwrap())
            .unwrap()
    };
    enter(&mut app, "InsertKink SubCrv");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, &format!("w{}", format_model_point(at(0.8))));
    app.update_subcurve_hover(at(0.6));
    enter(&mut app, "D");
    enter(&mut app, "1");
    for f in [0.2, 0.5] {
        enter(&mut app, &format!("w{}", format_model_point(at(f))));
    }
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 2);
    let result = app
        .document
        .objects()
        .find(|o| o.id() != id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!((result.length(app.document.tolerance()).unwrap() - 1.).abs() < 1e-8);
    for n in 0..33 {
        let p = result
            .evaluate(result.parameter_at(n as f64 / 32.).unwrap())
            .unwrap();
        let closest = source
            .as_ref()
            .closest_parameter(p, app.document.tolerance())
            .unwrap();
        assert!(
            p.distance_to(source.as_ref().evaluate(closest).unwrap())
                .unwrap()
                < 1e-8
        );
    }
    assert_eq!(
        app.document
            .object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_owned(),
        source
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}
