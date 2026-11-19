use super::super::circle_preview::{screen_point, viewport_frame};
use super::super::extrude_surface::tangents::profile;
use super::*;

#[test]
fn surface_path_tangent_option_changes_the_subcurve_preview_and_survives_completion() {
    for split in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Front);
        app.active_viewport = 0;
        app.maximized_viewport = Some(0);
        app.osnap = false;
        app.grid_snap = false;
        let source = profile("capsule_seam", app.document.tolerance());
        let id = app.document.add_geometry(source).unwrap();
        let rail = app
            .document
            .add_geometry(Geometry::NurbsCurve(
                viboceros_geometry::NurbsCurve::try_new(
                    1,
                    vec![
                        point(10., 20., 0.),
                        point(10., 21., 2.),
                        point(10., 20., 4.),
                    ],
                    vec![0., 0., 0.5, 1., 1.],
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = format!("{:?}", app.document);
        enter(&mut app, "ExtrudeSrfAlongCrv SubCurve Solid=Yes");
        assert!(
            !app.surface_path_prompt
                .as_ref()
                .unwrap()
                .options
                .split_at_tangents
        );
        if split {
            enter(&mut app, "_SplitAtTangents");
            assert!(
                app.command_log
                    .back()
                    .unwrap()
                    .contains("SplitAtTangents=Yes")
            );
        }
        enter(&mut app, &format!("Path={rail}"));
        enter(&mut app, "w10,20,0");
        let ctx = egui::Context::default();
        viewport_frame(&mut app, &ctx, vec![]);
        viewport_frame(&mut app, &ctx, vec![]);
        app.viewports[0]
            .zoom_extents(&app.document, Default::default())
            .unwrap();
        viewport_frame(&mut app, &ctx, vec![]);
        let end = screen_point(&app, point(10., 20., 3.));
        let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(!wires.is_empty());
        let seam = screen_point(&app, point(-2., -1., 1.5));
        assert_eq!(
            wires.iter().flatten().any(|p| p.distance(seam) < 1e-3),
            split,
        );
        let bend_y = screen_point(&app, point(0., 1., 2.)).y;
        assert!(wires.iter().any(|[a, b]| {
            (a.y - bend_y).abs() < 1e-3 && (b.y - bend_y).abs() < 1e-3 && (a.x - b.x).abs() > 1.
        }));
        assert_eq!(format!("{:?}", app.document), before);

        enter(&mut app, "w10,20.5,3");
        {
            assert!(app.surface_path_prompt.is_none(), "{:?}", app.command_log);
            let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
            else {
                panic!("expected the committed extrusion");
            };
            assert_eq!(result.faces().len(), if split { 12 } else { 4 });
            assert_eq!(
                app.commands.surface_path_settings().split_at_tangents,
                split
            );
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().count(), 2);
        }
    }
}
