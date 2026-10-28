use super::*;
use crate::app::tests::test_app;

#[path = "../../../crates/viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../../crates/viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn setup() -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,5", "Circle 0,0,0 2", "Point 9,8,7"] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    let ids = app.document.objects().map(|o| o.id()).collect();
    app.document.clear_history().unwrap();
    (app, ids)
}

fn start(app: &mut VibocerosApp, ids: &[ObjectId], explicit_station: bool) {
    enter(
        app,
        &format!(
            "Sweep1 RailId={}{}",
            ids[0],
            if explicit_station {
                " Parameters=0"
            } else {
                ""
            }
        ),
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert!(app.sweep_session.is_some(), "{:?}", app.command_log);
}

#[test]
fn refit_getters_create_qualified_surfaces_with_one_undo_step() {
    let observed: serde_json::Value = serde_json::from_slice(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tools/rhino_oracle/observations/sweep1_refit_sdk.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let native = observed["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "closed-refit-0.001")
        .unwrap();
    let native = nurbs_surface::from_definition(&native["value"][0]["definition"]);
    for (name, value, explicit_station) in [
        ("Refit", "0.001", true),
        ("RefitTolerance", "0.01", false),
        ("Simplify", "Refit", true),
    ] {
        let (mut app, ids) = setup();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        start(&mut app, &ids, explicit_station);
        enter(&mut app, name);
        assert_eq!(app.sweep_session.as_ref().unwrap().value, Some(name));
        enter(&mut app, value);
        assert!(app.sweep_session.as_ref().unwrap().value.is_none());
        assert_eq!(app.document.undo_label(), None);
        enter(&mut app, "");
        assert!(app.sweep_session.is_none(), "{:?}", app.command_log);
        let surfaces = app
            .document
            .objects()
            .filter_map(|o| match o.geometry() {
                Geometry::Brep(b) => Some(b.faces()[0].surface()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(surfaces.len(), 1);
        assert!(
            profile_rows::complete_bound(surfaces[0], &native, 1e-7).is_some(),
            "{name}: {:?}",
            app.command_log
        );
        for original in &before {
            assert_eq!(app.document.object(original.id()), Some(original));
        }
        assert_eq!(app.document.undo_label(), Some("Sweep1"));
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn invalid_refit_edits_and_mode_switches_preserve_local_and_model_history() {
    let (mut app, ids) = setup();
    app.commands
        .execute(&mut app.document, "Point 1,2,3")
        .unwrap();
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    let redo = app.document.redo_label().map(str::to_owned);
    start(&mut app, &ids, true);
    enter(&mut app, "Simplify");
    enter(&mut app, "Refit");
    enter(&mut app, "RefitTolerance");
    let state = format!("{:?}", app.sweep_session.as_ref().unwrap().state);
    let undo = app.sweep_session.as_ref().unwrap().undo.len();
    for invalid in ["-0.1", "NaN", "inf", "1e999", ""] {
        enter(&mut app, invalid);
        let session = app.sweep_session.as_ref().unwrap();
        assert_eq!(format!("{:?}", session.state), state);
        assert_eq!(session.undo.len(), undo);
        assert_eq!(session.value, Some("RefitTolerance"));
        assert_eq!(app.document.redo_label(), redo.as_deref());
        assert_eq!(app.document.undo_label(), None);
    }
    enter(&mut app, "0.001");
    enter(&mut app, "Simplify");
    enter(&mut app, "Rebuild");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .refit_tolerance,
        None
    );
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_point_count,
        5
    );
    enter(&mut app, "Undo");
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .refit_tolerance,
        Some(0.001)
    );
    assert_eq!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .state
            .options
            .rebuild_point_count,
        0
    );
    let state = format!("{:?}", app.sweep_session.as_ref().unwrap().state);
    let undo = app.sweep_session.as_ref().unwrap().undo.len();
    enter(&mut app, "Refit=0.01 Rebuild=7");
    assert_eq!(
        format!("{:?}", app.sweep_session.as_ref().unwrap().state),
        state
    );
    assert_eq!(app.sweep_session.as_ref().unwrap().undo.len(), undo);
    enter(&mut app, "RefitTolerance");
    assert_eq!(
        app.sweep_session.as_ref().unwrap().value,
        Some("RefitTolerance")
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(format!("{:?}", app.document), before);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
