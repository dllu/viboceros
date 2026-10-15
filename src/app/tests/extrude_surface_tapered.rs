use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                2,
                2,
                3,
                3,
                (0..3)
                    .flat_map(|j| {
                        (0..3).map(move |i| {
                            point(
                                i as f64 * 3. - 3.,
                                j as f64 * 3. - 3.,
                                if i == 1 && j == 1 { 1. } else { 0. },
                            )
                        })
                    })
                    .collect(),
                vec![0., 0., 0., 1., 1., 1.],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn surface_taper_source_picker_options_flip_and_cancel() {
    let mut app = test_app();
    let s = source(&mut app);
    let original = app.document.object(s).unwrap().clone();
    enter(&mut app, "ExtrudeSrfTapered DraftAngle=10 Solid=No");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(s),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Some(InteractiveCommand::ExtrudeSurfaceTapered { options, .. }) = app.active_command else {
        panic!()
    };
    assert_eq!(options.draft_angle, 10.);
    assert!(!options.solid);
    enter(&mut app, "Solid=Yes DeleteInput=Yes FlipAngle");
    let active = app.active_command;
    let Some(InteractiveCommand::ExtrudeSurfaceTapered { options, .. }) = active else {
        panic!()
    };
    assert_eq!(options.draft_angle, -10.);
    enter(&mut app, "Solid=No DraftAngle=90");
    assert_eq!(app.active_command, active);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("ExtrudeSrfTapered")
    );
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert!(app.surface_extrusion_sources.is_none());
    assert_eq!(app.document.object(s), Some(&original));
}
#[test]
fn surface_taper_changed_source_preserves_the_getter() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfTapered");
    app.document
        .replace_object_geometries([(s, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "2");
    assert!(app.active_command.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.back().unwrap().contains("source changed"));
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn surface_taper_portable_decline_retains_frozen_faces() {
    let mut app = test_app();
    let s = source(&mut app);
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfTapered");
    let before = format!("{:?}", app.document);
    enter(&mut app, "2");
    assert!(app.active_command.is_some());
    assert!(app.surface_extrusion_sources.is_some());
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_numeric_and_two_point_input_commit_from_component_sources() {
    for numeric in [true, false] {
        let mut app = test_app();
        let s = source(&mut app);
        let original = app.document.object(s).unwrap().clone();
        app.accept_component_click(crate::viewport::ComponentClick {
            picks: vec![crate::viewport::ComponentPick {
                object: s,
                kind: viboceros_command::ComponentSelectionKind::BrepFace,
                index: 0,
            }],
            preselection: true,
            modifiers: egui::Modifiers::default(),
        });
        enter(&mut app, "ExtrudeSrfTapered DraftAngle=10 DeleteInput=Yes");
        assert!(app.object_prompt.is_none());
        if numeric {
            enter(&mut app, "2");
        } else {
            assert!(app.accept_drafting_point(point(10., 20., 30.)));
            assert!(app.accept_drafting_point(point(10., 20., 32.)));
        }
        assert!(app.active_command.is_none());
        assert!(app.document.object(s).is_none());
        let Geometry::Brep(b) = app.document.selected_objects().next().unwrap().geometry() else {
            panic!()
        };
        assert!(b.is_solid());
        let Geometry::NurbsSurface(surface) = original.geometry() else {
            panic!()
        };
        for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.8, 0.7)] {
            let expected = surface.evaluate(u, v).unwrap();
            assert!(b.faces().iter().any(|f| {
                f.surface()
                    .closest_parameters(expected, app.document.tolerance())
                    .is_ok_and(|(a, c)| {
                        f.surface()
                            .evaluate(a, c)
                            .unwrap()
                            .distance_to(expected)
                            .unwrap()
                            < 1e-6
                    })
            }));
        }
        let k = 10_f64.to_radians().tan();
        assert!(
            (b.signed_volume(app.document.tolerance()).unwrap().abs()
                - (68. - 48. * k + 32. / 3. * k * k))
                .abs()
                < 1e-4
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(s), Some(&original));
        enter(&mut app, "Redo");
        assert!(app.document.object(s).is_none());
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_taper_failed_direction_can_be_repaired_with_the_same_source_getter() {
    let mut app = test_app();
    let s = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 0.),
                    point(0., 6., 0.),
                    point(0., 0., 6.),
                    point(0., 6., 6.),
                ],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([s], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeSrfTapered DraftAngle=10 Direction=0,0,1");
    let before = format!("{:?}", app.document);
    enter(&mut app, "2");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeSurfaceTapered { base: None, .. })
    ));
    enter(&mut app, "Direction");
    enter(&mut app, "Direction=1,0,0");
    enter(&mut app, "2");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 1);
}
