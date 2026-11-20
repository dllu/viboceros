use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::NurbsCurve;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn sources(app: &mut VibocerosApp) -> Vec<ObjectId> {
    for input in [
        "Line 0,0,0 0,0,5",
        "Line 2,0,5 2,0,0",
        "Line 2,0,5 0,0,5",
        "Line 0,0,0 2,0,0",
    ] {
        enter(app, input);
    }
    app.document.objects().map(|o| o.id()).collect()
}
#[test]
fn sweep_two_rail_picker_filters_and_cancels_without_model_edits() {
    let mut app = test_app();
    let ids = sources(&mut app);
    let other = app
        .document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Sweep2");
    assert_eq!(
        app.command_hint(),
        "Select the first rail or choose ChainEdges"
    );
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(other),
        mode: SelectionMode::Replace,
    });
    assert!(!app.document.is_selected(other));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        app.command_hint(),
        "Select the second rail or choose ChainEdges or Undo"
    );
    enter(&mut app, "");
    assert!(app.sweep_two_rails.is_some());
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn sweep_two_rail_preselection_and_clicked_input_commit_one_undoable_result() {
    for (pre, command) in [(false, "Sweep2"), (true, "Sweep2"), (true, "_-Sweep2")] {
        let mut app = test_app();
        let ids = sources(&mut app);
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if pre {
            app.document
                .select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, command);
        if !pre {
            for id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(*id),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        if command == "Sweep2" {
            assert!(app.sweep_two_rails.is_some());
            assert_eq!(app.document.objects().count(), 4);
            assert_eq!(app.viewport_object_filter(), None);
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.objects().count(), 5);
        assert_eq!(app.document.undo_label(), Some("Sweep2"));
        let result = app
            .document
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .clone();
        assert!(matches!(result.geometry(),Geometry::Brep(b) if b.faces().len()==1));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(result.id()), Some(&result));
    }
}

fn wait_for_draft(app: &mut VibocerosApp, context: &egui::Context) -> Vec<NurbsCurve> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let strokes = viewport_frame(app, context, vec![]);
        let wires = app.sweep_two_rails.as_ref().unwrap().preview_wires();
        if !wires.is_empty() && !strokes.is_empty() {
            return wires.to_vec();
        }
        assert!(Instant::now() < deadline, "{:?}", app.command_log);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn sweep_two_rail_profile_bracket_shortens_preview_and_commit() {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,8", "Line 2,0,8 2,0,0"] {
        enter(&mut app, input);
    }
    let rails = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    let sections = [(2., 0.8), (6., 1.2)].map(|(z, height)| {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0., 0., z), point(1., height, z), point(2., 0., z)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        app.document
            .add_geometry(Geometry::NurbsCurve(curve))
            .unwrap()
    });
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    for id in rails {
        enter(&mut app, &id.to_string());
    }
    viewport_frame(&mut app, &context, vec![]);
    for (id, z) in sections.into_iter().zip([2., 6.]) {
        let pixel = screen_point(&app, point(1., 0., z));
        viewport_click(&mut app, &context, pixel);
        assert!(app.document.is_selected(id));
        let wires = wait_for_draft(&mut app, &context);
        let max_z = wires
            .iter()
            .flat_map(|wire| wire.control_points().iter().map(|c| c.point().z()))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max_z - if z == 2. { 8. } else { 6. }).abs() < 1e-8);
    }
    let bracket = wait_for_draft(&mut app, &context);
    enter(&mut app, "");
    assert_eq!(wait_for_draft(&mut app, &context), bracket);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    let tolerance = app.document.tolerance();
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), bracket);
    assert!((brep.area(tolerance).unwrap() - 9.201940759406227).abs() < 1e-5);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn sweep_two_rail_refit_review_and_script_use_the_same_draft() {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,8", "Line 1,0,0 5,0,8"] {
        enter(&mut app, input);
    }
    for (width, z, height) in [(1., 0., 0.8), (5., 8., 1.)] {
        let arch = NurbsCurve::try_new(
            2,
            vec![
                point(0., 0., z),
                point(width / 2., height, z),
                point(width, 0., z),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        app.document
            .add_geometry(Geometry::NurbsCurve(arch))
            .unwrap();
    }
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let ids = before.iter().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.document
        .select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    let unchanged = wait_for_draft(&mut app, &context);
    enter(&mut app, "RefitRails");
    assert_eq!(app.command_hint(), "Refit rails? Choose Yes or No");
    enter(&mut app, "Yes");
    let fitted = wait_for_draft(&mut app, &context);
    assert_ne!(unchanged, fitted);
    enter(&mut app, "RefitRails=No");
    assert_eq!(wait_for_draft(&mut app, &context), unchanged);
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), fitted);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    let tolerance = app.document.tolerance();
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), fitted);
    let area = brep.area(tolerance).unwrap();
    assert!((area - 27.281218625057083).abs() < 1e-5, "{area}");
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "_-Sweep2 _RefitRails=_Yes");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a scripted Sweep2 B-rep");
    };
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), fitted);
}

