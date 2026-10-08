use super::boolean_intersection::{enter, pick};
use super::*;
use viboceros_command::CommandContext;
use viboceros_geometry::{Brep, NurbsSurface};
#[test]
fn boolean_split_compound_target_drops_inactive_shell_and_preserves_original_on_undo() {
    let mut app = test_app();
    let frame = CommandContext::default().construction_plane;
    let original = Brep::try_disjoint_union(
        vec![
            Brep::try_box(frame, [[0., 2.]; 3], app.document.tolerance()).unwrap(),
            Brep::try_box(frame, [[4., 6.]; 3], app.document.tolerance()).unwrap(),
        ],
        app.document.tolerance(),
    )
    .unwrap();
    let target = app.document.add_geometry(Geometry::Brep(original)).unwrap();
    app.document
        .set_object_geometry_user_text([target], "Code", Some("original"))
        .unwrap();
    let surface = NurbsSurface::try_bilinear([
        point(1., -1., -1.),
        point(1., 3., -1.),
        point(1., 3., 3.),
        point(1., -1., 3.),
    ])
    .unwrap();
    let cutter = app
        .document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "BooleanSplit");
    pick(&mut app, target);
    enter(&mut app, "");
    pick(&mut app, cutter);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3);
    for o in app.document.objects().filter(|o| o.id() != cutter) {
        assert_eq!(o.geometry_user_text()["Code"], "original");
        let Geometry::Brep(b) = o.geometry() else {
            panic!()
        };
        assert!((b.signed_volume(app.document.tolerance()).unwrap() - 4.).abs() < 1e-10);
    }
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}
