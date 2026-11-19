use super::*;
use crate::app::extrude_tapered::Stage;
use viboceros_geometry::{NurbsSurface, Polyline3};

fn raised_profile(app: &mut VibocerosApp, surface: bool) -> ObjectId {
    let corners = [
        point(0., 0., 2.),
        point(6., 0., 2.),
        point(6., 4., 2.),
        point(0., 4., 2.),
    ];
    let geometry = if surface {
        Geometry::NurbsSurface(NurbsSurface::try_bilinear(corners).unwrap())
    } else {
        Geometry::Polyline(
            Polyline3::try_new(
                corners.into_iter().chain([corners[0]]).collect(),
                app.document.tolerance(),
            )
            .unwrap(),
        )
    };
    let source = app.document.add_geometry(geometry).unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    source
}

fn command(surface: bool) -> &'static str {
    if surface {
        "ExtrudeSrfTapered"
    } else {
        "ExtrudeCrvTapered"
    }
}

#[test]
fn tapered_getters_edit_the_distance_base_direction_and_options_independently() {
    for surface in [false, true] {
        let mut app = test_app();
        raised_profile(&mut app, surface);
        enter(&mut app, command(surface));
        let log = app.command_log.back().unwrap();
        assert!(
            log.contains("<1>") && log.contains("DraftAngle=5") && log.contains("Solid=No"),
            "{log}"
        );
        let first = app.tapered_distance_constraint().unwrap();
        assert_eq!(first.anchor, point(3., 2., 2.));
        enter(&mut app, "_SetBasePoint");
        enter(&mut app, "w10,20,30");
        assert_eq!(
            app.tapered_distance_constraint().unwrap().anchor,
            point(10., 20., 30.)
        );
        enter(&mut app, "_Direction");
        enter(&mut app, "w50,60,70");
        assert!(matches!(
            app.active_command,
            Some(InteractiveCommand::ExtrudeTapered {
                stage: Stage::DirectionEnd { .. },
                ..
            })
        ));
        enter(&mut app, "w50,60,64");
        let adjusted = app.tapered_distance_constraint().unwrap();
        assert_eq!(adjusted.anchor, point(10., 20., 30.));
        assert_eq!(
            adjusted.direction.unwrap().as_vector().to_array(),
            [0., 0., -1.]
        );
        enter(&mut app, "_DraftAngle");
        enter(&mut app, "17");
        enter(&mut app, "_Corners");
        enter(&mut app, "_Round");
        enter(&mut app, "_Solid _DeleteInput");
        let Some(InteractiveCommand::ExtrudeTapered {
            stage: Stage::Distance,
            options,
            ..
        }) = app.active_command
        else {
            panic!()
        };
        assert_eq!(options.draft_angle, 17.);
        assert!(options.round_corners && options.solid && options.delete_input);
        enter(&mut app, "Cancel");
        let settings = app.commands.tapered_extrusion_settings(surface);
        assert_eq!(settings, Default::default());
        assert!(!app.document.can_undo());
        assert_eq!(app.document.objects().count(), 1);
    }
}

#[cfg(feature = "native-smlib")]
fn result(app: &VibocerosApp, source: ObjectId) -> &viboceros_geometry::Brep {
    let Some(Geometry::Brep(result)) = app
        .document
        .objects()
        .find(|object| object.id() != source)
        .map(|object| object.geometry())
    else {
        panic!("expected tapered extrusion: {:?}", app.command_log);
    };
    result
}

