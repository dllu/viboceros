use super::*;
fn enter(app: &mut VibocerosApp, word: &str) {
    app.command_input = word.into();
    app.run_command();
}
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn sources(app: &mut VibocerosApp) -> [ObjectId; 2] {
    [p(12., 0.), p(0., 12.)].map(|end| {
        app.document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(p(0., 0.), end, app.document.tolerance())
                    .unwrap(),
            ))
            .unwrap()
    })
}
#[test]
fn arc_tangent_native_frontend_intervals_sources_and_actual_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Arc",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_basic_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_modes_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_spatial_rhino_20261017.json"
                ),
                5,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_memory.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_memory_rhino_20261017.json"
                ),
                3,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_curved.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_curved_rhino_20261017.json"
                ),
                5,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_tangent_curved_variants.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_curved_variants_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_tolerance.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_tolerance_rhino_20261017.json"
                ),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tangent_pair_types.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tangent_pair_types_rhino_20261017.json"
                ),
                1,
            ),
        ],
    );
}
#[test]
fn arc_tangent_final_choice_preview_retries_preferences_and_one_history_owner() {
    let mut app = test_app();
    let [a, b] = sources(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Arc");
    enter(&mut app, "Tangent");
    assert!(app.pick_tangent_at(a, None, p(3., 0.)));
    enter(&mut app, "3");
    assert!(app.pick_tangent_at(b, None, p(0., 4.)));
    assert_eq!(app.document.objects().len(), 2);
    let support = app
        .tangent_session
        .as_ref()
        .unwrap()
        .support
        .as_ref()
        .unwrap()
        .clone();
    let before = format!("{:?}", app.document);
    let preview = viboceros_geometry::try_tangent_arc(
        support.circle,
        &support.contact_points,
        p(1., 1.),
        app.document.tolerance(),
    )
    .unwrap();
    assert_eq!(format!("{:?}", app.document), before);
    assert!(!app.accept_drafting_point(support.circle.center()));
    assert!(app.tangent_session.as_ref().unwrap().support.is_some());
    assert!(app.accept_drafting_point(p(1., 1.)));
    let Geometry::Arc(arc) = app
        .document
        .objects()
        .find(|o| ![a, b].contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!()
    };
    assert_eq!(*arc, preview);
    assert_eq!(app.commands.arc_tangent_radius(), 3.);
    assert_eq!(app.commands.circle_preferences().radius, 1.);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Arc"));
    assert_eq!(app.commands.arc_tangent_radius(), 3.);
    assert!(app.document.undo().unwrap().is_none());
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Arc"));
    assert!(!app.document.is_selected(a) && !app.document.is_selected(b));
}
#[test]
fn arc_tangent_source_changes_discard_the_reference_circle_and_cancel_cleanly() {
    for ready in [false, true] {
        for change in 0..5 {
            let mut app = test_app();
            let [a, b] = sources(&mut app);
            enter(&mut app, "A Tangent");
            assert!(app.pick_tangent_at(a, None, p(3., 0.)));
            if ready {
                enter(&mut app, "3");
                assert!(app.pick_tangent_at(b, None, p(0., 4.)));
                assert!(app.tangent_session.as_ref().unwrap().support.is_some());
            }
            match change {
                0 => {
                    app.document
                        .replace_object_geometries([(a, Geometry::Point(p(0., 0.)))])
                        .unwrap();
                }
                1 => {
                    app.document.set_objects_visibility([a], false).unwrap();
                }
                2 => {
                    app.document.set_objects_locked([a], true).unwrap();
                }
                3 => {
                    app.document.delete_objects([a]).unwrap();
                }
                _ => {
                    app.document
                        .set_tolerance(Tolerance::try_new(2e-9, 1e-12, 1e-10).unwrap());
                }
            }
            app.document.clear_history().unwrap();
            app.validate_tangent_sources();
            assert!(app.picking_tangent_curve());
            assert!(app.tangent_session.as_ref().unwrap().support.is_none());
            assert!(
                app.tangent_session
                    .as_ref()
                    .unwrap()
                    .input
                    .anchor()
                    .is_none()
            );
            enter(&mut app, "Cancel");
            assert!(app.tangent_session.is_none());
            assert!(app.document.undo().unwrap().is_none());
        }
    }
}
