use super::*;
use viboceros_oracle::revolve_subd::{Fixture, setup, snapshot};
#[path = "../../../crates/viboceros-oracle/tests/support/revolve_subd.rs"]
mod topology;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn revolve_subd_axis_picking_and_output_edits_match_native_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "revolve_subd_pilot",
        "revolve_subd_holdouts",
        "revolve_subd_edges",
        "revolve_subd_policy",
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
            .filter(|op| op["op"] == "revolve_subd")
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
            enter(
                &mut app,
                if f.crease_splitting {
                    "CreaseSplitting Enable"
                } else {
                    "CreaseSplitting Disable"
                },
            );
            let compare = |app: &VibocerosApp, phase: &str| {
                let actual = snapshot(&app.document, source).unwrap();
                topology::compare(
                    &actual,
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                    topology::epsilon(op["id"].as_str().unwrap()),
                );
            };
            compare(&app, "before");
            enter(
                &mut app,
                &format!(
                    "Revolve Output=Surface Angle={} StartAngle={} DeleteInput={} {}",
                    f.sweep_degrees,
                    f.start_degrees,
                    if f.delete_input { "Yes" } else { "No" },
                    if f.seed_surface {
                        "Deformable=Yes PointCount=16 SplitAtTangents=Yes"
                    } else {
                        ""
                    }
                ),
            );
            let p = f.origin;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            enter(
                &mut app,
                &format!("Output=SubD SegmentCount={}", f.segment_count),
            );
            assert!(app.active_command.is_some());
            let p = f.axis_end;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            assert!(app.active_command.is_none());
            compare(&app, "after");
            assert_eq!(app.document.undo_label(), Some("Revolve"));
            enter(&mut app, "Undo");
            compare(&app, "undo");
            enter(&mut app, "Redo");
            compare(&app, "redo");
        }
    }
    assert_eq!(count, 80);
}

#[test]
fn revolve_subd_option_groups_and_cancel_preserve_model_selection_and_redo() {
    let mut app = test_app();
    for input in [
        "Line 3,0,0 3,0,5",
        "SelAll",
        "Revolve 0,0,0 0,0,1 120 Output=SubD SegmentCount=6",
        "Undo",
    ] {
        enter(&mut app, input);
    }
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "Revolve Output=SubD SegmentCount=8");
    app.accept_drafting_point(point(0., 0., 0.));
    let pending = app.active_command;
    for input in [
        "SegmentCount=100001",
        "SegmentCount=-1",
        "SegmentCount=4.5",
        "SegmentCount=NaN",
        "SegmentCount=4 SegmentCount=8",
        "Output=Surface Output=SubD",
        "Output=Mesh",
        "SegmentCount=7 PointCount=0",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, pending, "{input}");
    }
    enter(&mut app, "Output=Surface Deformable=Yes PointCount=16");
    enter(&mut app, "Output=SubD SegmentCount=0");
    let Some(InteractiveCommand::Revolve { options, .. }) = app.active_command else {
        panic!("Revolve getter required")
    };
    assert_eq!(options.segment_count, 4);
    assert_eq!(options.output, viboceros_command::revolve::Output::SubD);
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.redo_label(), Some("Revolve"));
    enter(&mut app, "Redo");
    assert!(matches!(
        app.document.objects().nth(1).unwrap().geometry(),
        Geometry::SubD(_)
    ));
}
