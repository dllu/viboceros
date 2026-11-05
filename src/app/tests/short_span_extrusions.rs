use super::*;
use viboceros_oracle::short_span_extrusions::{Fixture, setup, snapshot};
#[path = "../../../crates/viboceros-oracle/tests/support/short_span_extrusions.rs"]
mod comparison;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn short_span_extrusions_match_native_through_direction_point_getters() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/short_span_extrusions.json")).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, "short_span_extrusions");
    let operations = request["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 16);
    for (op, row) in operations.iter().zip(native["results"].as_array().unwrap()) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let (doc, source) = setup(&f, app.document.tolerance()).unwrap();
        app.document = doc;
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let compare = |app: &VibocerosApp, phase: &str| {
            let actual = snapshot(&app.document, source).unwrap();
            super::edge_analysis_mixed_support::compare(
                &actual,
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            );
            comparison::exact_basis(
                &actual,
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            );
        };
        compare(&app, "before");
        enter(
            &mut app,
            &format!(
                "ExtrudeCrv Solid=No DeleteInput=No BothSides={}",
                if f.both_sides { "Yes" } else { "No" }
            ),
        );
        assert!(app.active_command.is_some());
        app.accept_drafting_point(point(0., 0., 0.));
        let end = if op["id"].as_str().unwrap().starts_with("xz_") {
            point(0., f.height, 0.)
        } else {
            point(0., 0., f.height)
        };
        app.accept_drafting_point(end);
        assert!(app.active_command.is_none());
        compare(&app, "after");
        assert_eq!(app.document.object(source).unwrap(), &before[0]);
        assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
        enter(&mut app, "Undo");
        compare(&app, "undo");
        enter(&mut app, "Redo");
        compare(&app, "redo");
        let Geometry::Brep(brep) = app
            .document
            .objects()
            .find(|o| o.id() != source)
            .unwrap()
            .geometry()
        else {
            panic!("typed extrusion required")
        };
        assert!(
            brep.tessellate_with_normals(8, app.document.tolerance())
                .is_ok()
        );
    }
}

#[test]
fn curve_extrusion_options_control_caps_deletion_and_both_sides_at_either_getter() {
    for inline in [false, true] {
        let mut app = test_app();
        enter(&mut app, "Circle 0,0,0 2");
        enter(&mut app, "SelAll");
        let source = app.document.objects().next().unwrap().id();
        app.document.clear_history().unwrap();
        enter(
            &mut app,
            if inline {
                "ExtrudeCrv Solid=Yes Output=Surface BothSides=Yes DeleteInput=Yes"
            } else {
                "ExtrudeCrv Output=Surface"
            },
        );
        if !inline {
            enter(&mut app, "_Solid=_Yes");
        }
        app.accept_drafting_point(point(0., 0., 0.));
        if !inline {
            let before = app.active_command;
            enter(&mut app, "BothSides=Yes Solid=Maybe");
            assert_eq!(app.active_command, before);
            enter(&mut app, "BothSides=Yes DeleteInput=Yes");
        }
        app.accept_drafting_point(point(0., 0., 3.));
        assert!(app.active_command.is_none());
        assert!(app.document.object(source).is_none());
        let Geometry::Brep(output) = app.document.objects().next().unwrap().geometry() else {
            panic!("closed extrusion required")
        };
        assert!(output.is_solid());
        assert_eq!(
            output.extrusion().unwrap().nurbs_form().unwrap().caps,
            [true, true]
        );
        assert_eq!(output.bounds().min().z(), -3.);
        assert_eq!(output.bounds().max().z(), 3.);
        enter(&mut app, "Undo");
        assert!(app.document.object(source).is_some());
        enter(&mut app, "Redo");
        assert!(app.document.object(source).is_none());
    }
}

#[test]
fn curve_extrusion_option_edits_cancel_without_discarding_redo_or_selection() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 100,200,300");
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "ExtrudeCrv");
    enter(&mut app, "Solid=Yes BothSides=Yes");
    enter(&mut app, "Output=Mesh");
    assert!(app.active_command.is_some());
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.redo_label(), Some("Point"));
}
