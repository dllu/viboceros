use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn sources(app: &mut VibocerosApp) -> Vec<ObjectId> {
    for input in [
        "Line 0,0,0 0,0,5",
        "Line 2,0,5 2,0,0",
        "Line 2,0,5 0,0,5",
        "Line 0,0,0 2,0,0",
    ] {
        enter(app, input);
    }
    app.document.objects().map(|o| o.id()).collect()
}
#[test]
fn sweep_two_rail_picker_filters_and_cancels_without_model_edits() {
    let mut app = test_app();
    let ids = sources(&mut app);
    let other = app
        .document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Sweep2");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(other),
        mode: SelectionMode::Replace,
    });
    assert!(!app.document.is_selected(other));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn sweep_two_rail_preselection_and_clicked_input_commit_one_undoable_result() {
    for pre in [false, true] {
        let mut app = test_app();
        let ids = sources(&mut app);
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if pre {
            app.document
                .select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "Sweep2");
        if !pre {
            for id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(*id),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().count(), 5);
        assert_eq!(app.document.undo_label(), Some("Sweep2"));
        let result = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .clone();
        assert!(matches!(result.geometry(),Geometry::Brep(b) if b.faces().len()==1));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(result.id()), Some(&result));
    }
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn sweep_two_rail_unavailable_native_backend_keeps_getter_and_sources() {
    let mut app = test_app();
    let ids = sources(&mut app);
    enter(&mut app, "Sweep2");
    for id in &ids {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Add,
        });
    }
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
}
