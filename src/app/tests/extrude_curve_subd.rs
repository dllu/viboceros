use super::subd_face_comparison as topology;
use super::*;
use viboceros_oracle::extrude_curve_subd::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn extrude_curve_subd_direction_picking_and_output_edits_match_native_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "extrude_curve_subd_pilot",
        "extrude_curve_subd_holdouts",
        "extrude_curve_subd_confirmations",
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
            .filter(|op| op["op"] == "extrude_curve_subd")
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
                if count % 2 == 0 {
                    "UseExtrusions Polysurfaces"
                } else {
                    "UseExtrusions Extrusions"
                },
            );
            enter(&mut app, "ExtrudeCrv Output=Surface");
            app.accept_drafting_point(point(0., 0., 0.));
            enter(
                &mut app,
                &format!(
                    "Output=SubD Solid={} BothSides={} DeleteInput={}",
                    if f.solid { "Yes" } else { "No" },
                    if f.both_sides { "Yes" } else { "No" },
                    if f.delete_input { "Yes" } else { "No" }
                ),
            );
            assert!(app.active_command.is_some());
            let p = f.direction;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            assert!(app.active_command.is_none());
            compare(&app, "after");
            assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
            enter(&mut app, "Undo");
            compare(&app, "undo");
            enter(&mut app, "Redo");
            compare(&app, "redo");
        }
    }
    assert_eq!(count, 62);
}

#[test]
fn extrude_curve_subd_option_edits_and_cancel_preserve_model_selection_and_redo() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 3,0,0",
        "SelAll",
        "ExtrudeCrv 2 Output=SubD",
        "Undo",
    ] {
        enter(&mut app, input);
    }
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "ExtrudeCrv Output=SubD BothSides=Yes");
    app.accept_drafting_point(point(0., 0., 0.));
    let pending = app.active_command;
    for input in [
        "Output=Mesh",
        "Output=Surface Output=SubD",
        "Solid=Maybe",
        "BothSides=No BothSides=Yes",
        "Output=Surface Solid=Maybe",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, pending, "{input}");
    }
    enter(&mut app, "Output=Surface Solid=Yes");
    enter(&mut app, "_Output=_SubD");
    let Some(InteractiveCommand::ExtrudeCurve {
        options,
        base: Some(_),
        ..
    }) = app.active_command
    else {
        panic!("extrusion getter required")
    };
    assert_eq!(
        options.output,
        viboceros_command::CurveExtrusionOutput::SubD
    );
    assert!(!options.solid);
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.redo_label(), Some("ExtrudeCrv"));
    enter(&mut app, "Redo");
    assert!(matches!(
        app.document.objects().nth(1).unwrap().geometry(),
        Geometry::SubD(_)
    ));
}
