use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    let surface = viboceros_geometry::NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 2.]
            .into_iter()
            .flat_map(|y| {
                [(0., 0.), (1., 0.), (1., 1.)]
                    .into_iter()
                    .map(move |(x, z)| point(x, y, z))
            })
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    app.document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap()
}
#[test]
fn crease_division_picker_filters_objects_edits_options_and_cancels() {
    let mut app = test_app();
    let id = source(&mut app);
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(5., 5., 5.)))
        .unwrap();
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "DivideAlongCreases");
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::SurfaceComponents)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(peer),
        mode: SelectionMode::Replace,
    });
    assert!(!app.document.is_selected(peer));
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "SplitAtTangents=No");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.object(id), Some(&before));
    enter(&mut app, "Bad=Yes");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(id), Some(&before));
}
#[cfg(feature = "native-smlib")]
#[test]
fn crease_division_preselection_and_clicked_input_make_one_undoable_polysurface() {
    for pre in [false, true] {
        let mut app = test_app();
        let id = source(&mut app);
        let before = app.document.object(id).unwrap().clone();
        if pre {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "DivideAlongCreases");
        if !pre {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none());
        let after = app.document.object(id).unwrap().clone();
        assert!(matches!(after.geometry(),Geometry::Brep(b) if b.faces().len()==2));
        assert_eq!(app.document.undo_label(), Some("DivideAlongCreases"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id), Some(&before));
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id), Some(&after));
    }
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn crease_division_without_native_keeps_the_getter_and_source_available() {
    let mut app = test_app();
    let id = source(&mut app);
    enter(&mut app, "DivideAlongCreases");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
