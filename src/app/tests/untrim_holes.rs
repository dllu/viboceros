use super::*;
use crate::viewport::EdgePick;
use viboceros_command::UntrimHolesOptions;
use viboceros_geometry::Vector3;

fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture(app: &mut VibocerosApp, x: f64) -> EdgePick {
    let brep = viboceros_geometry::Brep::try_tube(
        viboceros_command::CommandContext::default().construction_plane,
        [2., 5.],
        8.,
        app.document.tolerance(),
    )
    .unwrap();
    let geometry = Geometry::Brep(brep)
        .transformed(
            viboceros_geometry::AffineTransform3::from_translation(
                Vector3::try_new(x, 0., 0.).unwrap(),
            ),
            app.document.tolerance(),
        )
        .unwrap();
    EdgePick {
        object: app.document.add_geometry(geometry).unwrap(),
        edge: 3,
    }
}
fn objects(app: &VibocerosApp) -> Vec<viboceros_document::Object> {
    app.document.objects().cloned().collect()
}
fn click(app: &mut VibocerosApp, pick: EdgePick) {
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_click: Some(vec![pick]),
        ..Default::default()
    }));
}

#[test]
fn picks_apply_immediately_local_undo_discards_last_pick_and_enter_or_escape_keeps_one_external_record()
 {
    for escape in [false, true] {
        let mut app = test_app();
        let first = fixture(&mut app, 0.);
        let second = fixture(&mut app, 20.);
        let group = app.document.add_group(None, [first.object]).unwrap();
        app.document.clear_history().unwrap();
        let before = objects(&app);
        submit(&mut app, "_UntrimHoles KeepTrimObjects=Yes");
        assert!(app.viewport_object_filter().is_none());
        assert!(app.hole_prompt.as_ref().unwrap().picking_edges());
        click(&mut app, first);
        let after_first = objects(&app);
        assert_ne!(after_first, before);
        assert_eq!(
            app.document.object(first.object).unwrap().group_ids(),
            &[group]
        );
        click(&mut app, second);
        assert_eq!(app.document.objects().len(), 4);
        submit(&mut app, "_Undo");
        assert_eq!(objects(&app), after_first);
        assert!(!app.document.can_redo());
        click(&mut app, second);
        let after = objects(&app);
        if escape {
            app.cancel_current_prompt_or_selection();
        } else {
            submit(&mut app, "");
        }
        assert!(app.hole_prompt.is_none());
        assert_eq!(objects(&app), after);
        submit(&mut app, "Undo");
        assert_eq!(objects(&app), before);
        assert!(!app.document.can_undo());
        submit(&mut app, "Redo");
        assert_eq!(objects(&app), after);
    }
}

#[test]
fn face_picks_and_option_subprompts_share_command_memory_outside_undo_and_reject_bad_values_atomically()
 {
    let mut app = test_app();
    let pick = fixture(&mut app, 0.);
    let before = objects(&app);
    submit(&mut app, "UntrimHoles");
    submit(&mut app, "All");
    assert!(!app.hole_prompt.as_ref().unwrap().picking_edges());
    submit(&mut app, "_Yes");
    assert!(app.hole_prompt.as_ref().unwrap().picking_faces());
    submit(&mut app, "MaximumEdgeLength");
    for invalid in ["-1", "NaN", "inf", "garbage"] {
        submit(&mut app, invalid);
    }
    assert_eq!(
        app.hole_prompt
            .as_ref()
            .unwrap()
            .options
            .maximum_edge_length,
        0.
    );
    submit(&mut app, "1");
    app.accept_component_face_hit(pick.object, 2, None);
    assert_eq!(objects(&app), before);
    let options_before = app.hole_prompt.as_ref().unwrap().options;
    submit(&mut app, "All=No KeepTrimObjects=Yes MaximumEdgeLength=-1");
    assert_eq!(app.hole_prompt.as_ref().unwrap().options, options_before);
    submit(&mut app, "MaximumEdgeLength=0 KeepTrimObjects=Yes");
    app.handle_viewport_action(ViewportOutput {
        face_click: Some((pick.object, 2)),
        ..Default::default()
    });
    assert_eq!(app.document.objects().len(), 2);
    submit(&mut app, "");
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), before);
    submit(&mut app, "UntrimHoles");
    assert_eq!(
        app.hole_prompt.as_ref().unwrap().options,
        UntrimHolesOptions {
            all: true,
            maximum_edge_length: 0.,
            keep_trim_objects: true
        }
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
}

#[test]
fn geometry_commands_finish_the_group_and_view_and_cplane_commands_preserve_it() {
    let mut app = test_app();
    let pick = fixture(&mut app, 0.);
    submit(&mut app, "UntrimHoles KeepTrimObjects=Yes");
    click(&mut app, pick);
    let after = objects(&app);
    submit(&mut app, "SetView World Top");
    assert!(app.hole_prompt.is_some());
    submit(&mut app, "CPlane");
    assert!(app.hole_prompt.is_some());
    submit(&mut app, "w0,0,7");
    assert!(app.hole_prompt.is_some());
    assert_eq!(objects(&app), after);
    submit(&mut app, "Point 10,20,30");
    assert!(app.hole_prompt.is_none());
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), after);
    submit(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn noops_ambiguity_and_stale_sources_cannot_modify_foreign_history_or_lose_redo() {
    let mut app = test_app();
    let pick = fixture(&mut app, 0.);
    submit(&mut app, "Point 1,2,3");
    submit(&mut app, "Undo");
    let before = format!("{:?}", app.document);
    submit(&mut app, "UntrimHoles MaximumEdgeLength=1");
    click(&mut app, pick);
    submit(&mut app, "Undo");
    assert_eq!(format!("{:?}", app.document), before);
    app.accept_hole_edges(vec![pick, EdgePick { edge: 0, ..pick }]);
    submit(&mut app, "99");
    assert_eq!(format!("{:?}", app.document), before);
    // A sidebar mutation occurs outside this command's group.
    app.document
        .set_objects_locked([pick.object], true)
        .unwrap();
    let changed = format!("{:?}", app.document);
    submit(&mut app, "1");
    assert_eq!(format!("{:?}", app.document), changed);
    assert!(app.hole_prompt.is_none());
    submit(&mut app, "UntrimHoles");
    app.document
        .set_objects_locked([pick.object], false)
        .unwrap();
    let changed = format!("{:?}", app.document);
    submit(&mut app, "Undo");
    assert!(app.hole_prompt.is_none());
    assert_eq!(format!("{:?}", app.document), changed);
}

#[test]
fn option_buttons_render_and_typed_component_arguments_continue_to_use_the_registry() {
    let mut app = test_app();
    let pick = fixture(&mut app, 0.);
    submit(&mut app, "UntrimHoles");
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show_hole_choices(ui),
        )
    };
    let output = frame(&mut app, vec![]);
    let button = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "KeepTrimObjects=No"
            {
                Some(text.pos + text.galley.rect.center().to_vec2())
            } else {
                None
            }
        })
        .unwrap();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(button),
                egui::Event::PointerButton {
                    pos: button,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    assert!(app.hole_prompt.as_ref().unwrap().options.keep_trim_objects);
    submit(&mut app, "");
    submit(
        &mut app,
        &format!("UntrimHoles {} 3 KeepTrimObjects=Yes", pick.object),
    );
    assert!(app.hole_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
}
