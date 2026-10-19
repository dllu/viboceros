use super::*;
use crate::viewport::{DisplayMode, ParametricPick, ViewKind, Viewport, ViewportInput};

fn open(name: &str) -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tools/rhino_oracle/fixtures/reparameterize_blocks/{name}.3dm"
    ));
    app.execute_command(&format!("Open \"{}\"", path.display()));
    let root = app
        .document
        .objects()
        .find(|o| o.attributes().name() == Some("Root0"))
        .unwrap()
        .id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    (app, root)
}
fn frame(
    app: &VibocerosApp,
    view: &mut Viewport,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    *time += 0.2;
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
            rect = ui.available_rect_before_wrap();
            let allowed = app
                .reparameterize_prompt
                .as_ref()
                .and_then(|p| p.source_id())
                .map(|id| [id].into_iter().collect());
            let highlights = app
                .reparameterize_prompt
                .as_ref()
                .map(|p| p.highlights())
                .unwrap_or_default();
            output = view.show(
                ui,
                &app.document,
                ViewportInput {
                    parametric_pick: app.picking_reparameterize(),
                    parametric_highlights: &highlights,
                    selection_allowed_objects: allowed.as_ref(),
                    ..Default::default()
                },
                &[],
                0,
                true,
            );
        },
    )
    .drop_without_applying_deltas();
    (output, rect)
}
fn click(app: &VibocerosApp, view: &mut Viewport, point: Point3) -> ViewportOutput {
    let ctx = egui::Context::default();
    let mut time = 0.;
    let (_, rect) = frame(app, view, &ctx, &mut time, vec![]);
    // The independent repeated root lies far away; frame the selected source
    // with the public camera operation rather than zooming to the document.
    view.zoom_target_from_point(
        point,
        Point3::try_new(point.x() + 8., point.y() + 8., point.z()).unwrap(),
    )
    .unwrap();
    let pixel = view.project(point, rect).unwrap();
    frame(
        app,
        view,
        &ctx,
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
    frame(
        app,
        view,
        &ctx,
        &mut time,
        vec![egui::Event::PointerButton {
            pos: pixel,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .0
}
#[test]
fn placed_curve_mouse_pick_stages_an_independent_copy_and_one_history_step() {
    for preselected in [false, true] {
        let (mut app, root) = open("line_explicit");
        if preselected {
            app.document
                .select_object(root, SelectionMode::Replace)
                .unwrap();
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let definitions = app
            .document
            .block_definitions()
            .cloned()
            .collect::<Vec<_>>();
        enter(&mut app, "Reparameterize");
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = DisplayMode::Shaded;
        let output = click(&app, &mut view, point(1.5, 2., 0.));
        assert_eq!(
            output.parametric_click,
            Some(vec![ParametricPick {
                object: root,
                member: Some(0),
                face: None
            }])
        );
        assert!(output.selection_click.is_none());
        app.handle_viewport_action(output);
        assert!(app.picking_reparameterize());
        assert_eq!(
            app.reparameterize_prompt.as_ref().unwrap().highlights(),
            vec![ParametricPick {
                object: root,
                member: Some(0),
                face: None
            }]
        );
        enter(&mut app, "");
        enter(&mut app, "-2");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "8");
        assert!(app.reparameterize_prompt.is_none());
        for source in &before {
            assert_eq!(app.document.object(source.id()), Some(source));
        }
        assert_eq!(
            app.document
                .block_definitions()
                .cloned()
                .collect::<Vec<_>>(),
            definitions
        );
        let copy = app
            .document
            .objects()
            .find(|o| !before.iter().any(|s| s.id() == o.id()))
            .unwrap()
            .clone();
        assert_eq!(copy.geometry().curve_ref().unwrap().domain(), -2. ..=8.);
        assert_eq!(
            copy.attributes().layer_id(),
            app.document.current_layer_id()
        );
        assert!(copy.attributes().name().is_none());
        assert!(copy.attributes().user_text().is_empty());
        assert!(copy.group_ids().is_empty());
        assert!(!app.document.is_selected(copy.id()));
        assert_eq!(
            copy.geometry_user_text()
                .get("GeometryCode")
                .map(String::as_str),
            Some("prototype")
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert_eq!(app.document.object(copy.id()), Some(&copy));
    }
}
#[test]
fn block_face_overlap_menu_retains_both_faces_and_escape_keeps_selection_active() {
    let (mut app, root) = open("box_wire_first");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Reparameterize");
    let mut view = Viewport::new(ViewKind::Top);
    let output = click(&app, &mut view, point(-1., 5., 2.));
    let picks = output.parametric_click.clone().unwrap();
    assert_eq!(picks.len(), 2);
    assert!(
        picks
            .iter()
            .all(|p| p.object == root && p.member == Some(0))
    );
    assert_ne!(picks[0].face, picks[1].face);
    app.handle_viewport_action(output);
    assert!(app.reparameterize_prompt.as_ref().unwrap().has_menu());
    assert!(
        app.reparameterize_prompt
            .as_ref()
            .unwrap()
            .highlights()
            .contains(&picks[0])
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.picking_reparameterize());
    assert!(!app.reparameterize_prompt.as_ref().unwrap().has_menu());
    app.handle_viewport_action(ViewportOutput {
        parametric_click: Some(picks.clone()),
        selection_pointer: Some(egui::pos2(100., 100.)),
        ..Default::default()
    });
    let ctx = egui::Context::default();
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ui| {
            app.show_reparameterize_menu(ui, &[]);
        },
    )
    .drop_without_applying_deltas();
    assert!(
        app.reparameterize_prompt
            .as_ref()
            .unwrap()
            .highlights()
            .contains(&picks[1])
    );
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ui| {
            assert!(app.show_reparameterize_menu(ui, &[]));
        },
    )
    .drop_without_applying_deltas();
    assert!(app.picking_reparameterize());
    enter(&mut app, "");
    enter(&mut app, "-2 8 10 30");
    let copy = app
        .document
        .objects()
        .find(|o| !before.iter().any(|s| s.id() == o.id()))
        .unwrap();
    let Geometry::Brep(brep) = copy.geometry() else {
        panic!()
    };
    assert_eq!(brep.faces().len(), 6);
    assert_eq!(
        brep.faces()[picks[1].face.unwrap()].surface().domain_u(),
        -2. ..=8.
    );
    for source in &before {
        assert_eq!(app.document.object(source.id()), Some(source));
    }
}

#[test]
fn native_multiple_face_defaults_are_omitted_when_any_chart_value_differs() {
    let (mut app, root) = open("same_leaf_faces");
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/reparameterize_blocks.json"
    ))
    .unwrap();
    let case = capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "same_leaf_faces")
        .unwrap();
    assert!(
        case["history"]
            .as_str()
            .unwrap()
            .contains("U domain start ( Automatic )")
    );
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Reparameterize");
    for output in case["outputs"].as_array().unwrap() {
        let face = output["geometry"]["value"]["faces"]
            .as_array()
            .unwrap()
            .iter()
            .position(|f| f["definition"]["domain_u"][0] == -2.)
            .unwrap();
        app.handle_viewport_action(ViewportOutput {
            parametric_click: Some(vec![ParametricPick {
                object: root,
                member: Some(0),
                face: Some(face),
            }]),
            ..Default::default()
        });
    }
    enter(&mut app, "");
    let hint = app
        .reparameterize_prompt
        .as_ref()
        .unwrap()
        .hint()
        .to_owned();
    assert!(hint.starts_with("U domain start"));
    assert!(!hint.contains('<'));
    enter(&mut app, "");
    assert_eq!(app.reparameterize_prompt.as_ref().unwrap().hint(), hint);
    enter(&mut app, "-2 8 10 30");
    assert_eq!(app.document.objects().len(), before.len() + 2);
    for source in &before {
        assert_eq!(app.document.object(source.id()), Some(source));
    }
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
}
