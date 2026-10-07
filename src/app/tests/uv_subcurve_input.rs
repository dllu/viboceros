use super::*;
use serde_json::Value;
use viboceros_document::SelectionMode;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0");
    enter(&mut app, "Line 0,0 4,6");
    enter(&mut app, "Rectangle 0,0 4,6");
    let ids = app.document.objects().map(|o| o.id()).collect();
    app.document.clear_history().unwrap();
    app.document.clear_selection();
    (app, ids)
}

#[test]
fn apply_inline_subcurve_accepts_clicks_and_typed_endpoints_without_source_edits() {
    let (mut app, ids) = setup();
    let original = app.document.object(ids[1]).unwrap().geometry().clone();
    enter(&mut app, "ApplyCrv");
    enter(&mut app, "SubCrv");
    assert_eq!(
        app.intersection_prompt.as_ref().unwrap().filter(),
        viboceros_command::ObjectSelectionFilter::Curves
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert!(app.accept_drafting_point(point(1., 1.5, 0.)));
    enter(&mut app, "3,4.5,0");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .len(),
        1
    );
    assert_eq!(app.document.objects().len(), 3);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, &ids[2].to_string());
    enter(&mut app, "");
    enter(&mut app, &ids[0].to_string());
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.object(ids[1]).unwrap().geometry(), &original);
    let output = app
        .document
        .selected_objects()
        .find(|o| o.attributes().name().is_none())
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!(
        output
            .start_point()
            .unwrap()
            .distance_to(point(1., 1.5, 0.))
            .unwrap()
            < 1e-8
    );
    assert!(
        output
            .end_point()
            .unwrap()
            .distance_to(point(3., 4.5, 0.))
            .unwrap()
            < 1e-8
    );
    assert_eq!(app.document.undo_label(), Some("ApplyCrv"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 2);
}

#[test]
fn create_inline_subcurves_repeat_reverse_and_reject_coincident_endpoints() {
    let (mut app, ids) = setup();
    enter(&mut app, "CreateUVCrv");
    enter(&mut app, &ids[0].to_string());
    for (start, end) in [("3,4.5", "1,1.5"), ("1.6,2.4", "2.4,3.6")] {
        enter(&mut app, "SubCrv");
        enter(&mut app, &ids[1].to_string());
        enter(&mut app, start);
        enter(&mut app, start);
        assert!(
            app.intersection_prompt
                .as_ref()
                .unwrap()
                .uv_subcurves
                .pending
                .is_some()
        );
        enter(&mut app, end);
    }
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 6);
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(!app.document.is_selected(ids[1]));
    let outputs = app
        .document
        .objects()
        .filter(|o| !ids.contains(&o.id()) && o.attributes().name().is_none())
        .collect::<Vec<_>>();
    assert!(
        outputs[1]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .distance_to(point(3., 4.5, 0.))
            .unwrap()
            < 1e-8
    );
}

#[test]
fn cancelling_nested_uv_subcurve_restores_initial_selection_and_history() {
    for command in ["ApplyCrv", "CreateUVCrv"] {
        for stage in 0..3 {
            let (mut app, ids) = setup();
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
            let before = app
                .document
                .objects()
                .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
                .collect::<Vec<_>>();
            let selection = app.document.selected_object_ids().collect::<Vec<_>>();
            enter(&mut app, command);
            if command == "ApplyCrv" {
                assert!(app.intersection_prompt.as_ref().unwrap().first.is_none());
            }
            enter(&mut app, "SubCrv");
            if stage > 0 {
                enter(&mut app, &ids[1].to_string());
            }
            if stage > 1 {
                enter(&mut app, "1,1.5");
            }
            app.cancel_current_prompt_or_selection();
            assert!(app.active_command.is_none());
            assert!(app.intersection_prompt.is_none());
            assert_eq!(
                app.document
                    .objects()
                    .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                selection
            );
            assert!(!app.document.can_undo());
            assert!(!app.document.can_redo());
        }
    }
}

#[test]
fn selnone_discards_temporary_ranges_and_another_command_cancels_nested_input() {
    let (mut app, ids) = setup();
    enter(&mut app, "CreateUVCrv");
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "1,1.5");
    enter(&mut app, "3,4.5");
    enter(&mut app, "SelNone");
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .ranges
            .is_empty()
    );
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 4);
    enter(&mut app, "Undo");
    enter(&mut app, "ApplyCrv");
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "1,1.5");
    enter(&mut app, "Line 10,0 11,1");
    assert!(app.intersection_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("Line"));
}

