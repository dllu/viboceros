use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::curve_fillet::enter;
use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::{Brep, NurbsCurve, NurbsSurface};

mod alignment;
mod miters;

fn setup(profile: &str, pole: Point3) -> (VibocerosApp, [ObjectId; 3]) {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,5", profile] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.add_geometry(Geometry::Point(pole)).unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    (app, ids.try_into().unwrap())
}

fn wait_for_wires(
    app: &mut VibocerosApp,
    context: &egui::Context,
    ready: impl Fn(&[NurbsCurve]) -> bool,
) -> Vec<NurbsCurve> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        viewport_frame(app, context, vec![]);
        let wires = app.sweep_session.as_ref().unwrap().preview_wires();
        if ready(wires) {
            return wires.to_vec();
        }
        assert!(
            Instant::now() < deadline,
            "Sweep1 preview did not update: {:?}",
            app.command_log
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn hover_pole(
    app: &mut VibocerosApp,
    context: &egui::Context,
    cursor: Point3,
    pole: Point3,
) -> (egui::Pos2, Vec<NurbsCurve>) {
    viewport_frame(app, context, vec![]);
    let pixel = screen_point(app, cursor);
    viewport_frame(app, context, vec![egui::Event::PointerMoved(pixel)]);
    let wires = wait_for_wires(app, context, |wires| {
        wires.iter().any(|wire| {
            let domain = wire.domain();
            [*domain.start(), *domain.end()]
                .into_iter()
                .any(|parameter| {
                    wire.evaluate(parameter)
                        .is_ok_and(|point| point.distance_to(pole).unwrap() < 1e-5)
                })
        })
    });
    assert!(!viewport_frame(app, context, vec![]).is_empty());
    (pixel, wires)
}

#[test]
fn sweep_section_refit_updates_preview_and_commits_the_reviewed_geometry() {
    let (mut app, [rail, profile, _]) =
        setup("Curve 0,0,0 1,2,0 3,0,0 Degree=2", point(9., 9., 9.));
    let tolerance = app.document.tolerance();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1");
    for id in [rail, profile] {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "");
    let initial = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());

    enter(&mut app, "Simplify");
    enter(&mut app, "Refit");
    let fitted = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != initial
    });
    enter(&mut app, "RefitTolerance");
    assert!(app.command_log.back().unwrap().contains("[0.01]"));
    enter(&mut app, "0.1");
    let coarse = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != fitted && wires != initial
    });
    assert_ne!(fitted, coarse);
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == fitted);
    enter(&mut app, "RefitTolerance=0.0005");
    let adaptive = wait_for_wires(&mut app, &context, |wires| {
        wires.iter().any(|wire| wire.control_points().len() == 6)
    });
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the refitted Sweep1 draft to commit");
    };
    assert_eq!(output.wireframe_curves(1, tolerance).unwrap(), adaptive);
    let surface = output.faces()[0].surface();
    assert_eq!((surface.degree_u(), surface.degree_v()), (1, 3));
    assert_eq!(surface.control_point_count_v(), 6);
    let native = point(0.2746064667026328, 0.5492129334052656, 0.);
    let handle = surface.control_point(0, 1).unwrap().point();
    assert!(handle.distance_to(native).unwrap() < 1e-7);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    enter(&mut app, "Sweep1");
    enter(&mut app, "RefitTolerance");
    assert!(app.command_log.back().unwrap().contains("[0.0005]"));
}

