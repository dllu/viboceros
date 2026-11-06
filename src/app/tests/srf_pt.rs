use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/srf_pt.rs"]
mod replay;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn corner_surface_getters_match_native_trimmed_geometry_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("corners.3dm");
    let mut count = 0;
    for family in [
        "srf_pt_pilot",
        "srf_pt_holdouts",
        "srf_pt_precision",
        "srf_pt_admission",
        "srf_pt_singular",
        "srf_pt_origin",
    ] {
        let data = native::read(family);
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            let mut app = test_app();
            app.document = native::setup(v);
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let plane = native::frame(v);
            app.viewports[0].set_construction_plane(plane);
            if v["success"] != true {
                app.commands
                    .execute(&mut app.document, "Point 100,200,300")
                    .unwrap();
                app.document.undo().unwrap();
            }
            enter(&mut app, "SrfPt");
            for token in replay::tokens(v) {
                enter(&mut app, if token == "Enter" { "" } else { &token });
            }
            assert!(
                app.srf_pt_session.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert!(app.active_command.is_none());
            assert!(app.drafting_plane.is_none());
            assert_eq!(app.viewports[0].construction_plane(), plane);
            let (mut error, face_sense_matches) = replay::check_with_face_sense(
                &app.document,
                v,
                "after",
                family == "srf_pt_singular",
            );
            let mut archive_objects = None;
            if v["success"] == true {
                assert_eq!(app.document.undo_label(), Some("SrfPt"));
                app.document.undo().unwrap();
                error = error.max(replay::check(&app.document, v, "undo"));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                error = error.max(
                    replay::check_with_face_sense(
                        &app.document,
                        v,
                        "redo",
                        family == "srf_pt_singular",
                    )
                    .0,
                );
                app.commands
                    .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                    .unwrap();
                let mut restored = Document::new(app.document.tolerance());
                app.commands
                    .execute(&mut restored, &format!("Open3dm {}", path.display()))
                    .unwrap();
                let mut archive = v.clone();
                for o in archive["after"].as_array_mut().unwrap() {
                    o["selected"] = serde_json::json!(0);
                }
                error = error.max(
                    replay::check_with_face_sense(
                        &restored,
                        &archive,
                        "after",
                        family == "srf_pt_singular",
                    )
                    .0,
                );
                archive_objects = Some(restored.objects().len());
            } else {
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
            }
            for parent in parents {
                assert_eq!(app.document.object(parent.id()), Some(&parent));
                assert!(
                    parent.geometry_snapshot().shares_storage_with(
                        app.document
                            .object(parent.id())
                            .unwrap()
                            .geometry_snapshot()
                    )
                );
            }
            println!(
                "SRF_PT_UI {}",
                serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error,"archive_objects":archive_objects,"face_sense_matches_native":face_sense_matches})
            );
            assert!(error <= 1e-6, "{} {error:e}", row["id"]);
            count += 1;
        }
    }
    assert_eq!(count, 60);
}

#[test]
fn corner_surface_lifetime_and_cross_view_picks_keep_incomplete_geometry_outside_history() {
    let mut app = test_app();
    enter(&mut app, "SurfaceFromCorners");
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    app.active_viewport = 1;
    assert!(app.accept_drafting_point(point(3., 0., 0.)));
    app.active_viewport = 2;
    assert!(app.accept_drafting_point(point(1., 2., 4.)));
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Enter");
    let Geometry::Brep(b) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.vertices().len(), 3);
    assert_eq!(b.edges().len(), 3);
    assert!(
        b.faces()[0]
            .contains_parameters(0., 0., app.document.tolerance())
            .unwrap()
    );
    assert!(
        !b.faces()[0]
            .contains_parameters(
                *b.faces()[0].surface().domain_u().end(),
                *b.faces()[0].surface().domain_v().end(),
                app.document.tolerance()
            )
            .unwrap()
    );
    enter(&mut app, "SrfPt");
    enter(&mut app, "w0,0,0");
    app.document
        .add_geometry(Geometry::Point(point(8., 9., 10.)))
        .unwrap();
    assert!(!app.accept_drafting_point(point(3., 4., 5.)));
    assert!(app.active_command.is_none());
    assert!(app.srf_pt_session.is_none());
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "SrfPt");
    enter(&mut app, "w0,0,0");
    assert!(
        app.document
            .set_tolerance(Tolerance::try_new(0.01, 1e-9, 1e-6).unwrap())
    );
    assert!(!app.accept_drafting_point(point(3., 4., 5.)));
    assert!(app.active_command.is_none());
}

#[test]
fn corner_surface_viewport_preview_follows_picks_without_committing_objects() {
    let mut app = test_app();
    enter(&mut app, "SrfPt");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w4,0,0");
    let context = egui::Context::default();
    let ink = egui::Color32::from_rgb(20, 115, 190);
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let mut output = ViewportOutput::default();
        let input = ViewportInput {
            drafting: DraftingInput {
                active: true,
                anchor: app.active_command.and_then(InteractiveCommand::anchor),
                ..Default::default()
            },
            srf_pt_input: app.srf_pt_input(),
            ..Default::default()
        };
        let drawn = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
            },
        );
        let lines = drawn
            .shapes
            .iter()
            .filter(|s| matches!(&s.shape,egui::Shape::LineSegment{stroke,..} if stroke.color==ink))
            .count();
        drawn.drop_without_applying_deltas();
        (output, lines)
    };
    frame(&mut app, vec![]);
    let (output, lines) = frame(
        &mut app,
        vec![egui::Event::PointerMoved(egui::pos2(510., 230.))],
    );
    assert_eq!(lines, 3);
    assert!(output.picked_point.is_none());
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "w1,3,2");
    let (output, lines) = frame(&mut app, vec![]);
    assert_eq!(lines, 4);
    assert!(output.picked_point.is_none());
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Enter");
    assert!(app.srf_pt_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
}
