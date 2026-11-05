use super::*;
use crate::viewport::preview_test_support::frame;
use viboceros_command::direction::DirectionEdit;
use viboceros_oracle::subd_direction::source;

#[test]
fn subd_limit_center_markers_handle_quads_ngons_and_extraordinary_vertices() {
    for scene in ["bent_grid", "open_cube", "cube", "pentagon", "fan7"] {
        let mut doc = Document::default();
        let geometry = source(scene, doc.tolerance()).unwrap();
        let Geometry::SubD(s) = &geometry else {
            unreachable!()
        };
        let count = s.faces().len();
        let closed = s.solid_orientation() != viboceros_geometry::SubDSolidOrientation::NotSolid;
        let id = doc.add_geometry(geometry).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let mut session = DirectionSession::new(&doc, false).unwrap();
        let markers = collect_markers(session.geometries(), doc.tolerance());
        assert_eq!(markers.len(), count, "{scene}");
        session.apply(&doc, DirectionEdit::Flip, None).unwrap();
        let flipped = collect_markers(session.geometries(), doc.tolerance());
        for (a, b) in markers.iter().zip(flipped) {
            assert!(!a.surface && a.axes[0].is_none() && a.axes[1].is_none());
            assert!(a.point.distance_to(b.point).unwrap() < 1e-12, "{scene}");
            let dot = a.axes[2].unwrap().dot(b.axes[2].unwrap()).unwrap();
            assert!(
                (dot - if closed { 1. } else { -1. }).abs() < 1e-12,
                "{scene}"
            );
        }
    }
}

#[test]
fn subd_marker_density_stays_bounded_for_large_control_nets() {
    let mut doc = Document::default();
    viboceros_command::CommandRegistry::with_builtins()
        .execute(&mut doc, "SubDPlane XCount=33 YCount=31 0,0,0 10,10,0")
        .unwrap();
    let object = doc.objects().last().unwrap();
    let markers = collect_markers([(object.id(), object.geometry())], doc.tolerance());
    assert!(!markers.is_empty());
    assert!(markers.len() <= 256);
    assert!(markers.iter().all(|m| m.axes[2].unwrap().z() > 0.999999999));
}

#[test]
fn folded_control_quad_hover_uses_the_visible_triangle_normal() {
    use crate::viewport::preview_test_support::point;
    let mut doc = Document::default();
    // The second triangle lies above the first and projects inside it. A
    // projection-only test would incorrectly return the first triangle's +Z.
    let subd = viboceros_geometry::SubD::try_from_polygons(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(2., 2., 0.),
            point(2., 0., 2.),
        ],
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::SubD(subd)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    viboceros_command::CommandRegistry::with_builtins()
        .execute(&mut doc, "SubDDisplayToggle")
        .unwrap();
    let session = DirectionSession::new(&doc, false).unwrap();
    let ids = BTreeSet::from([id]);
    let overlay = Overlay {
        edits: &session,
        ids: &ids,
        markers: &[],
    };
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    frame(&context, &mut view, &doc, ViewportInput::default(), vec![]);
    let rect = view.last_rect.unwrap();
    let sample = point(1.6, 0.4, 1.2);
    let pointer = view.project(sample, rect).unwrap();
    let marker = view
        .direction_hit(pointer, rect, &doc, overlay)
        .unwrap()
        .1
        .unwrap();
    assert!(marker.point.distance_to(sample).unwrap() < 1e-6);
    let expected = viboceros_geometry::Vector3::try_new(1., -1., -1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    assert!(marker.axes[2].unwrap().dot(expected).unwrap() > 1. - 1e-12);
}

#[test]
fn subd_hover_flip_click_and_paint_use_cached_geometry_in_all_display_modes() {
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        for flat in [false, true] {
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let mut doc = Document::default();
                let id = doc
                    .add_geometry(source("bent_grid", doc.tolerance()).unwrap())
                    .unwrap();
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
                if flat {
                    viboceros_command::CommandRegistry::with_builtins()
                        .execute(&mut doc, "SubDDisplayToggle")
                        .unwrap();
                }
                let storage = doc.object(id).unwrap().geometry_snapshot().clone();
                let mut session = DirectionSession::new(&doc, false).unwrap();
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                let context = egui::Context::default();
                frame(&context, &mut view, &doc, ViewportInput::default(), vec![]);
                let rect = view.last_rect.unwrap();
                let sample = collect_markers(session.geometries(), doc.tolerance())[5].point;
                let pointer = view.project(sample, rect).unwrap();
                let ids = BTreeSet::from([id]);
                let hover = |session: &DirectionSession| {
                    let markers = collect_markers(session.geometries(), doc.tolerance());
                    let overlay = Overlay {
                        edits: session,
                        ids: &ids,
                        markers: &markers,
                    };
                    view.direction_hit(pointer, rect, &doc, overlay)
                        .unwrap()
                        .1
                        .unwrap()
                };
                let initial = hover(&session);
                session.apply(&doc, DirectionEdit::Flip, None).unwrap();
                let flipped = hover(&session);
                assert!(initial.point.distance_to(flipped.point).unwrap() < 1e-12);
                assert!(
                    initial.axes[2]
                        .unwrap()
                        .dot(flipped.axes[2].unwrap())
                        .unwrap()
                        < -0.999999999
                );
                let markers = collect_markers(session.geometries(), doc.tolerance());
                let overlay = Overlay {
                    edits: &session,
                    ids: &ids,
                    markers: &markers,
                };
                let paint_context = egui::Context::default();
                let output = paint_context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        ..Default::default()
                    },
                    |ui| view.paint_direction(ui.painter(), rect, overlay, Some(&flipped)),
                );
                assert!(!output.shapes.is_empty());
                output.drop_without_applying_deltas();
                let input = ViewportInput {
                    direction: Some(overlay),
                    ..Default::default()
                };
                let click = |pressed| egui::Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                };
                frame(
                    &context,
                    &mut view,
                    &doc,
                    input,
                    vec![egui::Event::PointerMoved(pointer), click(true)],
                );
                let output = frame(&context, &mut view, &doc, input, vec![click(false)]);
                assert_eq!(
                    output.direction_click,
                    Some(id),
                    "{kind:?} {mode:?} flat={flat}"
                );
                assert!(output.selection_click.is_none());
                assert!(storage.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
            }
        }
    }
}