#[test]
fn sweep_refit_previews_a_shared_grid_for_different_profiles() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 0,0,5",
        "Curve 0,0,0 1,2,0 3,0,0 Degree=2",
        "Curve 0,0,5 2,4,5 6,0,5 Degree=2",
    ] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();

    enter(&mut app, "Sweep1 Simplify=Refit RefitRail=Yes");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let coarse = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "RefitTolerance=0.001");
    let adaptive = wait_for_wires(&mut app, &context, |wires| {
        wires.iter().any(|wire| wire.control_points().len() == 14)
    });
    assert_ne!(coarse, adaptive);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    let Geometry::Brep(output) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the shared-grid Sweep1 draft to commit");
    };
    assert_eq!(
        output
            .wireframe_curves(1, app.document.tolerance())
            .unwrap(),
        adaptive
    );
    let surface = output.faces()[0].surface();
    assert_eq!(surface.control_point_count_v(), 14);
    let handle = surface.control_point(0, 1).unwrap().point();
    assert!(
        handle
            .distance_to(point(0.12477591125166383, 0.24955182250332766, 0.))
            .unwrap()
            < 1e-7
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
}

#[test]
fn sweep_refit_reports_unresolved_subdivision_during_review() {
    let mut app = test_app();
    for input in [
        "Line 0,0,0 0,0,5",
        "Curve 0,0,0 2,4,0 6,0,0 Degree=2",
        "Curve 0,0,5 2,4,5 6,0,5 Degree=2",
    ] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.clear_history().unwrap();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();

    enter(&mut app, "Sweep1 Refit=0.001");
    for source in &sources {
        enter(&mut app, &source.id().to_string());
    }
    enter(&mut app, "");
    let diagnostic = "could not resolve the shared profile subdivision";
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        viewport_frame(&mut app, &context, vec![]);
        if app.command_hint().contains(diagnostic) {
            break;
        }
        assert!(Instant::now() < deadline, "{}", app.command_hint());
        std::thread::sleep(Duration::from_millis(5));
    }
    enter(&mut app, "");
    assert!(app.command_log.back().unwrap().contains(diagnostic));
    assert!(app.command_hint().contains(diagnostic));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "RefitTolerance=0.01");
    wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    assert!(app.command_hint().starts_with("Review the sweep"));
}

#[test]
fn sweep_roadlike_axis_uses_the_snapped_cursor_and_commits_the_draft() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let mid = std::f64::consts::FRAC_1_SQRT_2;
    let arc = CircularArc3::try_from_three_points(
        point(1., 0., 0.),
        point(mid, mid, 0.),
        point(0., 1., 0.),
        tolerance,
    )
    .unwrap();
    let rail = app.document.add_geometry(Geometry::Arc(arc)).unwrap();
    enter(&mut app, "Line 1,0,0 2,0,0.5");
    let profile = app.document.objects().last().unwrap().id();
    let default_tip = point(-1., -1.5, 0.25);
    let oblique_tip = point(-1., -0.5, 0.25);
    for p in [default_tip, oblique_tip] {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    app.document.clear_history().unwrap();
    let originals = app.document.objects().cloned().collect::<Vec<_>>();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();

    enter(&mut app, "Sweep1");
    for id in [rail, profile] {
        enter(&mut app, &id.to_string());
    }
    let initial = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    enter(&mut app, "Style");
    enter(&mut app, "_Roadlike");
    assert!(app.command_hint().contains("axis base point"));
    enter(&mut app, "w-1,-1.5,-0.75");
    assert!(app.command_hint().contains("second Roadlike axis point"));
    enter(&mut app, "w-1,-1.5,-0.75");
    assert!(app.command_log.back().unwrap().contains("distinct points"));
    enter(&mut app, "Undo");
    assert!(app.command_hint().contains("axis base point"));
    enter(&mut app, "w-1,-1.5,-0.75");
    enter(&mut app, "Point");
    assert!(app.snaps.model_override.is_some());
    let (_, upright) = hover_pole(&mut app, &context, default_tip, point(0., 2., 0.5));
    let (pixel, oblique) = hover_pole(
        &mut app,
        &context,
        oblique_tip,
        point(0., 2.060660171779821, -0.35355339059327373),
    );
    assert_ne!(upright, oblique);
    assert_eq!(app.document.undo_label(), None);
    viewport_click(&mut app, &context, pixel);
    assert!(app.snaps.model_override.is_none());
    wait_for_wires(&mut app, &context, |wires| wires == oblique);

    enter(&mut app, "SetAxis");
    enter(&mut app, "w0,0,0");
    app.cancel_current_prompt_or_selection();
    assert!(app.sweep_session.is_some());
    assert!(app.active_command.is_none());
    wait_for_wires(&mut app, &context, |wires| wires == oblique);
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == initial);
    enter(&mut app, "SetAxis");
    enter(&mut app, "w-1,-1.5,-0.75");
    enter(&mut app, "wr0,1,1");
    wait_for_wires(&mut app, &context, |wires| wires == oblique);
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );

    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected the Sweep1 draft to commit");
    };
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), oblique);
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (1, 4));
    // Rhino 8.32 with a two-point axis parallel to (0, 1, 1).
    let middle = sweep.faces()[0]
        .surface()
        .control_point(1, 1)
        .unwrap()
        .point();
    let native = point(2.179539348757548, 2.136646129944096, -0.5118016810571365);
    assert!(middle.distance_to(native).unwrap() < 1e-8, "{middle:?}");
    assert!((sweep.area(tolerance).unwrap() - 2.755682092970889).abs() < 1e-6);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
}

