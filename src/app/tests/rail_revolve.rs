use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn sources(app: &mut VibocerosApp) -> [ObjectId; 2] {
    enter(app, "Line 2,0,2 1,0,5");
    let profile = app.document.objects().last().unwrap().id();
    enter(app, "Circle 0,0,2 2");
    let rail = app.document.objects().last().unwrap().id();
    app.document
        .set_object_names([(rail, Some("Rail".into()))])
        .unwrap();
    [profile, rail]
}
#[test]
fn rail_revolve_picker_keeps_a_preselected_profile_filters_geometry_and_cancels() {
    let mut app = test_app();
    let ids = sources(&mut app);
    let other = app
        .document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "RailRevolve");
    assert!(app.object_prompt.is_some());
    assert!(app.document.is_selected(ids[0]));
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(other),
        mode: SelectionMode::Add,
    });
    assert!(!app.document.is_selected(other));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "ScaleHeight=Yes");
    enter(&mut app, "");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::RailRevolve(state)) if state.options.scale_height)
    );
    enter(&mut app, "0,0,0");
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn rail_revolve_preselection_and_postselection_axes_commit_one_undoable_surface() {
    for pre in [false, true] {
        let mut app = test_app();
        let ids = sources(&mut app);
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if pre {
            app.document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "RailRevolve");
        if !pre {
            for id in ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        assert!(
            matches!(app.active_command, Some(InteractiveCommand::RailRevolve(_))),
            "{:?}",
            app.command_log
        );
        enter(&mut app, "ScaleHeight Yes SplitAtTangents No");
        enter(&mut app, "0,0,0");
        enter(&mut app, "0,0,0");
        assert!(app.active_command.is_some());
        enter(&mut app, "");
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().count(), 3, "{:?}", app.command_log);
        let output = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .clone();
        assert!(matches!(output.geometry(), Geometry::NurbsSurface(_)));
        assert_eq!(app.document.undo_label(), Some("RailRevolve"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(output.id()), Some(&output));
    }
}
#[cfg(feature = "native-smlib")]
#[test]
fn rail_revolve_typed_axes_and_named_rail_survive_postselection_option_edits() {
    let mut app = test_app();
    let ids = sources(&mut app);
    enter(
        &mut app,
        "RailRevolve 0,0,0 0,0,1 RailName=Rail ScaleHeight=No",
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "ScaleHeight=Yes SplitAtTangents=No");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 3, "{:?}", app.command_log);
    assert!(
        app.document
            .objects()
            .any(|o| matches!(o.geometry(), Geometry::NurbsSurface(_)))
    );
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn rail_revolve_without_native_keeps_model_after_finishing_axis_input() {
    let mut app = test_app();
    let ids = sources(&mut app);
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "RailRevolve");
    enter(&mut app, "0,0,0");
    enter(&mut app, "");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.command_log.iter().any(|s| s.contains("native-smlib")));
}
