use super::*;

#[test]
fn blend_combs_render_in_each_view_and_mode_without_modifying_the_document() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line -5,0,0 0,0,0").unwrap();
    registry.execute(&mut doc, "Line 10,3,0 15,3,0").unwrap();
    doc.clear_history().unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    let mut input = viboceros_command::blend_curve::Input::new(doc.tolerance());
    for (i, p) in [
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(10., 3., 0.).unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        input.pick_curve(&doc, ids[i], None, p).unwrap();
    }
    input.option("DisplayCurvature").unwrap();
    let display = input
        .curvature_display(&viboceros_command::curvature_graph::Options {
            scale: 110,
            color: [3, 211, 137],
            ..Default::default()
        })
        .unwrap()
        .unwrap();
    let before = format!("{doc:?}");
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut counts = Vec::new();
            for overlay in [None, Some(&display)] {
                let context = egui::Context::default();
                let mut viewport = Viewport::new(kind);
                viewport.display_mode = mode;
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        ..Default::default()
                    },
                    |ui| {
                        viewport.show(
                            ui,
                            &doc,
                            ViewportInput {
                                blend_curve: Some(&input),
                                blend_curve_curvature: overlay,
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                );
                counts.push(output.shapes.iter().filter(|s|matches!(&s.shape,egui::Shape::LineSegment{stroke,..}if stroke.color==Color32::from_rgb(3,211,137))).count());
                output.drop_without_applying_deltas();
            }
            assert_eq!(counts[0], 0);
            assert!(counts[1] > 0, "{kind:?} {mode:?}");
        }
    }
    assert_eq!(format!("{doc:?}"), before);
    assert!(!doc.can_undo());
}
