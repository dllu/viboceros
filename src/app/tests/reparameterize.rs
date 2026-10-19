use super::*;
use crate::viewport::{ComponentClick, ComponentPick, SelectionChoice, SelectionClick};
use viboceros_command::{ComponentSelectionKind, ObjectSelectionFilter};
use viboceros_document::SelectionMode;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn curve_app() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 3,4,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    (app, id)
}

#[test]
fn curve_pre_and_postselection_stage_numbers_and_commit_one_edit() {
    for preselected in [false, true] {
        let (mut app, id) = curve_app();
        if preselected {
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
        }
        app.document
            .set_object_geometry_user_text([id], "Code", Some("kept"))
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.object(id).unwrap().clone();
        enter(&mut app, "Reparameterize");
        assert!(!app.command_line_idle());
        assert!(!app.component_preselection_available());
        if !preselected {
            assert_eq!(
                app.viewport_object_filter(),
                Some(ObjectSelectionFilter::Parametric)
            );
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        assert!(!app.picking_reparameterize());
        assert!(app.viewport_object_filter().is_none());
        enter(&mut app, "-2");
        assert_eq!(app.document.object(id).unwrap(), &before);
        assert!(!app.document.can_undo());
        enter(&mut app, "8");
        assert!(app.reparameterize_prompt.is_none());
        let after = app.document.object(id).unwrap().clone();
        assert_eq!(after.geometry().curve_ref().unwrap().domain(), -2. ..=8.);
        assert_eq!(after.attributes(), before.attributes());
        assert_eq!(after.geometry_user_text(), before.geometry_user_text());
        assert_eq!(app.document.is_selected(id), preselected);
        enter(&mut app, "Undo");
        assert_eq!(app.document.object(id).unwrap(), &before);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(id).unwrap(), &after);
    }
}

#[test]
fn queued_automatic_and_partial_initial_values_use_the_picked_curve() {
    let (mut app, id) = curve_app();
    app.execute_command(&format!("Reparameterize Object={id} -2,8"));
    app.document.clear_history().unwrap();
    enter(&mut app, "Reparameterize Automatic");
    assert!(app.picking_reparameterize());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    assert!(app.reparameterize_prompt.is_none());
    assert_eq!(
        app.document
            .object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain(),
        0. ..=5.
    );
    assert_eq!(app.document.undo_label(), Some("Reparameterize"));
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Reparameterize -4");
    assert!(
        app.reparameterize_prompt
            .as_ref()
            .unwrap()
            .hint()
            .starts_with("Domain end")
    );
    enter(&mut app, "6");
    assert!(app.reparameterize_prompt.is_none());
    assert_eq!(
        app.document
            .object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain(),
        -4. ..=6.
    );
}

#[test]
fn defaults_invalid_values_escape_and_replacement_preserve_redo() {
    let (mut app, id) = curve_app();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.execute_command("SetObjectName Future");
    app.execute_command("Undo");
    let before = app.document.object(id).unwrap().clone();
    enter(&mut app, "Reparameterize");
    for _ in 0..2 {
        enter(&mut app, "");
    }
    assert!(app.reparameterize_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap(), &before);
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    enter(&mut app, "Reparameterize");
    enter(&mut app, "20");
    let hint = app
        .reparameterize_prompt
        .as_ref()
        .unwrap()
        .hint()
        .to_owned();
    for bad in ["", "20", "-5", "NaN", "inf", "1,2", "Automatic"] {
        enter(&mut app, bad);
        assert_eq!(app.reparameterize_prompt.as_ref().unwrap().hint(), hint);
        assert_eq!(app.document.object(id).unwrap(), &before);
        assert!(app.document.can_redo());
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.reparameterize_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap(), &before);
    assert!(app.document.can_redo());
    enter(&mut app, "Reparameterize");
    enter(&mut app, "Domain");
    assert!(app.reparameterize_prompt.is_none());
    assert!(app.command_log.back().unwrap().contains("Curve domain"));
    assert!(app.document.can_redo());
    enter(&mut app, "Reparameterize");
    enter(&mut app, "-2");
    enter(&mut app, "Cancel");
    assert!(app.reparameterize_prompt.is_none());
    assert!(app.document.can_redo());
}