#[test]
fn sweep_point_preview_tracks_the_snapped_pole_and_committed_geometry() {
    for (command, profile_command, getter, height, rail_first) in [
        ("Sweep1", "Line 0,0,2.5 2,0,2.5", "StartPoint", 0., true),
        ("Sweep1", "Line 0,0,2.5 2,0,2.5", "EndPoint", 5., true),
        (
            "Sweep1 Parameters=2.5 FrameStyle=Roadlike Axis=0,1,0",
            "Circle 0,0,2.5 2",
            "EndPoint",
            5.,
            false,
        ),
    ] {
        let pole = point(1.5, 0., height);
        let (mut app, [rail, profile, _]) = setup(profile_command, pole);
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let context = egui::Context::default();
        enter(&mut app, command);
        let [first, last] = if rail_first {
            [rail, profile]
        } else {
            [profile, rail]
        };
        for id in [first, last] {
            enter(&mut app, &id.to_string());
        }
        let initial = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
        let closed = profile_command.starts_with("Circle");
        if !closed {
            enter(&mut app, "");
            assert!(app.command_hint().starts_with("Review the sweep"));
            enter(&mut app, "Undo");
            assert!(app.command_hint().starts_with("Select profiles"));
            assert_eq!(app.document.selected_object_count(), 2);
            enter(&mut app, "");
        }
        enter(&mut app, "Simplify");
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("None, Rebuild, or Refit")
        );
        enter(&mut app, "Rebuild");
        wait_for_wires(&mut app, &context, |wires| {
            !wires.is_empty() && wires != initial
        });
        enter(&mut app, "Undo");
        wait_for_wires(&mut app, &context, |wires| wires == initial);

        enter(&mut app, getter);
        let free = point(-1., 0., height);
        let (_, provisional) = hover_pole(&mut app, &context, free, free);
        viewport_frame(&mut app, &context, vec![egui::Event::PointerGone]);
        assert!(
            app.sweep_session
                .as_ref()
                .unwrap()
                .preview_wires()
                .is_empty()
        );
        enter(&mut app, "Point");
        let (pixel, hovered) = hover_pole(&mut app, &context, point(1.6, 0., height), pole);
        assert_ne!(hovered, provisional);
        assert!(app.snaps.model_override.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.document.undo_label(), None);
        viewport_click(&mut app, &context, pixel);
        assert!(app.snaps.model_override.is_none());
        wait_for_wires(&mut app, &context, |wires| wires == hovered);
        if closed {
            enter(&mut app, "");
            assert!(app.command_hint().starts_with("Drag seam points"));
            enter(&mut app, "");
        }
        assert!(app.command_hint().starts_with("Review the sweep"));
        wait_for_wires(&mut app, &context, |wires| wires == hovered);
        enter(&mut app, "");

        let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
            panic!("expected Sweep1 B-rep: {:?}", app.command_log);
        };
        assert!(brep.vertices().iter().any(|vertex| vertex.point() == pole));
        assert_eq!(
            brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
            hovered
        );
        assert_eq!(app.document.undo_label(), Some("Sweep1"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn sweep_one_closed_profiles_preview_drag_flip_reset_and_native_seam_geometry() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    for command in ["Line 1,0,0 1,0,6", "Circle 0,0,0 1", "Circle 0,0,6 1"] {
        enter(&mut app, command);
    }
    let upper = app.document.objects().last().unwrap();
    let upper_id = upper.id();
    let curve = upper
        .geometry_snapshot()
        .nurbs_curve_representation()
        .unwrap()
        .unwrap();
    let shifted = viboceros_command::prepare_curve_seam(
        &viboceros_geometry::Curve3::NurbsCurve(curve.clone()),
        curve.parameter_at(0.25).unwrap(),
    )
    .unwrap();
    app.document
        .replace_object_geometries([(upper_id, Geometry::from(shifted))])
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    let markers = |app: &VibocerosApp| {
        app.sweep_session
            .as_ref()
            .unwrap()
            .seam_editor()
            .unwrap()
            .markers()
    };

    enter(&mut app, "Sweep1");
    for object in &before {
        enter(&mut app, &object.id().to_string());
    }
    enter(&mut app, "");
    assert_eq!(markers(&app).len(), 2);
    assert!(app.command_hint().contains("SnapToKnots=Yes"));
    let automatic = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    assert!(
        markers(&app)[1]
            .point
            .distance_to(point(1., 0., 6.))
            .unwrap()
            < 1e-8
    );

    enter(&mut app, "Natural");
    let original = point(0., 1., 6.);
    assert!(markers(&app)[1].point.distance_to(original).unwrap() < 1e-8);
    let natural = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != automatic
    });
    let from = screen_point(&app, original);
    let destination = point(0., -1., 6.);
    let to = screen_point(&app, destination);
    let button = |position, pressed| egui::Event::PointerButton {
        pos: position,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(to)]);
    assert!(markers(&app)[1].point.distance_to(destination).unwrap() < 1e-8);
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != natural
    });
    viewport_frame(&mut app, &context, vec![button(to, false)]);
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == natural);

    let tangent = markers(&app)[1].tangent.unwrap();
    enter(&mut app, "Flip");
    enter(&mut app, &format!("SelID {upper_id}"));
    assert!(markers(&app)[1].tangent.unwrap().dot(tangent).unwrap() < -0.99);
    enter(&mut app, "Natural");
    enter(&mut app, "SnapToKnots=No");
    enter(&mut app, &format!("SelID {upper_id}"));
    let quarter = std::f64::consts::FRAC_1_SQRT_2;
    enter(&mut app, &format!("w{quarter},{quarter},6"));
    assert!(
        markers(&app)[1]
            .point
            .distance_to(point(quarter, quarter, 6.))
            .unwrap()
            < 1e-8
    );
    enter(&mut app, "SnapToKnots=Yes");
    enter(&mut app, "Automatic");
    wait_for_wires(&mut app, &context, |wires| wires == automatic);
    enter(&mut app, "Natural");
    wait_for_wires(&mut app, &context, |wires| wires == natural);

    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    assert_eq!(app.document.undo_label(), None);
    let aligned_marker = |app: &VibocerosApp| {
        app.sweep_session
            .as_ref()
            .unwrap()
            .alignment_markers()
            .unwrap()[1]
    };
    enter(&mut app, "AlignCrossSections");
    let marker = aligned_marker(&app);
    assert!(marker.point.distance_to(original).unwrap() < 1e-8);
    enter(&mut app, &format!("SelID {upper_id}"));
    let reversed = aligned_marker(&app);
    assert!(reversed.point.distance_to(marker.point).unwrap() < 1e-8);
    assert!(
        reversed
            .tangent
            .unwrap()
            .dot(marker.tangent.unwrap())
            .unwrap()
            < -0.99
    );
    enter(&mut app, "Undo");
    enter(&mut app, "");
    enter(&mut app, "ShapeBlending");
    enter(&mut app, "Global");
    wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != natural
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == natural);
    enter(&mut app, "");

    let Geometry::Brep(result) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 surface: {:?}", app.command_log);
    };
    assert_eq!((result.faces().len(), result.edges().len()), (1, 3));
    assert_eq!(result.wireframe_curves(1, tolerance).unwrap(), natural);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    let face = result.faces()[0].surface();
    assert!(face.is_closed_v().unwrap());
    assert_eq!([face.degree_u(), face.degree_v()], [3, 2]);
    let seam = face
        .evaluate(*face.domain_u().end(), *face.domain_v().start())
        .unwrap();
    assert!(seam.distance_to(original).unwrap() < 1e-8);
    let inner_seam = face.evaluate(2., *face.domain_v().start()).unwrap();
    assert!(
        inner_seam
            .distance_to(point(20. / 27., 7. / 27., 2.))
            .unwrap()
            < 1e-8
    );
    let area = result.area(tolerance).unwrap();
    assert!((area - 32.446010095173506).abs() < 1e-5, "{area}");
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn renaming_a_named_rail_invalidates_the_sweep_preview() {
    let (mut app, [rail, profile, _]) = setup("Line 0,0,2.5 2,0,2.5", point(1., 0., 5.));
    app.document
        .set_object_names([(rail, Some("SweepRail".into()))])
        .unwrap();
    enter(&mut app, "Sweep1 RailName=SweepRail");
    enter(&mut app, &profile.to_string());
    let context = egui::Context::default();
    wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    app.document
        .set_object_names([(rail, Some("OtherRail".into()))])
        .unwrap();
    let label = app.document.undo_label().unwrap().to_owned();
    viewport_frame(&mut app, &context, vec![]);
    assert!(app.sweep_session.is_none());
    assert_eq!(app.document.undo_label(), Some(label.as_str()));
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn sweep_one_surface_edge_pick_previews_the_selected_frame_style() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let support = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(-2., 0., 0.),
        point(-2., 6., -2.),
        point(0., 6., 0.),
    ])
    .unwrap();
    let brep = Brep::try_surface_face(support, tolerance).unwrap();
    let rail = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    enter(&mut app, "Line 0,0,0 2,0,0");
    let profile = app.document.objects().last().unwrap().id();
    let originals = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();

    enter(&mut app, "Sweep1");
    viewport_frame(&mut app, &context, vec![]);
    let pick = screen_point(&app, point(0., 3., 0.));
    viewport_click(&mut app, &context, pick);
    assert!(
        app.sweep_session
            .as_ref()
            .unwrap()
            .rail_targets()
            .any(|target| target.object() == rail)
    );
    enter(&mut app, &profile.to_string());
    let freeform = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    enter(&mut app, "Style");
    assert!(app.command_log.back().unwrap().contains("AlignWithSurface"));
    enter(&mut app, "AlignWithSurface");
    let aligned = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != freeform
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == freeform);
    enter(&mut app, "Style=AlignWithSurface");
    wait_for_wires(&mut app, &context, |wires| wires == aligned);
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 B-rep");
    };
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), aligned);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );

    let repeat_on_edge = |app: &mut VibocerosApp, expected: &[NurbsCurve]| {
        app.restore_curve_selection(vec![]);
        enter(app, "Sweep1");
        viewport_click(app, &context, pick);
        enter(app, &profile.to_string());
        enter(app, "");
        wait_for_wires(app, &context, |wires| wires == expected);
        enter(app, "Cancel");
    };
    repeat_on_edge(&mut app, &aligned);

    enter(&mut app, "Line 0,0,0 0,6,0");
    let loose_rail = app.document.objects().last().unwrap().id();
    app.restore_curve_selection(vec![loose_rail, profile]);
    enter(&mut app, "Sweep1");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("FrameStyle=Freeform")
    );
    wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "");
    assert!(app.command_line_idle());
    enter(&mut app, "Undo");
    enter(&mut app, "Undo");
    repeat_on_edge(&mut app, &freeform);
}

