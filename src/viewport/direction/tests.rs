use super::*;
use crate::viewport::preview_test_support::{frame, point};
use viboceros_command::{CommandRegistry, direction::DirectionEdit};

fn surface() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([
            point(-2., -2., 0.),
            point(2., -2., 0.),
            point(2., 2., 0.),
            point(-2., 2., 0.),
        ])
        .unwrap(),
    )
}

#[test]
fn staged_axes_follow_parameter_edits_and_face_sense() {
    let mut doc = Document::default();
    let id = doc.add_geometry(surface()).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    let mut session = DirectionSession::new(&doc, false).unwrap();
    let initial = collect_markers(&session, doc.tolerance()).remove(0);
    session.apply(&doc, DirectionEdit::ReverseU, None).unwrap();
    let reversed = collect_markers(&session, doc.tolerance()).remove(0);
    for (index, sign) in [(0, -1.), (1, 1.), (2, 1.)] {
        assert!(
            (initial.axes[index]
                .unwrap()
                .dot(reversed.axes[index].unwrap())
                .unwrap()
                - sign)
                .abs()
                < 1e-12
        );
    }
    session.apply(&doc, DirectionEdit::Flip, None).unwrap();
    let flipped = collect_markers(&session, doc.tolerance()).remove(0);
    assert!(
        (initial.axes[2]
            .unwrap()
            .dot(flipped.axes[2].unwrap())
            .unwrap()
            + 1.)
            .abs()
            < 1e-12
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn inspection_paints_and_clicks_in_every_view_and_display_mode() {
    for geometry in [
        surface(),
        Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(-2., 0., 0.),
                point(2., 0., 0.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    ] {
        let mut doc = Document::default();
        // A coincident uninspected source cannot absorb the click.
        doc.add_geometry(geometry.clone()).unwrap();
        let id = doc.add_geometry(geometry).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let session = DirectionSession::new(&doc, false).unwrap();
        let markers = collect_markers(&session, doc.tolerance());
        let ids = BTreeSet::from([id]);
        let overlay = Overlay {
            edits: &session,
            markers: &markers,
            ids: &ids,
        };
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
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
                let context = egui::Context::default();
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        ..Default::default()
                    },
                    |ui| view.paint_direction(ui.painter(), rect, overlay, None),
                );
                assert!(!output.shapes.is_empty(), "{kind:?} {mode:?}");
                output.drop_without_applying_deltas();
                // Side views collapse a planar surface to a line; test face
                // picking in views where it has area.
                if matches!(kind, ViewKind::Top | ViewKind::Perspective) {
                    frame(&context, &mut view, &doc, ViewportInput::default(), vec![]);
                    let pointer = view
                        .project(point(0., 0., 0.), view.last_rect.unwrap())
                        .unwrap();
                    let hit = view
                        .direction_hit(pointer, view.last_rect.unwrap(), &doc, overlay)
                        .unwrap();
                    assert_eq!(hit.0, id);
                    assert!(hit.1.is_some());
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
                    assert_eq!(output.direction_click, Some(id));
                    assert!(output.selection_click.is_none());
                    assert!(output.selection_window.is_none());
                    assert!(output.parametric_click.is_none());
                }
            }
        }
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn trimmed_face_markers_and_hover_avoid_holes_and_retain_face_identity() {
    let fixture: viboceros_oracle::brep_direction::Fixture = serde_json::from_value(
        serde_json::json!({"source":"holed","edit":"flip","preselect":true}),
    )
    .unwrap();
    let mut doc = Document::default();
    let source = fixture.source(doc.tolerance()).unwrap();
    let id = doc.add_geometry(Geometry::Brep(source.clone())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let session = DirectionSession::new(&doc, false).unwrap();
    let markers = collect_markers(&session, doc.tolerance());
    assert_eq!(markers.len(), 1);
    let (u, v) = source.faces()[0]
        .surface()
        .closest_parameters(markers[0].point, doc.tolerance())
        .unwrap();
    assert!(
        source.faces()[0]
            .contains_parameters(u, v, doc.tolerance())
            .unwrap()
    );
    let context = egui::Context::default();
    let mut view = Viewport::new(ViewKind::Top);
    frame(&context, &mut view, &doc, ViewportInput::default(), vec![]);
    let rect = view.last_rect.unwrap();
    let ids = BTreeSet::from([id]);
    let overlay = Overlay {
        edits: &session,
        markers: &markers,
        ids: &ids,
    };
    let material = view.project(markers[0].point, rect).unwrap();
    let hit = view.direction_hit(material, rect, &doc, overlay).unwrap();
    assert_eq!(hit.0, id);
    assert!(hit.1.is_some());
    // The frozen source has an interior hole at (3,3).
    let hole = view.project(point(3., 3., 0.), rect).unwrap();
    assert!(view.direction_hit(hole, rect, &doc, overlay).is_none());
}

#[test]
fn mesh_normals_reverse_without_changing_source_storage() {
    let mut doc = Document::default();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "MeshPlane -2,-2,0 2,2,0 XCount=1 YCount=1")
        .unwrap();
    let id = doc.objects().last().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let mut session = DirectionSession::new(&doc, false).unwrap();
    let initial = collect_markers(&session, doc.tolerance());
    assert!(!initial.is_empty());
    session.apply(&doc, DirectionEdit::Flip, None).unwrap();
    let flipped = collect_markers(&session, doc.tolerance());
    assert_eq!(initial.len(), flipped.len());
    for (a, b) in initial.iter().zip(flipped) {
        assert!((a.axes[2].unwrap().dot(b.axes[2].unwrap()).unwrap() + 1.).abs() < 1e-12);
    }
}