#[test]
fn numeric_inline_confirmation_replays_native_lengths_orientation_and_clamping() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_numeric_followup.json"
    ))
    .unwrap();
    for recipe in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["value"]["command"] != "SubCrv")
    {
        let v = &recipe["value"];
        let case = recipe["id"]
            .as_str()
            .unwrap()
            .strip_prefix("numeric_followup_")
            .unwrap();
        let mut app = test_app();
        let mut ids = Vec::new();
        for row in v["before"].as_array().unwrap() {
            let d = &row["definition"];
            let controls = d["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    viboceros_geometry::WeightedPoint3::try_new(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                        )
                        .unwrap(),
                        c["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            let floats = |x: &Value| serde_json::from_value::<Vec<f64>>(x.clone()).unwrap();
            let g = if row["kind"] == "curve" {
                Geometry::NurbsCurve(
                    viboceros_geometry::NurbsCurve::try_new_rational(
                        d["degree"].as_u64().unwrap() as usize,
                        controls,
                        floats(&d["knots"]),
                    )
                    .unwrap(),
                )
            } else {
                Geometry::NurbsSurface(
                    viboceros_geometry::NurbsSurface::try_new_rational(
                        d["degree"][0].as_u64().unwrap() as usize,
                        d["degree"][1].as_u64().unwrap() as usize,
                        d["control_count"][0].as_u64().unwrap() as usize,
                        d["control_count"][1].as_u64().unwrap() as usize,
                        controls,
                        floats(&d["knots_u"]),
                        floats(&d["knots_v"]),
                    )
                    .unwrap(),
                )
            };
            ids.push(app.document.add_geometry(g).unwrap());
        }
        let original = app
            .document
            .objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>();
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        let apply = v["command"] == "ApplyCrv";
        enter(&mut app, v["command"].as_str().unwrap());
        if !apply {
            enter(&mut app, &ids[0].to_string());
        }
        enter(&mut app, "SubCrv");
        if case == "empty_nested" {
            enter(&mut app, "");
        } else {
            enter(&mut app, &ids[1].to_string());
            let start = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(v["start_point"].clone()).unwrap(),
            )
            .unwrap();
            assert!(app.accept_drafting_point(start));
            if case == "point_control" {
                enter(&mut app, "3,4.5,0");
            } else if case == "zero" {
                enter(&mut app, "0");
            } else {
                enter(&mut app, v["length_token"].as_str().unwrap());
                assert!(app.point_constraint.is_none());
                if case == "replace_number" {
                    enter(&mut app, "1");
                }
                if case == "confirm_unavailable" {
                    assert!(
                        app.intersection_prompt
                            .as_ref()
                            .unwrap()
                            .uv_subcurves
                            .pending
                            .as_ref()
                            .unwrap()
                            .length
                            .is_none()
                    );
                    enter(&mut app, "3,4.5,0");
                } else if matches!(case, "number_only" | "number_reference") {
                    if case == "number_reference" {
                        enter(&mut app, &ids[2].to_string());
                    }
                    enter(&mut app, "");
                } else {
                    let confirmation = if case == "confirm_coordinates" || case == "replace_number"
                    {
                        point(3., 4.5, 0.)
                    } else if case == "confirm_empty" {
                        point(5., 5., 0.)
                    } else {
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(v["confirmation"].clone()).unwrap(),
                        )
                        .unwrap()
                    };
                    assert!(app.accept_drafting_point(confirmation), "{case}");
                }
            }
        }
        assert_eq!(app.document.objects().len(), ids.len(), "{case}");
        assert!(!app.document.can_undo());
        if apply {
            enter(&mut app, &ids[3].to_string());
        }
        enter(&mut app, "");
        if apply {
            enter(&mut app, &ids[0].to_string());
        }
        assert!(app.intersection_prompt.is_none(), "{case}");
        let outputs = app
            .document
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        let native = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), native.len(), "{case}");
        for (expected, actual) in native.iter().zip(&outputs) {
            let curve = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
            for station in expected["samples"].as_array().unwrap() {
                let p =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(station.clone()).unwrap())
                        .unwrap();
                let t = curve
                    .closest_parameter(p, app.document.tolerance())
                    .unwrap();
                assert!(
                    curve.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                    "{case} {p:?}"
                );
            }
            let samples = expected["samples"].as_array().unwrap();
            assert!(
                curve
                    .evaluate(*curve.domain().start())
                    .unwrap()
                    .distance_to(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(samples[0].clone()).unwrap()
                        )
                        .unwrap()
                    )
                    .unwrap()
                    < 1e-6,
                "{case} start"
            );
            assert!(
                curve
                    .evaluate(*curve.domain().end())
                    .unwrap()
                    .distance_to(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(samples[32].clone()).unwrap()
                        )
                        .unwrap()
                    )
                    .unwrap()
                    < 1e-6,
                "{case} end"
            );
        }
        for (id, g) in original {
            assert_eq!(app.document.object(id).unwrap().geometry(), &g);
        }
        let output_ids = outputs.iter().map(|o| o.id()).collect::<BTreeSet<_>>();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), ids.len());
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.selected_object_ids().collect::<BTreeSet<_>>(),
            output_ids
        );
    }
}

#[test]
fn length_confirmation_retries_bad_quantities_and_cancels_without_edits() {
    let (mut app, ids) = setup();
    enter(&mut app, "ApplyCrv");
    enter(&mut app, "SubCrv");
    enter(&mut app, &ids[1].to_string());
    enter(&mut app, "1,1.5");
    for value in ["20", "NaN", "1/0"] {
        enter(&mut app, value);
        assert!(
            app.intersection_prompt
                .as_ref()
                .unwrap()
                .uv_subcurves
                .pending
                .as_ref()
                .unwrap()
                .length
                .is_none()
        );
    }
    enter(&mut app, "2mm");
    assert_eq!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .pending
            .as_ref()
            .unwrap()
            .length,
        Some(2.)
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.intersection_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
}