#[test]
fn sweep_one_rail_chain_preserves_connected_direction_and_keeps_sources() {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,3,0", "Line 0,6,0 0,3,0", "Line 0,0,0 2,0,0"] {
        enter(&mut app, input);
    }
    let originals = app.document.objects().cloned().collect::<Vec<_>>();
    let ids = originals
        .iter()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();

    enter(&mut app, "Sweep1 ChainEdges");
    enter(&mut app, &ids[0].to_string());
    assert_eq!(app.chain_prompt.as_ref().unwrap().highlights().len(), 2);
    app.cancel_current_prompt_or_selection();
    assert!(app.chain_prompt.is_none());
    assert!(app.command_hint().starts_with("Select a rail"));
    enter(&mut app, "ChainEdges AutoChain=No");
    for (index, id) in ids[..2].iter().enumerate() {
        enter(&mut app, &id.to_string());
        assert_eq!(
            app.chain_prompt.as_ref().unwrap().highlights().len(),
            index + 1
        );
    }
    enter(&mut app, "");
    assert!(app.chain_prompt.is_none());
    assert_eq!(
        app.sweep_session.as_ref().unwrap().rail_targets().count(),
        2
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &ids[2].to_string());
    let preview = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.command_hint().starts_with("Review the sweep"));
    enter(&mut app, "");
    let Geometry::Brep(sweep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 B-rep");
    };
    // Rhino 8.32 sweeps both rail pieces into one rectangular face.
    assert_eq!((sweep.faces().len(), sweep.edges().len()), (1, 4));
    assert!((sweep.area(tolerance).unwrap() - 12.).abs() < 1e-8);
    assert_eq!(sweep.wireframe_curves(1, tolerance).unwrap(), preview);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
}

