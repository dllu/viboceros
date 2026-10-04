use super::*;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

pub(crate) fn captured_destination(
    document: &Document,
    pending: &serde_json::Value,
    input: ViewportInput<'_>,
) -> (Point3, AffineTransform3) {
    let view = super::super::clip_tests::captured_view(&pending["camera"]);
    let [x, y]: [f32; 2] =
        serde_json::from_value(pending["frame"]["click_client"].clone()).unwrap();
    let cursor = view
        .affine_drafting_cursor(
            Pos2::new(x, y),
            view.last_rect.unwrap(),
            document,
            input.drafting,
            &input,
        )
        .unwrap();
    let transform = view
        .resolve_affine_preview(input.affine_preview.unwrap(), Some(cursor.point), document)
        .0
        .unwrap()
        .transform;
    (cursor.source_point, transform)
}

#[test]
fn affine_preview_resets_invalid_targets_retains_outside_and_rejects_batch_overflow() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(point(1., 2., 3.)))
        .unwrap();
    let large = document
        .add_geometry(Geometry::Point(point(1e308, 0., 0.)))
        .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let sources = [id, large];
    let map = AffineTransform3::identity();
    let preview = AffinePreview {
        grips: &[],
        sources: &sources,
        definition: PointTransform::Scale {
            center: point(0., 0., 0.),
            reference: point(1., 0., 0.),
        },
        frame: None,
        copy: false,
        last_transform: Some(map),
    };
    let (objects, update) =
        view.resolve_affine_preview(preview, Some(point(2., 0., 0.)), &document);
    assert_eq!(objects.unwrap().transform, map);
    assert_eq!(update, Some(Some(map)));
    for definition in [
        PointTransform::Rotate {
            center: point(0., 0., 0.),
            reference: point(1., 0., 0.),
        },
        preview.definition,
    ] {
        let preview = AffinePreview {
            grips: &[],
            sources: &sources[..1],
            definition,
            last_transform: Some(AffineTransform3::from_translation(
                Vector3::try_new(2., 0., 0.).unwrap(),
            )),
            ..preview
        };
        assert_eq!(
            preview
                .resolve(None, view.construction_plane(), document.tolerance())
                .0
                .unwrap()
                .transform,
            preview.last_transform.unwrap()
        );
        assert_eq!(
            preview
                .resolve(
                    Some(point(0., 0., 0.)),
                    view.construction_plane(),
                    document.tolerance()
                )
                .0
                .unwrap()
                .transform,
            map
        );
    }
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

#[test]
fn affine_preview_egui_snap_filter_and_click_use_the_same_point_and_hold_outside() {
    use viboceros_drafting::{ObjectSnapModes, PointFilter, PointFilterSession};
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
    let sources = [id];
    let preview = AffinePreview {
        grips: &[],
        sources: &sources,
        definition: PointTransform::Scale1D {
            center: point(0., 0., 0.),
            reference: point(4., 0., 0.),
        },
        frame: None,
        copy: true,
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
        affine_preview: Some(preview),
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
    assert_eq!(output.picked_point, Some(point(1., 5., 0.)));
    let map = output.affine_preview.unwrap().unwrap();
    assert_eq!(
        map.transform_point(point(4., 0., 0.)).unwrap(),
        point(2., 0., 0.)
    );
    let retained = AffinePreview {
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
                affine_preview: Some(retained),
                point_filter: filter,
                ..input
            },
            events,
        );
        assert_eq!(output.affine_preview, None);
        assert_eq!(
            retained
                .resolve(None, view.construction_plane(), document.tolerance())
                .0
                .unwrap()
                .transform,
            map
        );
    }
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn affine_preview_zero_numeric_factor_keeps_the_collapsed_directional_map() {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(point(1., 2., 3.)))
        .unwrap();
    let view = Viewport::new(ViewKind::Top);
    let preview = AffinePreview {
        grips: &[],
        sources: &[id],
        definition: PointTransform::Scale1DDirection {
            center: point(0., 0., 0.),
            factor: 0.,
        },
        frame: None,
        copy: true,
        last_transform: None,
    };
    let objects = view
        .resolve_affine_preview(preview, Some(point(1., 0., 0.)), &document)
        .0
        .unwrap();
    assert!(!objects.reversing);
    assert_eq!(
        objects
            .transform
            .transform_point(point(1., 2., 3.))
            .unwrap(),
        point(0., 2., 3.)
    );
}
