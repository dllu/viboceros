use super::boolean_intersection::{enter, pick};
use super::*;
use viboceros_command::CommandContext;
use viboceros_geometry::{Brep, NurbsSurface};

fn plane(app: &mut VibocerosApp, partial: bool, surface: bool) -> ObjectId {
    let geometry = NurbsSurface::try_bilinear([
        point(1., -1., -1.),
        point(1., if partial { 1. } else { 3. }, -1.),
        point(1., if partial { 1. } else { 3. }, 3.),
        point(1., -1., 3.),
    ])
    .unwrap();
    app.document
        .add_geometry(if surface {
            Geometry::NurbsSurface(geometry)
        } else {
            Geometry::Brep(Brep::try_surface_face(geometry, app.document.tolerance()).unwrap())
        })
        .unwrap()
}

#[test]
fn boolean_split_plane_picker_accepts_surfaces_and_preserves_source_until_confirmation() {
    for surface in [false, true] {
        let mut app = test_app();
        let target = app
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
        let cutter = plane(&mut app, false, surface);
        let source = app
            .document
            .object(cutter)
            .unwrap()
            .geometry_snapshot()
            .clone();
        app.document.clear_history().unwrap();
        enter(&mut app, "BooleanSplit");
        pick(&mut app, target);
        enter(&mut app, "");
        pick(&mut app, cutter);
        assert_eq!(app.document.objects().len(), 2);
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert!(app.intersection_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().len(), 3);
        assert_eq!(app.document.selected_object_count(), 0);
        assert!(
            source.shares_storage_with(app.document.object(cutter).unwrap().geometry_snapshot())
        );
        assert!(app.document.object(target).is_none());
        for object in app.document.objects().filter(|o| o.id() != cutter) {
            let Geometry::Brep(b) = object.geometry() else {
                panic!()
            };
            assert!((b.signed_volume(app.document.tolerance()).unwrap() - 4.).abs() < 1e-10);
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 3);
    }
}

#[test]
fn boolean_split_plane_partial_sheet_reports_no_split_and_cancels_without_history() {
    let mut app = test_app();
    let target = app
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
    let cutter = plane(&mut app, true, true);
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BooleanSplit");
    pick(&mut app, target);
    enter(&mut app, "");
    pick(&mut app, cutter);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(app.command_log.back().unwrap().contains("did not split"));
}
