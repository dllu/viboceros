use super::*;

#[test]
fn returning_to_options_reuses_geometry_and_commits_current_admission_flags() {
    let (mut app, id) = ready();
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetSrf");
    enter(&mut app, "0.75");
    enter(&mut app, "0.5");
    enter(&mut app, "1");
    assert_eq!(
        app.offset_surface_session
            .as_ref()
            .unwrap()
            .preview_preparations,
        3
    );
    enter(&mut app, "Solid=Yes");
    enter(&mut app, "DeleteInput=Yes");
    enter(&mut app, "Solid=No");
    enter(&mut app, "Solid=Yes");
    assert_eq!(
        app.offset_surface_session
            .as_ref()
            .unwrap()
            .preview_preparations,
        4
    );
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 1);
    let Geometry::Brep(wall) = app.document.object(id).unwrap().geometry() else {
        panic!("wall")
    };
    assert!(wall.is_solid());
    assert!((wall.signed_volume(app.document.tolerance()).unwrap() - 12.).abs() < 1e-8);
    let result = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), result);
}

#[test]
fn prior_previews_are_bounded_and_never_survive_changed_source_metadata() {
    let (mut app, id) = ready();
    enter(&mut app, "OffsetSrf");
    for distance in [0.6, 0.7, 0.8, 0.9, 1.] {
        enter(&mut app, &distance.to_string());
        assert!(
            app.offset_surface_session
                .as_ref()
                .unwrap()
                .cached_previews()
                <= 2
        );
    }
    app.document
        .set_object_geometry_user_text([id], "changed", Some("yes"))
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "0.9");
    assert!(app.command_line_idle());
    assert!(app.offset_surface_session.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}