#[cfg(feature = "native-smlib")]
#[test]
fn tapered_distance_and_reverse_direction_match_rhino_signed_draft_and_selection() {
    for surface in [false, true] {
        for reverse in [false, true] {
            let mut app = test_app();
            let source = raised_profile(&mut app, surface);
            enter(&mut app, &format!("{} Solid=Yes", command(surface)));
            if reverse {
                enter(&mut app, "_Direction");
                enter(&mut app, "w10,20,30");
                enter(&mut app, "w10,20,25");
            }
            enter(&mut app, "-2");
            let volume = result(&app, source)
                .signed_volume(app.document.tolerance())
                .unwrap()
                .abs();
            let expected = if reverse {
                51.5811920476562
            } else {
                44.582098965582226
            };
            assert!((volume - expected).abs() < 1e-5, "{volume}");
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                [source]
            );
            let settings = app.commands.tapered_extrusion_settings(surface);
            assert_eq!(settings.distance, -2.);
            assert!(settings.options.solid);
            assert_eq!(settings.options.direction, None);
            assert_eq!(
                app.commands.tapered_extrusion_settings(!surface),
                Default::default()
            );
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().count(), 1);
            assert_eq!(app.document.selected_object_count(), 0);
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().count(), 2);
            assert_eq!(app.document.selected_object_count(), 0);
            assert_eq!(app.commands.tapered_extrusion_settings(surface), settings);
        }
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn tapered_enter_uses_the_last_signed_distance_and_successful_options() {
    let mut app = test_app();
    let source = raised_profile(&mut app, true);
    enter(&mut app, "ExtrudeSrfTapered");
    enter(&mut app, "DraftAngle=12 Solid=Yes Corners=Round");
    enter(&mut app, "-2");
    let initial = result(&app, source).clone();
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfTapered DraftAngle=17 Solid=No");
    enter(&mut app, "Cancel");
    enter(&mut app, "ExtrudeSrfTapered");
    assert!(app.command_log.back().unwrap().contains("<-2>"));
    enter(&mut app, "_Enter");
    assert_eq!(result(&app, source), &initial);
}

#[cfg(feature = "native-smlib")]
#[test]
fn tapered_viewport_hover_previews_the_same_draft_and_height_as_click() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
    use std::time::{Duration, Instant};

    for surface in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Front);
        app.active_viewport = 0;
        app.maximized_viewport = Some(0);
        app.osnap = false;
        app.grid_snap = false;
        let source = raised_profile(&mut app, surface);
        let ctx = egui::Context::default();
        enter(&mut app, command(surface));
        viewport_frame(&mut app, &ctx, vec![]);
        viewport_frame(&mut app, &ctx, vec![]);
        let bounds = viboceros_geometry::BoundingBox3::from_points([
            point(-3., -5., -4.),
            point(9., 9., 8.),
        ])
        .unwrap();
        app.viewports[0].zoom_bounding_box(bounds).unwrap();
        let target = point(3., 2., 5.);
        let move_pointer = |app: &mut VibocerosApp| {
            viewport_frame(app, &ctx, vec![]);
            for _ in 0..8 {
                let pointer = screen_point(app, target);
                viewport_frame(app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
                if screen_point(app, target) == pointer {
                    return pointer;
                }
            }
            panic!("tapered preview layout did not settle");
        };
        let wait = |app: &mut VibocerosApp, expanding: bool| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let lines = viewport_frame(app, &ctx, vec![]);
                if let Some((height, _)) = app
                    .tapered_extrusion_session
                    .as_ref()
                    .and_then(|session| session.preview_edges(&app.document, app.active_command))
                {
                    let offset =
                        height * 5_f64.to_radians().tan() * if expanding { -1. } else { 1. };
                    let expected = screen_point(app, point(offset, offset, 2. + height));
                    if lines
                        .iter()
                        .flatten()
                        .any(|pixel| pixel.distance(expected) < 0.5)
                    {
                        return height;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "taper preview missed its offset edge, surface={surface}, expanding={expanding}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        move_pointer(&mut app);
        assert!((wait(&mut app, false) - 3.).abs() < 0.02);
        enter(&mut app, "_FlipAngle");
        let pointer = move_pointer(&mut app);
        let height = wait(&mut app, true);
        assert_eq!(app.document.objects().count(), 1);
        assert!(!app.document.can_undo());
        viewport_click(&mut app, &ctx, pointer);
        let output = result(&app, source);
        let offset = height * 5_f64.to_radians().tan();
        assert!((output.bounds().max().z() - (2. + height)).abs() < 1e-6);
        assert!(output.vertices().iter().any(|vertex| {
            vertex
                .point()
                .distance_to(point(-offset, -offset, 2. + height))
                .unwrap()
                < 1e-5
        }));
    }
}
