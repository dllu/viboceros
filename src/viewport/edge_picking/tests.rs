use super::*;
use viboceros_geometry::{Brep, Frame3, NurbsSurface};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn surface(z: f64) -> NurbsSurface {
    NurbsSurface::try_bilinear([
        point(-4., -3., z),
        point(4., -3., z),
        point(4., 3., z),
        point(-4., 3., z),
    ])
    .unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn picks_real_edges_not_isocurves_and_keeps_overlapping_component_choices() {
    let mut doc = Document::default();
    let a = doc
        .add_geometry(Geometry::NurbsSurface(surface(0.)))
        .unwrap();
    let b = doc
        .add_geometry(Geometry::NurbsSurface(surface(1.)))
        .unwrap();
    let view = Viewport::new(ViewKind::Top);
    assert!(
        view.pick_edges(
            view.project(point(0., 0., 0.), rect()).unwrap(),
            rect(),
            &doc
        )
        .is_empty()
    );
    let pointer = view.project(point(0., -3., 0.), rect()).unwrap();
    let picks = view.pick_edges(pointer, rect(), &doc);
    assert_eq!(picks.len(), 2);
    assert!(picks.contains(&EdgePick { object: a, edge: 0 }));
    assert!(picks.contains(&EdgePick { object: b, edge: 0 }));
    doc.set_objects_locked([b], true).unwrap();
    assert_eq!(
        view.pick_edges(pointer, rect(), &doc),
        vec![EdgePick { object: a, edge: 0 }]
    );
    doc.set_objects_visibility([a], false).unwrap();
    assert!(view.pick_edges(pointer, rect(), &doc).is_empty());
}

#[test]
fn picking_refreshes_component_indices_after_edit_and_undo() {
    let mut doc = Document::default();
    let frame = Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        doc.tolerance(),
    )
    .unwrap();
    let brep = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], doc.tolerance())
        .unwrap()
        .try_split_edges_at_parameters(&[(0, vec![0.25, 0.75, 1.75])], doc.tolerance())
        .unwrap();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pointer = view.project(point(1., 0., 0.), rect()).unwrap();
    let before = view.pick_edges(pointer, rect(), &doc);
    assert!(before.contains(&EdgePick {
        object: id,
        edge: 13
    }));
    viboceros_command::CommandRegistry::with_builtins()
        .execute(&mut doc, &format!("MergeEdge {id} 13 All"))
        .unwrap();
    assert_ne!(view.pick_edges(pointer, rect(), &doc), before);
    doc.undo().unwrap();
    assert_eq!(view.pick_edges(pointer, rect(), &doc), before);
}

#[test]
fn real_pointer_events_capture_components_without_objects_or_drafting_points() {
    let mut doc = Document::default();
    let object = doc
        .add_geometry(Geometry::NurbsSurface(surface(7.)))
        .unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    let mut frame = |events| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(rect()),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = view.show(
                        ui,
                        &doc,
                        ViewportInput {
                            edge_pick: true,
                            drafting: DraftingInput {
                                active: true,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        (output, view.last_rect.unwrap())
    };
    let (_, area) = frame(vec![]);
    let pointer = Viewport::new(ViewKind::Top)
        .project(point(0., -3., 7.), area)
        .unwrap();
    let button = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![egui::Event::PointerMoved(pointer), button(true)]);
    let (output, _) = frame(vec![button(false)]);
    assert_eq!(output.edge_click, Some(vec![EdgePick { object, edge: 0 }]));
    assert!(output.selection_click.is_none());
    assert!(output.selection_window.is_none());
    assert!(output.picked_point.is_none());
}
