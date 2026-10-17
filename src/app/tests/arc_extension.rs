use super::*;
fn enter(app: &mut VibocerosApp, word: &str) {
    app.command_input = word.into();
    app.run_command();
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn arc_extension_native_frontend_sources_and_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Arc",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_relative.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_relative_rhino_20261017.json"
                ),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_selection.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_selection_rhino_20261017.json"
                ),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_end_choice.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_end_choice_rhino_20261017.json"
                ),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_sources.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_sources_rhino_20261017.json"
                ),
                8,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_basic_rhino_20261017.json"
                ),
                4,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_extension_center_numeric.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_center_numeric_rhino_20261017.json"
                ),
                2,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_extension_center_cursor.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_center_cursor_rhino_20261017.json"
                ),
                2,
            ),
        ],
    );
}
#[test]
fn arc_extension_preview_center_projection_history_and_source_invalidation() {
    for center in [false, true] {
        for change in 0..5 {
            let mut app = test_app();
            let id = app
                .document
                .add_geometry(Geometry::Line(
                    viboceros_geometry::LineSegment::try_new(
                        p(0., 0., 0.),
                        p(10., 0., 0.),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            app.document.clear_history().unwrap();
            enter(&mut app, "Arc Deformable Degree=5 PointCount=8 Extension");
            assert!(app.picking_arc_extension_curve());
            assert!(app.pick_arc_extension_at(id, None, p(9.5, 0., 0.)));
            if center {
                enter(&mut app, "Center");
                enter(&mut app, "w12,4,0");
                assert_eq!(app.last_point, Some(p(10., 4., 0.)));
            }
            let s = app.arc_extension_session.as_ref().unwrap();
            assert!(s.input.preview(p(13., 4., 0.)).is_some());
            assert_eq!(app.document.objects().len(), 1);
            assert!(app.document.undo().unwrap().is_none());
            match change {
                0 => {
                    app.document
                        .replace_object_geometries([(id, Geometry::Point(p(0., 0., 0.)))])
                        .unwrap();
                }
                1 => {
                    app.document.set_objects_visibility([id], false).unwrap();
                }
                2 => {
                    app.document.set_objects_locked([id], true).unwrap();
                }
                3 => {
                    app.document.delete_objects([id]).unwrap();
                }
                _ => {
                    app.document.set_tolerance(
                        viboceros_geometry::Tolerance::try_new(0.01, 0.01, 0.01).unwrap(),
                    );
                }
            }
            app.validate_arc_extension_source();
            assert!(app.picking_arc_extension_curve());
            assert!(!app.accept_drafting_point(p(13., 4., 0.)));
            assert_eq!(app.commands.arc_deformation(), Default::default());
            enter(&mut app, "Cancel");
            assert!(app.arc_extension_session.is_none());
        }
    }
}
#[test]
fn arc_extension_preview_and_commit_share_output_and_keep_source_storage() {
    for deformable in [false, true] {
        let mut app = test_app();
        let id = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    p(0., 0., 0.),
                    p(10., 0., 0.),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let source = app.document.object(id).unwrap().geometry_snapshot().clone();
        app.document.clear_history().unwrap();
        enter(
            &mut app,
            if deformable {
                "Arc Deformable Extension"
            } else {
                "Arc Extension"
            },
        );
        assert!(app.pick_arc_extension_at(id, None, p(9.5, 0., 0.)));
        let arc = app
            .arc_extension_session
            .as_ref()
            .unwrap()
            .input
            .preview(p(13., 4., 0.))
            .unwrap();
        let expected = if let Some(s) = app.arc_deform_session {
            s.output().curve(arc).unwrap()
        } else {
            arc.to_nurbs().unwrap()
        };
        assert!(app.accept_drafting_point(p(13., 4., 0.)));
        let output = app.document.objects().find(|o| o.id() != id).unwrap();
        assert_eq!(
            output
                .geometry()
                .nurbs_curve_representation()
                .unwrap()
                .unwrap(),
            expected
        );
        assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Arc"));
        assert!(app.document.undo().unwrap().is_none());
        assert_eq!(app.document.redo().unwrap().as_deref(), Some("Arc"));
    }
}
