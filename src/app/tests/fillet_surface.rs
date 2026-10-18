use super::boolean_intersection::enter;
use super::*;
fn pair() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 10,0,0 10,10,0 0,10,0");
    enter(&mut app, "SrfPt 5,0,-5 5,0,5 5,10,5 5,10,-5");
    let ids = app
        .document
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    app.document.clear_history().unwrap();
    (app, ids)
}
#[test]
fn surface_picks_and_options_cancel_without_edits() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "FilletSrf");
    assert!(!app.command_line_idle());
    assert!(app.viewport_object_filter().is_none());
    assert!(app.picking_fillet_surface());
    assert!(!app.component_preselection_available());
    enter(&mut app, "Radius 0.75");
    app.accept_component_face_hit(ids[0], 0, Some(Point3::try_new(2., 5., 0.).unwrap()));
    assert_eq!(
        app.fillet_surface_prompt.as_ref().unwrap().excluded(),
        Some(ids[0])
    );
    app.accept_component_face_hit(ids[0], 0, Some(Point3::try_new(2., 5., 0.).unwrap())); // same parent does not advance
    assert!(app.picking_fillet_surface());
    app.accept_component_face_hit(ids[1], 0, None);
    assert!(app.picking_fillet_surface());
    enter(&mut app, &format!("{} 0.2,0.5", ids[1]));
    assert!(!app.picking_fillet_surface());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.fillet_surface_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn preview_recomputes_and_accepts_as_one_undoable_edit() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "FilletSrf Trim=No");
    for id in ids {
        enter(&mut app, &format!("{id} 0.2 0.5"));
    }
    assert_eq!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        3
    );
    enter(&mut app, "Trim=Split");
    assert_eq!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        5
    );
    enter(&mut app, "Trim=Yes Join=Yes");
    assert_eq!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        1
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.fillet_surface_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 1);
}
#[test]
fn stale_source_and_new_command_cancel_the_prompt() {
    let (mut app, ids) = pair();
    enter(&mut app, "FilletSrf");
    enter(&mut app, &format!("{} 0.2,0.5", ids[0]));
    app.document.delete_object(ids[0]).unwrap();
    app.validate_fillet_surface();
    assert!(app.fillet_surface_prompt.is_none());
    enter(&mut app, "FilletSrf");
    enter(&mut app, "Point 1,2,3");
    assert!(app.fillet_surface_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
}

#[cfg(feature = "native-smlib")]
#[test]
fn failed_recompute_clears_preview_and_corrected_options_recover() {
    let (mut app, ids) = pair();
    enter(&mut app, "FilletSrf Extend=No");
    for id in ids {
        enter(&mut app, &format!("{id} 0.2,0.5"));
    }
    assert!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
    enter(&mut app, "Radius 100");
    assert!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_none()
    );
    enter(&mut app, "");
    assert!(app.fillet_surface_prompt.is_some());
    assert!(!app.document.can_undo());
    enter(&mut app, "Radius 1");
    assert!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .is_some()
    );
    enter(&mut app, "Join Yes");
    assert_eq!(
        app.fillet_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        1
    );
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 1);
}
