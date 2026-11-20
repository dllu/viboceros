use super::*;
use crate::app::tests::test_app;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    for command in ["Line 0,0,0 0,0,5", "Line 0,0,4 3,0,4", "Line 0,0,1 2,0,1"] {
        app.commands.execute(&mut app.document, command).unwrap();
    }
    let ids = app.document.objects().map(|o| o.id()).collect();
    (app, ids)
}

#[test]
fn sweep_picker_accepts_ordered_sections_and_restores_sources_with_one_undo() {
    for partial in [false, true] {
        let (mut app, ids) = setup();
        if partial {
            app.document
                .select_objects_direct([ids[0]], SelectionMode::Replace)
                .unwrap();
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Sweep1");
        assert!(app.object_prompt.is_some());
        app.select_prompt_objects(ids.clone(), SelectionMode::Replace);
        enter(&mut app, "");
        assert!(app.command_hint().starts_with("Review the sweep"));
        assert_eq!(app.document.objects().count(), 3);
        enter(&mut app, "");
        assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().count(), 4);
        assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), ids);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_picker_cancel_restores_partial_preselection_without_model_changes() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Sweep1 RefitRail=Yes");
    app.select_prompt_objects([ids[1]], SelectionMode::Add);
    app.cancel_interactive_command(false);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
}

#[test]
fn sweep_picker_retains_inline_rail_and_frame_options() {
    let (mut app, ids) = setup();
    enter(
        &mut app,
        &format!(
            "Sweep1 RailId={} RefitRail=Yes FrameStyle=Roadlike Axis=0,1,0",
            ids[0]
        ),
    );
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([ids[1], ids[2]], SelectionMode::Replace);
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 4);
}

#[test]
fn sweep_open_preselection_reviews_and_dashed_command_runs_directly() {
    let (mut app, ids) = setup();
    app.restore_curve_selection(ids.clone());
    app.document.clear_history().unwrap();
    enter(&mut app, "_Sweep1");
    assert!(app.command_hint().starts_with("Review the sweep"));
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "_-Sweep1");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
}

#[test]
fn sweep_command_file_accepts_an_open_profile_with_one_enter() {
    let (mut app, ids) = setup();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sweep.txt");
    std::fs::write(&path, format!("Sweep1 {} {} _Enter\n", ids[0], ids[1])).unwrap();
    enter(
        &mut app,
        &format!("-ReadCommandFile \"{}\"", path.display()),
    );
    for _ in 0..16 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(
        app.command_file.last_result(),
        Some(true),
        "{:?}",
        app.command_log
    );
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().count(), 4);
}
