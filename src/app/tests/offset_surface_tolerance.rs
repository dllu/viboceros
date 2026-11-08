use super::boolean_intersection::{enter, pick};
use super::offset_surface::inputs;
use super::offset_surface_loose::loose;
use super::{brep_comparison, plane_surface_native_support as native, *};
#[path = "../../../crates/viboceros-command/tests/support/offset_surface_tolerance.rs"]
#[allow(dead_code)]
mod tolerance;

#[test]
fn tolerance_getters_previews_and_cancellation_match_native() {
    replay(tolerance::rows(), "TOLERANCE_OFFSET");
}

#[test]
fn trimmed_getters_previews_and_cancellation_match_native() {
    replay(tolerance::trimmed_rows(), "TRIM_OFFSET");
}

#[test]
fn rational_pilot_getters_previews_and_cancellation_match_native() {
    replay(tolerance::rational_rows("pilot"), "RATIONAL_PILOT_OFFSET");
}

#[test]
fn rational_holdout_getters_previews_and_cancellation_match_native() {
    replay(
        tolerance::rational_rows("holdouts"),
        "RATIONAL_HOLDOUT_OFFSET",
    );
}

#[test]
fn rational_final_getters_previews_and_cancellation_match_native() {
    replay(tolerance::rational_rows("final"), "RATIONAL_FINAL_OFFSET");
}

fn replay(rows: Vec<(String, serde_json::Value)>, prefix: &str) {
    let mut count = 0;
    let mut declined = 0;
    for (id, v) in rows {
        println!("{prefix}_UI_CASE {id}");
        let mut app = test_app();
        app.document = tolerance::setup(&v);
        app.commands
            .remember_offset_surface_options(inputs::initial(&v))
            .unwrap();
        let state = inputs::State::capture(&app.document);
        tolerance::check(
            &app.document,
            &v["before"],
            false,
            1e-6,
            app.document.tolerance().absolute(),
        );
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "OffsetSrf");
        for (index, action) in v["prescribed_case"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let prompt = v["input_trace"][0]["prompts"][index].as_str().unwrap();
            if prompt.starts_with("Select surfaces or polysurfaces") {
                assert!(app.object_prompt.is_some());
            } else {
                let expected = if prompt.starts_with("Offset tolerance") {
                    Some("Tolerance")
                } else if prompt.starts_with("Offset distance") {
                    Some("Distance")
                } else {
                    None
                };
                assert_eq!(
                    app.offset_surface_session.as_ref().unwrap().getter(),
                    expected,
                    "{id}: {prompt}"
                );
            }
            if action == "Cancel" {
                app.cancel_current_prompt_or_selection();
            } else if action == "Enter" {
                enter(&mut app, "");
            } else if let Some(i) = action.get("curve").and_then(serde_json::Value::as_u64) {
                let id = inputs::guide(&app.document, i as usize);
                pick(&mut app, id);
            } else if let Some(n) = action.as_f64() {
                enter(&mut app, &n.to_string());
            } else if let Some(n) = action.get("number").and_then(serde_json::Value::as_str) {
                enter(&mut app, n);
            } else {
                enter(&mut app, action.as_str().unwrap());
            }
            if !app.command_line_idle() {
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    original,
                    "{id}: preview mutation"
                );
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
            }
        }
        assert_eq!(
            app.commands.offset_surface_options().tolerance,
            tolerance::options(&v).tolerance
        );
        if !tolerance::supported(&id, &v) {
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(app.document.can_redo());
            if cfg!(feature = "native-smlib") {
                assert!(
                    app.command_log.iter().any(|s| s.contains("outside")),
                    "{:?}",
                    app.command_log
                );
            }
            println!("{prefix}_UI_DECLINED {id}");
            app.cancel_current_prompt_or_selection();
            declined += 1;
            continue;
        }
        assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
        assert_eq!(
            app.commands.offset_surface_options().tolerance,
            tolerance::options(&v).tolerance
        );
        let field = if inputs::success(&v) {
            tolerance::field_error(&app.document, &v)
        } else {
            (0., 0.)
        };
        let mut record = state.verify_with(&mut app.document, &v, |d, r, a| {
            let check = if id == "offset_rf_two_axes_both" {
                tolerance::check_rational_partitions
            } else if prefix.starts_with("RATIONAL_") {
                tolerance::check_rational
            } else {
                tolerance::check
            };
            check(
                d,
                r,
                a,
                tolerance::epsilon(&v),
                tolerance::reference_tolerance(&v),
            )
        });
        if prefix.starts_with("RATIONAL_") {
            record["rational_topology"] = tolerance::rational_topology(&app.document, &v);
        }
        record["native_partition_difference"] = (id == "offset_rf_two_axes_both").into();
        record["id"] = id.into();
        record["maximum_field_error"] = field.0.into();
        record["native_maximum_field_error"] = field.1.into();
        record["native_reference_tolerance"] = tolerance::reference_tolerance(&v).into();
        println!("{prefix}_UI {record}");
        count += 1;
    }
    println!("{prefix}_UI_COUNT {count} DECLINED {declined}");
}

#[test]
fn tolerance_getter_owns_clicks_and_retains_preferences_across_documents() {
    let mut app = test_app();
    app.document = tolerance::setup(&tolerance::rows()[0].1);
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "OffsetSrf");
    enter(&mut app, "Tolerance");
    let source = inputs::guide(&app.document, 0);
    let preview = app
        .offset_surface_session
        .as_ref()
        .unwrap()
        .scene()
        .unwrap()
        .objects()
        .cloned()
        .collect::<Vec<_>>();
    app.handle_viewport_action(crate::viewport::ViewportOutput {
        face_click: Some((source, 0)),
        ..Default::default()
    });
    assert_eq!(
        app.offset_surface_session
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .cloned()
            .collect::<Vec<_>>(),
        preview
    );
    for text in ["-1", "NaN", "1,2,3"] {
        enter(&mut app, text);
        let s = app.offset_surface_session.as_ref().unwrap();
        assert_eq!(s.getter(), Some("Tolerance"));
    }
    enter(&mut app, ".0002");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    let t = Tolerance::try_new(0.003, 1e-9, 1e-6).unwrap();
    app.document.set_tolerance(t);
    enter(&mut app, "OffsetSrf");
    assert_eq!(
        app.commands.offset_surface_options().effective_tolerance(t),
        0.0002
    );
    enter(&mut app, "Tolerance");
    enter(&mut app, "Default");
    app.cancel_current_prompt_or_selection();
    let t = Tolerance::try_new(0.004, 1e-9, 1e-6).unwrap();
    app.document.set_tolerance(t);
    enter(&mut app, "OffsetSrf");
    assert_eq!(
        app.commands.offset_surface_options().effective_tolerance(t),
        0.004
    );
    enter(&mut app, "Loose=Yes");
    enter(&mut app, "Tolerance");
    assert_eq!(app.offset_surface_session.as_ref().unwrap().getter(), None);
    assert!(app.command_log.back().unwrap().contains("Loose=No"));
    app.cancel_current_prompt_or_selection();
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
}
