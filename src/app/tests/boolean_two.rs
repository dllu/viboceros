use super::boolean_intersection::{enter, pick};
use super::*;
use serde_json::Value;
use viboceros_command::boolean_two::Mode;
#[test]
fn boolean_two_cycles_native_results_without_source_edits_or_preview_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/boolean_two_command.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let mut app = test_app();
        for bounds in v["bounds"].as_array().unwrap() {
            let bounds = serde_json::from_value::<[[f64; 2]; 3]>(bounds.clone()).unwrap();
            app.document
                .add_geometry(Geometry::Brep(
                    viboceros_geometry::Brep::try_box(
                        viboceros_command::CommandContext::default().construction_plane,
                        bounds,
                        app.document.tolerance(),
                    )
                    .unwrap(),
                ))
                .unwrap();
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(
            &mut app,
            &format!(
                "Boolean2Objects DeleteInput={}",
                if v["delete"] == true { "Yes" } else { "No" }
            ),
        );
        assert!(!app.command_line_idle());
        pick(&mut app, ids[0]);
        pick(&mut app, ids[1]);
        enter(&mut app, "");
        if v["command"]["success"] == false && v["cancel"] != true {
            assert!(app.boolean_two_prompt.is_none());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
            continue;
        }
        assert_eq!(app.boolean_two_prompt.as_ref().unwrap().mode, Mode::Union);
        for step in 0..v["cycles"].as_u64().unwrap() {
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
            let out = ViewportOutput {
                source_viewport_click: true,
                ..Default::default()
            };
            app.handle_viewport_action(out);
            assert_eq!(
                app.boolean_two_prompt.as_ref().unwrap().mode,
                Mode::ALL[(step as usize + 1) % 5]
            );
        }
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        if v["cancel"] == true {
            enter(&mut app, "Cancel");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            continue;
        }
        enter(&mut app, "");
        assert!(app.boolean_two_prompt.is_none(), "{:?}", app.command_log);
        let expected = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), expected.len());
        for (o, native) in app.document.objects().zip(expected) {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            assert!(
                (b.signed_volume(app.document.tolerance()).unwrap()
                    - native["volume"].as_f64().unwrap())
                .abs()
                    < 1e-9
            );
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), expected.len());
    }
}
#[test]
fn boolean_two_stale_geometry_is_rejected_and_scene_cache_survives_cycle_wrap() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0 2,2 2");
    enter(&mut app, "Box 1,1,1 3,3,1 2");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document
        .select_command_results(ids.iter().copied())
        .unwrap();
    enter(&mut app, "Boolean2Objects");
    enter(&mut app, "SetDisplayMode Viewport=All Mode=Ghosted");
    assert!(app.boolean_two_prompt.is_some());
    assert!(
        app.viewports
            .iter()
            .all(|v| v.display_mode == DisplayMode::Ghosted)
    );
    let snapshot = app
        .boolean_two_prompt
        .as_ref()
        .unwrap()
        .scene()
        .unwrap()
        .objects()
        .next()
        .unwrap()
        .geometry_snapshot()
        .clone();
    for _ in 0..5 {
        app.cycle_boolean_two();
    }
    assert!(
        snapshot.shares_storage_with(
            app.boolean_two_prompt
                .as_ref()
                .unwrap()
                .scene()
                .unwrap()
                .objects()
                .next()
                .unwrap()
                .geometry_snapshot()
        )
    );
    app.document
        .replace_object_geometries([(ids[1], Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    assert!(app.cycle_boolean_two());
    assert!(app.boolean_two_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
}
