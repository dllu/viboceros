use super::extrusion_splitting_comparison as topology;
use super::*;
use viboceros_oracle::extrusion_splitting::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn extrusion_splitting_direction_getters_and_mode_edits_match_native_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "extrusion_splitting_pilot",
        "extrusion_splitting_holdouts",
        "extrusion_splitting_confirmations",
        "extrusion_splitting_cancellations",
        "extrusion_splitting_curved",
        "extrusion_splitting_mixed",
    ] {
        let query: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = super::oracle_observation::read(&base, family);
        for op in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "extrusion_splitting")
        {
            count += 1;
            let row = native["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == op["id"])
                .unwrap();
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let mut app = test_app();
            let (doc, source) = setup(&f, app.document.tolerance()).unwrap();
            app.document = doc;
            let compare = |app: &VibocerosApp, phase: &str| {
                let actual = snapshot(&app.document, source).unwrap();
                topology::compare(
                    &actual,
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                    1e-12,
                );
            };
            enter(
                &mut app,
                if f.crease_splitting {
                    "CreaseSplitting Enable"
                } else {
                    "CreaseSplitting Disable"
                },
            );
            if let Some(seed) = f.seed_split {
                enter(&mut app, "UseExtrusions Polysurface");
                if f.seed_cancel {
                    enter(
                        &mut app,
                        if seed {
                            "ExtrudeCrv SplitAtTangents=Yes"
                        } else {
                            "ExtrudeCrv SplitAtTangents=No"
                        },
                    );
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(
                        &mut app,
                        &format!(
                            "ExtrudeCrv 0,0,0 0,3,0 SplitAtTangents={}",
                            if seed { "Yes" } else { "No" }
                        ),
                    );
                    enter(&mut app, "Undo");
                }
                app.document.clear_history().unwrap();
            }
            enter(
                &mut app,
                if f.use_extrusions {
                    "UseExtrusions Extrusion"
                } else {
                    "UseExtrusions Polysurface"
                },
            );
            compare(&app, "before");
            enter(&mut app, "ExtrudeCrv Output=Surface");
            enter(&mut app, if f.solid { "Solid=Yes" } else { "Solid=No" });
            app.accept_drafting_point(point(0., 0., 0.));
            if f.seed_subd {
                if !f.use_extrusions {
                    enter(&mut app, "SplitAtTangents=Yes");
                }
                enter(&mut app, "Output=SubD");
                enter(&mut app, "Output=Surface");
            } else if let Some(split) = f.split_at_tangents {
                enter(
                    &mut app,
                    if split {
                        "SplitAtTangents=Yes"
                    } else {
                        "SplitAtTangents=No"
                    },
                );
            }
            enter(
                &mut app,
                &format!(
                    "BothSides={} DeleteInput={}",
                    if f.both_sides { "Yes" } else { "No" },
                    if f.delete_input { "Yes" } else { "No" }
                ),
            );
            let Some(InteractiveCommand::ExtrudeCurve { base: Some(_), .. }) = app.active_command
            else {
                panic!("direction base lost")
            };
            let d = f.direction;
            app.accept_drafting_point(point(d[0], d[1], d[2]));
            assert!(app.active_command.is_none(), "{}", op["id"]);
            let expected_split = if f.seed_subd {
                false
            } else {
                f.split_at_tangents.unwrap_or(if f.seed_cancel {
                    true
                } else {
                    f.seed_split.unwrap_or(true)
                })
            };
            assert_eq!(
                app.commands.curve_extrusion_split_at_tangents(),
                expected_split,
                "{}",
                op["id"]
            );
            compare(&app, "after");
            assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
            enter(&mut app, "Undo");
            compare(&app, "undo");
            enter(&mut app, "Redo");
            compare(&app, "redo");
        }
    }
    assert_eq!(count, 163);
}

#[test]
fn extrusion_splitting_cancel_invalid_edits_and_failed_direction_keep_preferences_and_redo() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 3,0,0",
        "SelAll",
        "Point 30,20,10",
        "Undo",
        "UseExtrusions Polysurface",
        "ExtrudeCrv SplitAtTangents=No",
    ] {
        enter(&mut app, input);
    }
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    app.accept_drafting_point(point(0., 0., 0.));
    let pending = app.active_command;
    for input in [
        "SplitAtTangents=Maybe",
        "SplitAtTangents=Yes SplitAtTangents=No",
        "SplitAtTangents=Yes Output=Mesh",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, pending);
        assert!(app.commands.curve_extrusion_split_at_tangents());
    }
    app.accept_drafting_point(point(3., 0., 0.));
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert!(app.commands.curve_extrusion_split_at_tangents());
    app.cancel_current_prompt_or_selection();
    assert!(app.commands.curve_extrusion_split_at_tangents());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Undo");
    enter(&mut app, "ExtrudeCrv SplitAtTangents=No");
    app.accept_drafting_point(point(0., 0., 0.));
    app.accept_drafting_point(point(0., 3., 0.));
    assert!(app.active_command.is_none());
    assert!(!app.commands.curve_extrusion_split_at_tangents());
    enter(&mut app, "Undo");
    assert!(!app.commands.curve_extrusion_split_at_tangents());
    enter(&mut app, "UseExtrusions Extrusion");
    enter(&mut app, "ExtrudeCrv");
    let pending = app.active_command;
    enter(&mut app, "SplitAtTangents=Yes");
    assert_eq!(app.active_command, pending);
    app.cancel_current_prompt_or_selection();
    assert!(!app.commands.curve_extrusion_split_at_tangents());
}
