use super::boolean_intersection::{enter, pick};
use super::*;
use viboceros_geometry::{Brep, NurbsSurface};
fn sheet(app: &mut VibocerosApp, points: [Point3; 4]) -> ObjectId {
    app.document
        .add_geometry(Geometry::Brep(
            Brep::try_surface_face(
                NurbsSurface::try_bilinear(points).unwrap(),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn mixed_open_getter_preserves_stage_order_remainders_and_one_history_entry() {
    for reverse in [false, true] {
        let mut app = test_app();
        let target = sheet(
            &mut app,
            [
                point(1., -1., -1.),
                point(1., 3., -1.),
                point(1., 3., 3.),
                point(1., -1., 3.),
            ],
        );
        let overlap = sheet(
            &mut app,
            [
                point(1., 0., 0.),
                point(1., 2., 0.),
                point(1., 2., 2.),
                point(1., 0., 2.),
            ],
        );
        let cross = sheet(
            &mut app,
            [
                point(-1., 1., -1.),
                point(-1., 1., 3.),
                point(3., 1., 3.),
                point(3., 1., -1.),
            ],
        );
        app.document.clear_history().unwrap();
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "BooleanSplit");
        pick(&mut app, target);
        enter(&mut app, "");
        let order = if reverse {
            [cross, overlap]
        } else {
            [overlap, cross]
        };
        for id in order {
            pick(&mut app, id);
        }
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            original
        );
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().len(), if reverse { 4 } else { 5 });
        assert_eq!(app.document.selected_object_count(), 0);
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            original
        );
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), if reverse { 4 } else { 5 });
    }
}
