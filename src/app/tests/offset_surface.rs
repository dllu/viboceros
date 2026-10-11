use super::boolean_intersection::{enter, pick};
use super::*;

#[test]
fn offset_srf_picks_polysurfaces_and_replays_native_result_history() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0 2,2 2");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    app.document.clear_selection();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "OffsetSrf 0.25 DeleteInput=Yes");
    assert!(!app.command_line_idle());
    pick(&mut app, id);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert!(app.document.object(id).is_none());
    assert_eq!(app.document.objects().count(), 1);
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("native offset")
    };
    assert!(brep.is_solid());
    assert!((brep.signed_volume(app.document.tolerance()).unwrap() - 15.625).abs() < 1e-8);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 1);
}