#[test]
fn stale_geometry_visibility_and_tolerance_end_the_getter() {
    for change in ["geometry", "visibility", "tolerance"] {
        let (mut app, id) = curve_app();
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "Reparameterize");
        enter(&mut app, "-2");
        match change {
            "geometry" => app.execute_command("Move 0,0,0 1,0,0"),
            "visibility" => app.execute_command("Hide"),
            _ => {
                app.document
                    .set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-8).unwrap());
            }
        }
        let objects = app.document.objects().cloned().collect::<Vec<_>>();
        let label = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "8");
        assert!(app.reparameterize_prompt.is_none(), "{change}");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), label.as_deref());
    }
}

#[test]
fn curve_over_surface_uses_overlap_choice_before_the_face_hit() {
    let (mut app, curve) = curve_app();
    app.execute_command("SrfPt -1,-1,0 6,-1,0 6,6,0 -1,6,0");
    let surface = app
        .document
        .objects()
        .find(|o| o.id() != curve)
        .unwrap()
        .id();
    let before = app.document.object(surface).unwrap().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Reparameterize");
    app.handle_viewport_action(ViewportOutput {
        face_click: Some((surface, 0)),
        selection_choice: Some(SelectionChoice {
            object_ids: vec![curve, surface],
            mode: SelectionMode::Replace,
            pointer: egui::pos2(100., 100.),
            viewport: 0,
        }),
        ..Default::default()
    });
    assert!(app.picking_reparameterize());
    assert!(app.selection_menu.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(curve),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "-2 8");
    assert_eq!(
        app.document
            .object(curve)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain(),
        -2. ..=8.
    );
    assert_eq!(app.document.object(surface).unwrap(), &before);
}

