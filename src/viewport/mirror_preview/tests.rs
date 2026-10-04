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

#[test]
fn mirror_preview_follows_the_snapped_filtered_cursor_and_retains_it_outside_viewports() {
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
    let mut filter =
        PointFilterSession::new(PointFilter::parse(".wx").unwrap(), WorldPlane::Top.frame());
    filter.offer_point(point(2., 0., 0.)).unwrap();
    let preview = MirrorPreview {
        grips: &[],
        plane: MirrorPointPlane::TwoPoint {
            start: point(0., 0., 0.),
        },
        sources: &[id],
        copy: true,
        last_transform: None,
    };
    let input = ViewportInput {
        drafting: DraftingInput {
            active: true,
            osnap: ObjectSnapModes::ALL,
            ..Default::default()
        },
        point_filter: Some(filter),
        mirror_preview: Some(preview),
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
    assert_eq!(output.picked_point, Some(point(1., 5., 0.))); // App applies the filter on acceptance.
    let transform = output.mirror_preview.unwrap().unwrap();
    let expected = preview
        .plane
        .reflection_at(
            WorldPlane::Top.frame(),
            point(2., 5., 0.),
            document.tolerance(),
        )
        .unwrap();
    assert_eq!(transform, expected);
    let retained = MirrorPreview {
        grips: &[],
        last_transform: Some(transform),
        ..preview
    };
    assert_eq!(
        retained
            .resolve(
                Some(point(0., 0., 0.)),
                view.construction_plane(),
                document.tolerance()
            )
            .1,
        Some(Some(transform))
    );
    assert!(
        retained
            .resolve(None, view.construction_plane(), document.tolerance())
            .1
            .is_none()
    );
    assert_eq!(
        retained
            .resolve(None, view.construction_plane(), document.tolerance())
            .0
            .unwrap()
            .transform,
        transform
    );
    let outside = frame(
        &context,
        &mut view,
        &document,
        ViewportInput {
            mirror_preview: Some(retained),
            ..input
        },
        vec![egui::Event::PointerGone],
    );
    assert_eq!(outside.mirror_preview, None);
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(document.is_selected(id));
}
