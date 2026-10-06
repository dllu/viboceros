use super::*;
use viboceros_drafting::{ObjectSnapModes, PointFilter, PointFilterSession};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn frame(
    context: &egui::Context,
    view: &mut Viewport,
    document: &Document,
    input: ViewportInput<'_>,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut result = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| result = view.show(ui, document, input, &[], 0, true),
        )
        .drop_without_applying_deltas();
    result
}

/// Replay the captured input camera and integer cursor, never the resulting geometry.
pub(crate) fn captured_destination(
    document: &Document,
    pending: &serde_json::Value,
    input: ViewportInput<'_>,
) -> (Point3, AffineTransform3) {
    let view = super::super::clip_tests::captured_view(&pending["camera"]);
    let [x, y]: [f32; 2] =
        serde_json::from_value(pending["frame"]["click_client"].clone()).unwrap();
    let cursor = view
        .translation_drafting_cursor(
            Pos2::new(x, y),
            view.last_rect.unwrap(),
            document,
            input.drafting,
            drafting::CursorConstraints {
                filter: input.point_filter,
                point: input.point_constraint,
                translation: input.translation_constraint,
            },
        )
        .unwrap();
    let preview = view
        .resolve_translation_preview(
            input.translation_preview.unwrap(),
            Some(cursor.point),
            document,
        )
        .0
        .unwrap();
    (cursor.point, preview.transform)
}

#[test]
fn translation_preview_uses_snaps_filters_and_retains_geometry_outside_viewports() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(point(1., 5., 0.)))
        .unwrap();
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    let mut view = Viewport::new(ViewKind::Top);
    let preview = TranslationPreview {
        grips: &[],
        sources: &[id],
        base: point(3., 0., 0.),
        copy: true,
        reference: None,
        last_transform: None,
    };
    let mut filter =
        PointFilterSession::new(PointFilter::parse(".wx").unwrap(), WorldPlane::Top.frame());
    filter.offer_point(point(2., 0., 0.)).unwrap();
    let input = ViewportInput {
        drafting: DraftingInput {
            active: true,
            osnap: ObjectSnapModes::ALL,
            ..Default::default()
        },
        point_filter: Some(filter),
        translation_preview: Some(preview),
        ..Default::default()
    };
    frame(&context, &mut view, &document, input, vec![]);
    let pointer = view
        .project(point(1., 5., 0.), view.last_rect.unwrap())
        .unwrap()
        + Vec2::new(1., 0.);
    let output = frame(
        &context,
        &mut view,
        &document,
        input,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert_eq!(output.picked_point, Some(point(1., 5., 0.))); // Acceptance applies the filter once in the app.
    let map = output.translation_preview.unwrap().unwrap();
    assert_eq!(
        map.transform_point(preview.base).unwrap(),
        point(2., 5., 0.)
    );
    let retained = TranslationPreview {
        grips: &[],
        last_transform: Some(map),
        ..preview
    };
    for (events, filter) in [
        (vec![egui::Event::PointerGone], Some(filter)),
        (
            vec![egui::Event::PointerMoved(pointer)],
            Some(PointFilterSession::new(
                PointFilter::parse(".wx").unwrap(),
                WorldPlane::Top.frame(),
            )),
        ),
    ] {
        let output = frame(
            &context,
            &mut view,
            &document,
            ViewportInput {
                translation_preview: Some(retained),
                point_filter: filter,
                ..input
            },
            events,
        );
        assert_eq!(output.translation_preview, None);
        assert_eq!(retained.resolve(None).0.unwrap().transform, map);
    }
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn translation_preview_retains_the_whole_batch_when_translation_or_geometry_overflows() {
    let mut document = Document::default();
    let finite = document
        .add_geometry(Geometry::Point(point(1., 0., 0.)))
        .unwrap();
    let large = document
        .add_geometry(Geometry::Point(point(1e308, 0., 0.)))
        .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let map = AffineTransform3::from_translation(Vector3::try_new(2., 3., 4.).unwrap());
    let preview = TranslationPreview {
        grips: &[],
        sources: &[finite, large],
        base: point(0., 0., 0.),
        copy: false,
        reference: None,
        last_transform: Some(map),
    };
    let (objects, update) =
        view.resolve_translation_preview(preview, Some(point(1e308, 0., 0.)), &document);
    assert_eq!(objects.unwrap().transform, map);
    assert_eq!(update, Some(Some(map)));
    let preview = TranslationPreview {
        grips: &[],
        last_transform: None,
        ..preview
    };
    let (objects, update) =
        view.resolve_translation_preview(preview, Some(point(1e308, 0., 0.)), &document);
    assert!(objects.is_none());
    assert_eq!(update, Some(None));
    let preview = TranslationPreview {
        grips: &[],
        base: point(-1e308, 0., 0.),
        last_transform: Some(map),
        ..preview
    };
    assert_eq!(
        preview.resolve(Some(point(1e308, 0., 0.))).1,
        Some(Some(map))
    );
}