#[test]
fn actual_viewport_click_picks_the_front_face_without_selecting_the_whole_object() {
    use crate::viewport::{FacePickMode, ViewKind, Viewport, ViewportInput};
    use viboceros_geometry::{Brep, NurbsSurface};
    fn frame(
        app: &VibocerosApp,
        view: &mut Viewport,
        ctx: &egui::Context,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> (ViewportOutput, egui::Rect) {
        let mut output = ViewportOutput::default();
        let mut area = egui::Rect::NOTHING;
        *time += 0.25;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900., 700.),
                )),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ui| {
                area = ui.available_rect_before_wrap();
                let allowed = app
                    .reparameterize_prompt
                    .as_ref()
                    .filter(|p| p.selecting())
                    .and_then(|p| p.source_id())
                    .map(|id| [id].into_iter().collect::<std::collections::BTreeSet<_>>());
                output = view.show(
                    ui,
                    &app.document,
                    ViewportInput {
                        object_filter: app.viewport_object_filter(),
                        selection_allowed_objects: allowed.as_ref(),
                        face_pick: app
                            .picking_reparameterize()
                            .then_some(FacePickMode::SurfaceAndBrepAny),
                        component_preselection: app.component_preselection_available(),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
        (output, area)
    }
    for constrained in [false, true] {
        let mut app = test_app();
        let tolerance = app.document.tolerance();
        let parts = [0., 10.].map(|z| {
            Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    point(0., 0., z),
                    point(4., 0., z),
                    point(4., 6., z),
                    point(0., 6., z),
                ])
                .unwrap(),
                tolerance,
            )
            .unwrap()
        });
        let brep = Brep::try_combine(parts.to_vec(), tolerance).unwrap();
        let id = app
            .document
            .add_geometry(Geometry::Brep(brep.clone()))
            .unwrap();
        if constrained {
            let front = Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    point(0., 0., 20.),
                    point(4., 0., 20.),
                    point(4., 6., 20.),
                    point(0., 6., 20.),
                ])
                .unwrap(),
                tolerance,
            )
            .unwrap();
            app.document.add_geometry(Geometry::Brep(front)).unwrap();
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
        }
        let others = app
            .document
            .objects()
            .filter(|o| o.id() != id)
            .cloned()
            .collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let before = app.document.object(id).unwrap().clone();
        enter(&mut app, "Reparameterize");
        let mut view = Viewport::new(ViewKind::Top);
        let context = egui::Context::default();
        let mut time = 0.;
        frame(&app, &mut view, &context, &mut time, vec![]);
        view.zoom_extents(&app.document, Default::default())
            .unwrap();
        let (_, area) = frame(&app, &mut view, &context, &mut time, vec![]);
        let pixel = view.project(point(2., 3., 10.), area).unwrap();
        frame(
            &app,
            &mut view,
            &context,
            &mut time,
            vec![
                egui::Event::PointerMoved(pixel),
                egui::Event::PointerButton {
                    pos: pixel,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let (output, _) = frame(
            &app,
            &mut view,
            &context,
            &mut time,
            vec![egui::Event::PointerButton {
                pos: pixel,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(output.face_click, Some((id, 1)));
        assert!(app.handle_viewport_action(output));
        assert!(!app.picking_reparameterize());
        assert_eq!(
            app.reparameterize_prompt
                .as_ref()
                .unwrap()
                .highlight()
                .unwrap()
                .index,
            1
        );
        assert_eq!(app.document.object(id).unwrap(), &before);
        assert_eq!(app.document.is_selected(id), constrained);
        enter(&mut app, "-2 8 10 30");
        let Geometry::Brep(actual) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(actual.faces()[0], brep.faces()[0]);
        assert_eq!(actual.faces()[1].surface().domain_u(), -2. ..=8.);
        assert_eq!(actual.faces()[1].surface().domain_v(), 10. ..=30.);
        assert_eq!(
            app.document
                .objects()
                .filter(|o| o.id() != id)
                .cloned()
                .collect::<Vec<_>>(),
            others
        );
    }
}

#[test]
fn recorded_surface_getters_preserve_the_command_results_and_history() {
    use serde_json::Value;
    use viboceros_io::ThreeDmGeometry;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cases = 0;
    for (capture, folder, epsilon) in [
        ("reparameterize_faces", "reparameterize_faces", 1e-8),
        (
            "reparameterize_automatic_main",
            "reparameterize_automatic",
            1e-8,
        ),
        (
            "reparameterize_automatic_tight",
            "reparameterize_automatic",
            1e-12,
        ),
        (
            "reparameterize_automatic_loose",
            "reparameterize_automatic",
            1e-5,
        ),
    ] {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join(format!("tools/rhino_oracle/observations/{capture}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in data["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let face = row["value"]["face"].as_u64().unwrap() as usize;
            let tolerance = Tolerance::try_new(epsilon, 1e-12, 1e-8).unwrap();
            let model = viboceros_io::read_3dm_file(
                root.join(format!("tools/rhino_oracle/fixtures/{folder}/{name}.3dm")),
                tolerance,
            )
            .unwrap();
            let ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
                panic!()
            };
            let mut app = test_app();
            app.document = Document::new(tolerance);
            let id = app
                .document
                .add_geometry(Geometry::Brep(brep.clone()))
                .unwrap();
            app.document
                .set_object_geometry_user_text([id], "Code", Some("retained"))
                .unwrap();
            app.document.clear_history().unwrap();
            let before = app.document.object(id).unwrap().clone();
            let automatic = name.ends_with("automatic");
            let values = if automatic {
                vec!["Automatic".into()]
            } else {
                row["value"]["domains"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|axis| axis.as_array().unwrap())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            };
            let mut control = app.document.clone();
            app.commands
                .execute(
                    &mut control,
                    &format!(
                        "Reparameterize Object={id} Face={face} {}",
                        values.join(" ")
                    ),
                )
                .unwrap();
            if cases % 2 == 0 {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: id,
                        kind: ComponentSelectionKind::BrepFace,
                        index: face,
                    }],
                    preselection: true,
                    modifiers: Default::default(),
                });
                enter(&mut app, "Reparameterize");
            } else {
                enter(&mut app, "Reparameterize");
                app.handle_viewport_action(ViewportOutput {
                    face_click: Some((id, face)),
                    selection_click: Some(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    }),
                    ..Default::default()
                });
            }
            assert!(!app.picking_reparameterize(), "{name}");
            for (i, value) in values.iter().enumerate() {
                assert_eq!(
                    app.document.object(id).unwrap(),
                    &before,
                    "{name}: partial {i}"
                );
                assert!(!app.document.can_undo(), "{name}: partial history");
                enter(&mut app, value);
            }
            assert!(
                app.reparameterize_prompt.is_none(),
                "{name}: {:?}",
                app.command_log
            );
            assert_eq!(app.document.object(id), control.object(id), "{name}");
            let after = app.document.object(id).unwrap().clone();
            if before != after {
                enter(&mut app, "Undo");
                assert_eq!(app.document.object(id).unwrap(), &before, "{name}: undo");
                assert!(!app.document.can_undo(), "{name}: one step");
                enter(&mut app, "Redo");
                assert_eq!(app.document.object(id).unwrap(), &after, "{name}: redo");
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 54);
}

#[test]
fn enter_defaults_and_mixed_values_replay_five_actual_rhino_getters() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reparameterize_input.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    assert_eq!(capture["results"].as_array().unwrap().len(), 5);
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        assert_eq!(v["success"], true);
        let face = v["face"].as_u64().unwrap() as usize;
        let model = viboceros_io::read_3dm_file(
            root.join(format!(
                "tools/rhino_oracle/fixtures/reparameterize_input/{name}.3dm"
            )),
            Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap(),
        )
        .unwrap();
        let viboceros_io::ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
            panic!()
        };
        let mut app = test_app();
        app.document = Document::new(Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap());
        let id = app
            .document
            .add_geometry(Geometry::Brep(brep.clone()))
            .unwrap();
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        registry
            .execute(&mut app.document, "SetObjectName Future")
            .unwrap();
        registry.execute(&mut app.document, "Undo").unwrap();
        let before = app.document.object(id).unwrap().clone();
        assert!(app.document.can_redo());
        enter(&mut app, "Reparameterize");
        if brep.faces().len() > 1 {
            assert!(app.picking_reparameterize());
            assert!(
                app.reparameterize_prompt
                    .as_ref()
                    .unwrap()
                    .hint()
                    .contains("polysurface")
            );
            app.accept_component_face_hit(id, face, None);
        }
        for input in v["inputs"].as_array().unwrap() {
            assert_eq!(
                app.document.object(id).unwrap(),
                &before,
                "{name}: staged input"
            );
            assert!(app.document.can_redo(), "{name}: staged redo");
            let text = input.as_str().unwrap();
            enter(&mut app, if text == "_Enter" { "" } else { text });
        }
        assert!(app.reparameterize_prompt.is_none(), "{name}");
        let after = app.document.object(id).unwrap().clone();
        let Geometry::Brep(actual) = after.geometry() else {
            panic!()
        };
        let surface = actual.faces()[face].surface();
        for (axis, domain) in [surface.domain_u(), surface.domain_v()].iter().enumerate() {
            assert_eq!(
                *domain.start(),
                v["domains"][axis][0].as_f64().unwrap(),
                "{name}"
            );
            assert_eq!(
                *domain.end(),
                v["domains"][axis][1].as_f64().unwrap(),
                "{name}"
            );
        }
        let expected = &v["after"]["geometry"]["geometry"]["faces"][face];
        for (i, reference) in expected["samples"].as_array().unwrap().iter().enumerate() {
            let point = surface
                .evaluate(
                    surface.parameter_at_u((i % 9) as f64 / 8.).unwrap(),
                    surface.parameter_at_v((i / 9) as f64 / 8.).unwrap(),
                )
                .unwrap();
            let reference =
                Point3::try_from(serde_json::from_value::<[f64; 3]>(reference.clone()).unwrap())
                    .unwrap();
            assert!(
                point.distance_to(reference).unwrap() <= 1e-8,
                "{name}: image {i}"
            );
        }
        assert_eq!(actual.edges(), brep.edges());
        assert_eq!(actual.vertices(), brep.vertices());
        for (i, (a, b)) in actual.faces().iter().zip(brep.faces()).enumerate() {
            if i != face {
                assert_eq!(a, b);
            }
        }
        if before.geometry() == after.geometry() {
            assert!(!app.document.can_undo(), "{name}: no-op");
            assert!(app.document.can_redo(), "{name}: no-op redo");
        } else {
            enter(&mut app, "Undo");
            assert_eq!(app.document.object(id).unwrap(), &before, "{name}: undo");
            assert!(!app.document.can_undo());
            enter(&mut app, "Redo");
            assert_eq!(app.document.object(id).unwrap(), &after, "{name}: redo");
        }
    }
}
