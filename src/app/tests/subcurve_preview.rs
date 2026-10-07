use super::subcurve_direction::{enter, geometry, position};
use super::*;
use serde_json::Value;
use std::sync::Arc;

#[test]
fn subcurve_preview_replays_native_pending_results_without_model_or_history_changes() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_direction_grid.json"
    ))
    .unwrap();
    let mut cases = 0;
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        if v["success"] != true || v["finish_states"].as_array().unwrap().is_empty() {
            continue;
        }
        let case = v["case"].as_str().unwrap();
        let mut app = test_app();
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| app.document.add_geometry(geometry(row)).unwrap())
            .collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        if case.starts_with("inline_") {
            enter(&mut app, "CreateUVCrv");
            enter(&mut app, &ids[1].to_string());
            enter(&mut app, "SubCrv");
            enter(&mut app, &ids[0].to_string());
        } else {
            app.document.select_command_results([ids[0]]).unwrap();
            enter(&mut app, "SubCrv Copy=Yes FromMidpoint=No Mode=Shorten");
        }
        app.accept_drafting_point(position(&v["start"]));
        app.update_subcurve_hover(position(&v["aim"]));
        enter(&mut app, "D");
        enter(&mut app, v["length_token"].as_str().unwrap());
        app.update_subcurve_hover(position(&v["confirmation"]));
        let snapshots = app
            .document
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry_snapshot().clone(),
                    o.attributes().clone(),
                )
            })
            .collect::<Vec<_>>();
        let selected = app.document.selected_object_ids().collect::<Vec<_>>();
        let preview = app.subcurve_draft_preview().unwrap();
        assert!(preview.show_curve);
        let expected = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|o| o["source"].is_null())
            .unwrap();
        let curve = &preview.geometry.curve;
        for p in expected["samples"].as_array().unwrap() {
            let p = position(p);
            let t = curve
                .closest_parameter(p, app.document.tolerance())
                .unwrap();
            assert!(
                curve.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                "{case}"
            );
        }
        for (actual, expected) in preview
            .geometry
            .endpoints
            .iter()
            .zip([&expected["samples"][0], &expected["samples"][32]])
        {
            assert!(
                actual.distance_to(position(expected)).unwrap() < 1e-6,
                "{case}"
            );
        }
        for _ in 0..20 {
            assert!(Arc::ptr_eq(
                &preview.geometry,
                &app.subcurve_draft_preview().unwrap().geometry
            ));
        }
        for (id, snapshot, attributes) in snapshots {
            let object = app.document.object(id).unwrap();
            assert!(snapshot.shares_storage_with(object.geometry_snapshot()));
            assert_eq!(object.attributes(), &attributes);
        }
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        assert!(!app.document.can_undo());
        enter(&mut app, v["typed_inputs"][2].as_str().unwrap());
        assert!(app.subcurve_draft_preview().is_none(), "{case}");
        cases += 1;
    }
    assert_eq!(cases, 10);
}

#[test]
fn subcurve_preview_tracks_options_locked_side_midpoint_and_cancellation() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let id = app.document.objects().next().unwrap().id();
    app.document.select_command_results([id]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "SubCrv Copy=Yes");
    enter(&mut app, "2,3");
    app.update_subcurve_hover(point(3.2, 4.8, 0.));
    let shorten = app.subcurve_draft_preview().unwrap();
    assert!(shorten.show_curve);
    enter(&mut app, "Mode=MarkEnds");
    let marks = app.subcurve_draft_preview().unwrap();
    assert!(!marks.show_curve);
    assert!(Arc::ptr_eq(&shorten.geometry, &marks.geometry));
    enter(&mut app, "Copy=No");
    assert!(Arc::ptr_eq(
        &marks.geometry,
        &app.subcurve_draft_preview().unwrap().geometry
    ));
    enter(&mut app, "D");
    app.update_subcurve_hover(point(0.8, 1.2, 0.));
    assert!(app.subcurve_draft_preview().is_none());
    app.update_subcurve_hover(point(3.2, 4.8, 0.));
    assert!(app.subcurve_draft_preview().is_some());
    enter(&mut app, "FromMidpoint=Yes");
    let middle = app.subcurve_draft_preview().unwrap();
    assert!(
        middle.geometry.endpoints[0]
            .distance_to(point(0.8, 1.2, 0.))
            .unwrap()
            < 1e-6
    );
    assert!(
        middle.geometry.endpoints[1]
            .distance_to(point(3.2, 4.8, 0.))
            .unwrap()
            < 1e-6
    );
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.subcurve_draft_preview().is_none());
}
