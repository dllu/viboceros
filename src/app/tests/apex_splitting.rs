use super::apex_splitting_comparison as topology;
use super::*;
use viboceros_oracle::apex_splitting::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn apex_splitting_picking_and_mode_edits_match_native_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in ["apex_splitting_pilot", "apex_splitting_holdouts"] {
        let query: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = super::oracle_observation::read(&base, family);
        for op in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "apex_splitting")
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
            compare(&app, "before");
            enter(
                &mut app,
                if f.crease_splitting {
                    "CreaseSplitting Enable"
                } else {
                    "CreaseSplitting Disable"
                },
            );
            enter(&mut app, "ExtrudeCrvToPoint Output=Surface");
            enter(&mut app, if f.solid { "Solid=Yes" } else { "Solid=No" });
            if f.seed_subd {
                enter(&mut app, "SplitAtTangents=Yes");
                enter(&mut app, "Output=SubD");
                enter(&mut app, "Output=Surface");
            } else if f.set_split {
                enter(
                    &mut app,
                    if f.split_at_tangents {
                        "SplitAtTangents=Yes"
                    } else {
                        "SplitAtTangents=No"
                    },
                );
            }
            enter(
                &mut app,
                &format!("DeleteInput={}", if f.delete_input { "Yes" } else { "No" }),
            );
            assert!(app.active_command.is_some());
            let p = f.apex;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            assert!(app.active_command.is_none());
            compare(&app, "after");
            assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
            enter(&mut app, "Undo");
            compare(&app, "undo");
            enter(&mut app, "Redo");
            compare(&app, "redo");
        }
    }
    assert_eq!(count, 71);
}

#[test]
fn apex_splitting_invalid_edits_and_cancel_retain_options_model_and_redo() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 3,0,0",
        "SelAll",
        "ExtrudeCrvToPoint 0,2,3",
        "Undo",
    ] {
        enter(&mut app, input);
    }
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "ExtrudeCrvToPoint SplitAtTangents=No");
    let pending = app.active_command;
    for input in [
        "SplitAtTangents=Maybe",
        "SplitAtTangents=Yes SplitAtTangents=No",
        "SplitAtTangents=Yes Output=Mesh",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, pending, "{input}");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        assert_eq!(app.document.redo_label(), Some("ExtrudeCrvToPoint"));
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.redo_label(), Some("ExtrudeCrvToPoint"));
}