#[test]
fn sweep_one_closed_preview_toggles_the_return_span_and_matches_commit() {
    let mut app = test_app();
    for text in ["Circle 0,0,0 1", "Line 1,0,0 3,0,0", "Line 0,1,0 0,3,0"] {
        enter(&mut app, text);
    }
    let originals = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    let tolerance = app.document.tolerance();
    let context = egui::Context::default();
    enter(&mut app, "Sweep1");
    for object in &originals {
        enter(&mut app, &object.id().to_string());
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("Closed / Style"));
    let open = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    enter(&mut app, "Closed");
    assert!(app.command_log.back().unwrap().contains("Choose Yes or No"));
    enter(&mut app, "Yes");
    let closed = wait_for_wires(&mut app, &context, |wires| {
        !wires.is_empty() && wires != open
    });
    enter(&mut app, "Undo");
    wait_for_wires(&mut app, &context, |wires| wires == open);
    enter(&mut app, "Closed=Yes");
    wait_for_wires(&mut app, &context, |wires| wires == closed);
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.command_line_idle());
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected Sweep1 B-rep");
    };
    assert_eq!((brep.faces().len(), brep.edges().len()), (4, 12));
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), closed);
    assert_eq!(app.document.undo_label(), Some("Sweep1"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
    app.restore_curve_selection(originals.iter().map(|object| object.id()).collect());
    enter(&mut app, "Sweep1");
    assert!(app.command_hint().contains("Closed / Style"));
    wait_for_wires(&mut app, &context, |wires| wires == open);
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
}
