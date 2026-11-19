use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 0.),
                    point(2., 0., 0.),
                    point(0., 3., 0.),
                    point(2., 3., 1.),
                ],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn surface_apex_source_picker_and_atomic_options_survive_cancel() {
    let mut app = test_app();
    let s = source(&mut app);
    let before = app.document.object(s).unwrap().clone();
    enter(&mut app, "ExtrudeSrfToPoint Solid=No SplitAtTangents=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Some(InteractiveCommand::ExtrudeSurfaceToPoint { options }) = app.active_command else {
        panic!()
    };
    assert!(!options.solid);
    assert!(options.split_at_tangents);
    enter(&mut app, "Solid=Yes DeleteInput=Yes");
    let active = app.active_command;
    enter(&mut app, "Solid=No SplitAtTangents=No DeleteInput=bad");
    assert_eq!(app.active_command, active);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert!(app.surface_extrusion_sources.is_none());
    assert_eq!(app.document.object(s), Some(&before));
}
#[test]
fn surface_apex_changed_face_source_keeps_point_getter() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeSurfaceToPoint { .. })
    ));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_apex_portable_failure_retains_points_and_frozen_sources() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(app.active_command.is_some());
    assert!(app.surface_extrusion_sources.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_component_source_and_typed_point_commit_one_history_entry() {
    let mut app = test_app();
    let s = source(&mut app);
    let before = app.document.object(s).unwrap().clone();
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: s,
            kind: viboceros_command::ComponentSelectionKind::BrepFace,
            index: 0,
        }],
        preselection: true,
        modifiers: egui::Modifiers::default(),
    });
    enter(&mut app, "ExtrudeSrfToPoint Solid=Yes DeleteInput=Yes");
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_some());
    enter(&mut app, "1,1.5,6");
    assert!(app.active_command.is_none());
    assert!(app.document.object(s).is_none());
    let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(b.is_solid());
    assert!((b.signed_volume(app.document.tolerance()).unwrap().abs() - 11.5).abs() < 1e-5);
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(s), Some(&before));
    enter(&mut app, "Redo");
    assert!(app.document.object(s).is_none());
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_coplanar_failure_then_valid_clicked_target_uses_the_same_sources() {
    let mut app = test_app();
    let carrier = viboceros_geometry::NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(0., 3., 0.),
            point(2., 3., 0.),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let s = app
        .document
        .add_geometry(Geometry::NurbsSurface(carrier))
        .unwrap();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfToPoint");
    let before = format!("{:?}", app.document);
    assert!(!app.accept_drafting_point(point(1., 1., 0.)));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.active_command.is_some());
    assert!(app.surface_extrusion_sources.is_some());
    assert!(app.accept_drafting_point(point(1., 1.5, 6.)));
    assert!(app.active_command.is_none());
    assert!(app.document.object(s).is_some());
    assert_eq!(app.document.selected_object_count(), 1);
}

fn planar_apex_source(app: &mut VibocerosApp, z: f64) -> ObjectId {
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(0., 0., z),
        point(4., 0., z),
        point(4., 3., z),
        point(0., 3., z),
    ])
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    id
}

#[test]
fn surface_apex_viewport_uses_the_moving_apex_and_preserves_sources_until_acceptance() {
    use super::circle_preview::{screen_point, viewport_click, viewport_frame};

    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = planar_apex_source(&mut app, -2.);
    enter(&mut app, "ExtrudeSrfToPoint Solid=Yes");
    assert!(app.active_command.is_some());
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());

    let before = format!("{:?}", app.document);
    let mut apex = None;
    let mut old_preview = None;
    for target in [point(1., 1., 0.), point(2.25, 1.75, 0.)] {
        let pixel = screen_point(&app, target);
        let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
        assert_eq!(screen_point(&app, target), pixel);
        assert!(
            preview
                .iter()
                .any(|segment| segment[1].distance(pixel) < 1e-3)
        );
        if let Some(old) = old_preview {
            assert_ne!(preview, old);
        }
        old_preview = Some(preview);
        apex = Some((pixel, target));
    }
    assert_eq!(format!("{:?}", app.document), before);
    let (pixel, _target) = apex.unwrap();
    viewport_click(&mut app, &context, pixel);
    #[cfg(feature = "native-smlib")]
    {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(brep) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected accepted extrusion");
        };
        assert!(brep.is_solid());
        assert!(
            brep.vertices()
                .iter()
                .any(|vertex| vertex.point().distance_to(_target).unwrap() < 1e-5)
        );
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrfToPoint"));
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        assert!(app.active_command.is_some());
        assert_eq!(format!("{:?}", app.document), before);
    }
    assert!(app.document.object(source).is_some());
}

#[test]
fn surface_apex_bare_options_are_provisional_and_nonzero_scalars_keep_the_point_prompt() {
    let mut app = test_app();
    let source = planar_apex_source(&mut app, -2.);
    let defaults = app.commands.surface_apex_options();
    enter(&mut app, "ExtrudeSrfToPoint");
    enter(&mut app, "_Solid _DeleteInput _SplitAtTangents");
    let Some(InteractiveCommand::ExtrudeSurfaceToPoint { options }) = app.active_command else {
        panic!("point prompt left");
    };
    assert_eq!(options.solid, !defaults.solid);
    assert_eq!(options.delete_input, !defaults.delete_input);
    assert_eq!(options.split_at_tangents, !defaults.split_at_tangents);
    enter(&mut app, "6");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeSurfaceToPoint { options })
    );
    assert!(app.point_constraint.is_none());
    assert!(app.command_log.back().unwrap().contains("apex point"));
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.surface_apex_options(), defaults);
    assert!(app.document.is_selected(source));
    assert!(!app.document.can_undo());
}

