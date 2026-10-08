use super::boolean_intersection::{enter, pick};
use super::*;
use viboceros_command::CommandContext;
use viboceros_geometry::{Brep, NurbsSurface};

fn fixture(reverse: bool) -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    let sheet = Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            point(1., -1., -1.),
            point(1., 3., -1.),
            point(1., 3., 3.),
            point(1., -1., 3.),
        ])
        .unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    let target = app
        .document
        .add_geometry(Geometry::Brep(if reverse {
            sheet.reversed()
        } else {
            sheet
        }))
        .unwrap();
    app.document
        .set_object_geometry_user_text([target], "Code", Some("original"))
        .unwrap();
    let cutter = app
        .document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                CommandContext::default().construction_plane,
                [[0., 2.]; 3],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    (app, [target, cutter])
}
#[test]
fn boolean_split_open_picker_produces_shared_solid_and_open_boundary_with_history() {
    for reverse in [false, true] {
        let (mut app, ids) = fixture(reverse);
        let cutter = app
            .document
            .object(ids[1])
            .unwrap()
            .geometry_snapshot()
            .clone();
        enter(&mut app, "BooleanSplit");
        pick(&mut app, ids[0]);
        enter(&mut app, "");
        pick(&mut app, ids[1]);
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().len(), 3);
        assert_eq!(app.document.selected_object_count(), 0);
        assert!(
            cutter.shares_storage_with(app.document.object(ids[1]).unwrap().geometry_snapshot())
        );
        let mut solids = 0;
        let mut open = 0;
        for object in app.document.objects().filter(|o| o.id() != ids[1]) {
            assert_eq!(object.geometry_user_text()["Code"], "original");
            let Geometry::Brep(b) = object.geometry() else {
                panic!()
            };
            if b.is_solid() {
                solids += 1;
            } else {
                open += 1;
            }
        }
        assert_eq!((solids, open), (1, 1));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 3);
    }
}
#[test]
fn boolean_split_open_preselection_keep_and_cancellation_preserve_inputs() {
    let (mut app, ids) = fixture(false);
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.select_command_results([ids[0]]).unwrap();
    enter(&mut app, "BooleanSplit DeleteInput=No");
    pick(&mut app, ids[1]);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    app.document.select_command_results([ids[0]]).unwrap();
    enter(&mut app, "BooleanSplit");
    pick(&mut app, ids[1]);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.object(ids[0]).is_some());
}