#[test]
fn sweep_two_rail_single_profile_height_updates_preview_and_local_undo() {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,6", "Line 2,0,0 6,0,6"] {
        enter(&mut app, input);
    }
    let arch = NurbsCurve::try_new(
        2,
        vec![point(0., 0., 0.), point(1., 0.8, 0.), point(2., 0., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let profile = app
        .document
        .add_geometry(Geometry::NurbsCurve(arch))
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    viewport_frame(&mut app, &context, vec![]);
    for rail in [point(0., 0., 2.5), point(4., 0., 3.)] {
        let pixel = screen_point(&app, rail);
        viewport_click(&mut app, &context, pixel);
    }
    enter(&mut app, &profile.to_string());
    let scaled = wait_for_draft(&mut app, &context);
    enter(&mut app, "");
    assert!(app.command_hint().contains("MaintainHeight"));
    enter(&mut app, "MaintainHeight");
    assert_eq!(app.command_hint(), "Maintain height? Choose Yes or No");
    enter(&mut app, "Invalid");
    assert_eq!(wait_for_draft(&mut app, &context), scaled);
    enter(&mut app, "_Yes");
    let maintained = wait_for_draft(&mut app, &context);
    assert_ne!(scaled, maintained);
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), scaled);
    enter(&mut app, "MaintainHeight=Yes");
    assert_eq!(wait_for_draft(&mut app, &context), maintained);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert_eq!(
        brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
        maintained
    );
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn sweep_two_rail_review_updates_wires_and_commits_the_displayed_brep() {
    let mut app = test_app();
    let ids = sources(&mut app);
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    // Pick both rails in reverse creation order, followed by the two profiles.
    let order = [ids[1], ids[0], ids[3], ids[2]];
    enter(&mut app, &order[0].to_string());
    viewport_frame(&mut app, &context, vec![]);
    let rail_pixel = screen_point(&app, point(0., 0., 2.5));
    viewport_click(&mut app, &context, rail_pixel);
    enter(&mut app, &order[2].to_string());
    let profile_pixel = screen_point(&app, point(1., 0., 5.));
    viewport_click(&mut app, &context, profile_pixel);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        order
    );
    assert!(app.command_hint().starts_with("Select cross sections"));
    let unchanged = wait_for_draft(&mut app, &context);
    enter(&mut app, "");
    assert_eq!(wait_for_draft(&mut app, &context), unchanged);
    enter(&mut app, "Simplify");
    enter(&mut app, "Rebuild");
    let default_rebuild = wait_for_draft(&mut app, &context);
    assert_ne!(default_rebuild, unchanged);
    enter(&mut app, "Rebuild");
    enter(&mut app, "NaN");
    assert_eq!(wait_for_draft(&mut app, &context), default_rebuild);
    enter(&mut app, "7");
    let rebuilt = wait_for_draft(&mut app, &context);
    assert_ne!(rebuilt, default_rebuild);
    enter(&mut app, "Refit");
    enter(&mut app, "0.001");
    let refitted = wait_for_draft(&mut app, &context);
    assert_ne!(refitted, rebuilt);
    enter(&mut app, "Simplify=Rebuild");
    assert_eq!(wait_for_draft(&mut app, &context), rebuilt);
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), refitted);
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), rebuilt);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.command_line_idle());
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert_eq!(
        brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
        rebuilt
    );
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn changed_named_sweep_two_rail_cancels_review_and_restores_selection() {
    let mut app = test_app();
    let ids = sources(&mut app);
    app.document
        .set_object_names([
            (ids[0], Some("Left".into())),
            (ids[1], Some("Right".into())),
        ])
        .unwrap();
    let context = egui::Context::default();
    enter(&mut app, "Sweep2 Rail1Name=Left Rail2Name=Right");
    assert!(app.command_hint().starts_with("Select cross sections"));
    app.select_prompt_objects([ids[0], ids[3], ids[2]], SelectionMode::Add);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[3], ids[2]]
    );
    enter(&mut app, "");
    wait_for_draft(&mut app, &context);
    app.document
        .set_object_names([(ids[1], Some("Moved".into()))])
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    viewport_frame(&mut app, &context, vec![]);
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn sweep_two_rail_end_point_tracks_the_resolved_cursor_and_snap() {
    let mut app = test_app();
    app.grid_snap = false;
    let tolerance = Tolerance::try_new(0.35, 1e-12, 1e-10).unwrap();
    app.document = Document::new(tolerance);
    for line in ["Line -2,0,0 0,0,5", "Line 0,0,5 2,0,0", "Line -2,0,0 2,0,0"] {
        enter(&mut app, line);
    }
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let pole = point(0., 0., 5.);
    app.document.add_geometry(Geometry::Point(pole)).unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    for id in ids {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "Point");
    assert_eq!(
        app.command_hint(),
        "Pick the end point where both rails meet. Esc cancels"
    );
    enter(&mut app, "w0,0,7");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SweepTwoRailPoint)
    );
    assert_eq!(app.document.undo_label(), None);
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("point ends must lie on both rails"))
    );

    enter(&mut app, "NoSnap");
    viewport_frame(&mut app, &context, vec![]);
    let free_pixel = screen_point(&app, point(-0.2, 0., 5.));
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(free_pixel)],
    );
    let free = wait_for_draft(&mut app, &context);
    viewport_frame(&mut app, &context, vec![egui::Event::PointerGone]);
    assert!(
        app.sweep_two_rails
            .as_ref()
            .unwrap()
            .preview_wires()
            .is_empty()
    );

    enter(&mut app, "Undo");
    assert!(app.active_command.is_none());
    enter(&mut app, "Point");
    enter(&mut app, "Point"); // The ordinary one-shot object snap inside the getter.
    assert!(app.snaps.model_override.is_some());
    let pixel = screen_point(&app, point(0.1, 0., 5.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
    let snapped = wait_for_draft(&mut app, &context);
    assert_ne!(free, snapped);
    let endpoints = |wires: &[NurbsCurve]| {
        wires
            .iter()
            .flat_map(|curve| {
                let domain = curve.domain();
                [*domain.start(), *domain.end()].map(|parameter| curve.evaluate(parameter).unwrap())
            })
            .collect::<Vec<_>>()
    };
    let free_ends = endpoints(&free);
    assert!(
        free_ends
            .iter()
            .any(|&at| screen_point(&app, at).distance(free_pixel) < 0.1),
        "{free_ends:?}"
    );
    assert!(endpoints(&snapped).contains(&pole));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    viewport_click(&mut app, &context, pixel);
    assert!(app.snaps.model_override.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.viewport_object_filter(), None);
    assert_eq!(
        app.sweep_two_rails.as_ref().unwrap().preview_wires(),
        snapped
    );
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), snapped);
    assert!(brep.vertices().iter().any(|vertex| vertex.point() == pole));
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn sweep_two_rail_start_point_at_a_crossing_accepts_typed_world_input() {
    let mut app = test_app();
    for line in [
        "Line 2,0,-5 -2,0,5",
        "Line -2,0,-5 2,0,5",
        "Line -2,0,5 2,0,5",
    ] {
        enter(&mut app, line);
    }
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "Sweep2");
    for id in &ids[..2] {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "Point");
    enter(&mut app, ".xy");
    enter(&mut app, "w0,0,6");
    enter(&mut app, "w0,0,0mm");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, &ids[2].to_string());
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "");
    assert!(app.command_line_idle());
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert!(
        brep.vertices()
            .iter()
            .any(|vertex| vertex.point() == point(0., 0., 0.))
    );
}

mod rail_chains;
mod slashes;
