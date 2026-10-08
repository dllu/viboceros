use super::boolean_intersection::{enter, fixture, pick};
use super::*;
use serde_json::Value;
use viboceros_document::SelectionMode;

#[test]
fn boolean_split_picking_keeps_cutters_and_replays_pre_and_post_selected_history() {
    for pre in [false, true] {
        for keep in [false, true] {
            let (mut app, ids) = fixture();
            app.document.clear_history().unwrap();
            if pre {
                app.document
                    .select_objects_direct([ids[0]], SelectionMode::Replace)
                    .unwrap();
            }
            enter(
                &mut app,
                &format!(
                    "BooleanSplit DeleteInput={}",
                    if keep { "No" } else { "Yes" }
                ),
            );
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::SurfaceComponents)
            );
            if !pre {
                pick(&mut app, ids[2]);
                assert!(!app.document.is_selected(ids[2]));
                pick(&mut app, ids[0]);
                enter(&mut app, "");
            }
            assert_eq!(app.document.selected_object_count(), 0);
            enter(&mut app, "");
            assert!(app.intersection_prompt.is_some());
            assert!(!app.document.can_undo());
            pick(&mut app, ids[1]);
            enter(&mut app, "");
            assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
            assert!(app.document.object(ids[1]).is_some());
            assert!(!app.document.is_selected(ids[1]));
            assert_eq!(app.document.object(ids[0]).is_some(), keep);
            let outputs = app
                .document
                .objects()
                .filter(|o| !ids.contains(&o.id()))
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), 2);
            let mut volumes = outputs
                .iter()
                .map(|o| {
                    let Geometry::Brep(b) = o.geometry() else {
                        panic!()
                    };
                    assert_eq!(o.geometry_user_text()["Code"], "geometry-0");
                    assert_eq!(app.document.is_selected(o.id()), pre);
                    b.signed_volume(app.document.tolerance()).unwrap()
                })
                .collect::<Vec<_>>();
            volumes.sort_by(f64::total_cmp);
            assert!((volumes[0] - 1.).abs() < 1e-10 && (volumes[1] - 7.).abs() < 1e-10);
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 3);
            assert_eq!(app.document.selected_object_count(), 0);
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().len(), if keep { 5 } else { 4 });
            assert!(!app.document.is_selected(ids[1]));
            assert_eq!(
                app.document.selected_object_count(),
                if pre { 2 } else { 0 }
            );
        }
    }
}

#[test]
fn boolean_split_shared_sets_and_cancellation_keep_the_document_atomic() {
    let (mut app, ids) = fixture();
    app.document.clear_history().unwrap();
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BooleanSplit");
    pick(&mut app, ids[0]);
    enter(&mut app, "");
    pick(&mut app, ids[1]);
    enter(&mut app, "Cancel");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "BooleanSplit");
    pick(&mut app, ids[0]);
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    pick(&mut app, ids[0]);
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert!(app.document.object(ids[0]).is_none() && app.document.object(ids[1]).is_none());
}

#[test]
fn boolean_split_replays_native_cancellation_selection_without_geometry_or_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/boolean_split_command.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        if !case.contains("cancel") {
            continue;
        }
        let (mut app, ids) = fixture();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "BooleanSplit");
        if case == "slab_cancel_options" {
            enter(&mut app, "DeleteInput=No");
        } else {
            pick(&mut app, ids[0]);
            enter(&mut app, "");
            if case == "slab_cancel_cutters_options" {
                enter(&mut app, "DeleteInput=No");
            }
            pick(&mut app, ids[1]);
        }
        enter(&mut app, "Cancel");
        assert!(app.intersection_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            app.document.selected_object_count(),
            v["command"]["after"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count()
        );
        assert!(!app.document.can_undo());
        if let Some(followup) = v.get("followup") {
            enter(&mut app, "BooleanSplit");
            let retained = followup["after"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["source"] == 0);
            assert_eq!(
                app.intersection_prompt
                    .as_ref()
                    .unwrap()
                    .boolean
                    .as_ref()
                    .unwrap()
                    .delete_input,
                !retained
            );
        }
    }
}