#[test]
fn surface_apex_replays_the_hovered_apex_in_the_other_viewports() {
    use super::circle_preview::{screen_point, viewport_frame};

    let mut app = test_app();
    app.osnap = false;
    app.grid_snap = false;
    planar_apex_source(&mut app, -2.);
    enter(&mut app, "ExtrudeSrfToPoint");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    for view in [0, 2] {
        app.viewports[view]
            .zoom_extents(&app.document, Default::default())
            .unwrap();
    }
    viewport_frame(&mut app, &context, Vec::new());

    let apex = point(1., 1.5, 0.);
    let top = screen_point(&app, apex);
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(top)]);
    let lines = viewport_frame(&mut app, &context, Vec::new());
    let front = &app.viewports[2];
    let front_apex = front.project(apex, front.screen_rect().unwrap()).unwrap();
    assert!(lines.iter().any(|line| line[1].distance(front_apex) < 1e-3));

    app.cancel_current_prompt_or_selection();
    assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
    assert!(!app.document.can_undo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_apex_zero_uses_the_requested_origin_and_acceptance_remembers_options() {
    let mut app = test_app();
    let plane = viboceros_command::construction_plane::WorldPlane::Front
        .frame()
        .with_origin(point(10., 20., 30.));
    app.viewports[0].set_construction_plane(plane);

    for (token, expected) in [("0", plane.origin()), ("w0", point(0., 0., 0.))] {
        let source = planar_apex_source(&mut app, -2.);
        let old_ids = app
            .document
            .objects()
            .map(|o| o.id())
            .collect::<std::collections::BTreeSet<_>>();
        if token == "0" {
            enter(&mut app, "ExtrudeSrfToPoint");
            enter(&mut app, "Solid=No DeleteInput=Yes SplitAtTangents=Yes");
            enter(&mut app, "0");
        } else {
            enter(&mut app, "ExtrudeSrfToPoint w0");
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let product = app
            .document
            .objects()
            .find(|o| !old_ids.contains(&o.id()))
            .unwrap();
        let Geometry::Brep(brep) = product.geometry() else {
            panic!("expected extrusion");
        };
        assert!(!brep.is_solid());
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point().distance_to(expected).unwrap() < 1e-7)
        );
        assert!(app.document.object(source).is_none());
        let saved = app.commands.surface_apex_options();
        assert!(!saved.solid && saved.delete_input && saved.split_at_tangents);
    }
}

fn boundary_surface(app: &mut VibocerosApp, z: f64) -> ObjectId {
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(-3., -3., z),
        point(6., -3., z),
        point(6., 6., z),
        point(-3., 6., z),
    ])
    .unwrap();
    app.document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap()
}

#[test]
fn surface_apex_boundary_getter_picks_a_face_and_previews_only_to_the_boundary() {
    use super::circle_preview::{screen_point, viewport_click, viewport_frame};

    let mut app = test_app();
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = planar_apex_source(&mut app, 0.);
    let boundary = boundary_surface(&mut app, 3.);
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let location = screen_point(&app, point(5., 2., 3.));
    viewport_click(&mut app, &context, location);
    assert!(
        !app.picking_surface_apex_boundary(),
        "{:?}",
        app.command_log
    );
    assert!(app.command_hint().contains("beyond the boundary"));
    assert_eq!(format!("{:?}", app.document), before);

    app.viewports[0] = Viewport::new(ViewKind::Front);
    let plane = app.viewports[0]
        .construction_plane()
        .with_origin(point(0., 1.5, 0.));
    app.viewports[0].set_construction_plane(plane);
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let apex = screen_point(&app, point(2., 1.5, 6.));
    let rim = screen_point(&app, point(2., 1.5, 3.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(apex)]);
    assert!(preview.iter().any(|p| (p[1].y - rim.y).abs() < 1e-3));
    assert!(preview.iter().flatten().all(|p| p.y >= rim.y - 1e-3));
    assert_eq!(format!("{:?}", app.document), before);
    viewport_click(&mut app, &context, apex);
    #[cfg(feature = "native-smlib")]
    {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected truncated extrusion");
        };
        assert!(result.is_solid());
        assert_eq!(result.faces().len(), 6);
        assert!((result.signed_volume(app.document.tolerance()).unwrap() - 21.).abs() < 1e-6);
        let bounds = result.tight_bounds(app.document.tolerance()).unwrap();
        assert!((bounds.max().z() - 3.).abs() < 1e-6);
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrfToPoint"));
        enter(&mut app, "Undo");
    }
    #[cfg(not(feature = "native-smlib"))]
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.object(source).is_some());
    assert!(app.document.object(boundary).is_some());
}

#[test]
fn surface_apex_boundary_selection_and_failed_apex_keep_model_selection_private() {
    let mut app = test_app();
    let source = planar_apex_source(&mut app, 0.);
    let boundary = boundary_surface(&mut app, 3.);
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint");
    enter(&mut app, "ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    enter(&mut app, &format!("_SelID {boundary}"));
    assert!(app.command_hint().contains("beyond the boundary"));
    enter(&mut app, "w2,1.5,1");
    assert!(app.active_command.is_some());
    assert!(app.command_hint().contains("beyond the boundary"));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.is_selected(source));
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
}

#[test]
fn surface_apex_boundary_menu_enter_cancels_without_a_surface_pick() {
    let mut app = test_app();
    planar_apex_source(&mut app, 0.);
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}
