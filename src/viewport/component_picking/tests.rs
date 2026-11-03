use super::*;
use viboceros_geometry::{Brep, Polyline3};

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}
fn fixture() -> (Document, ObjectId) {
    let mut doc = Document::default();
    let boundary = |a: f64, b: f64, c: f64, d: f64| {
        Polyline3::try_new(
            vec![
                point(a, b),
                point(c, b),
                point(c, d),
                point(a, d),
                point(a, b),
            ],
            doc.tolerance(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let brep = Brep::try_planar_face_with_holes(
        &boundary(1., 1., 9., 9.),
        &[boundary(3., 3., 5., 5.), boundary(6., 6., 8., 8.)],
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    (doc, id)
}
fn selection(view: &Viewport, a: (f64, f64), b: (f64, f64)) -> Rect {
    Rect::from_two_pos(
        view.project(point(a.0, a.1), rect()).unwrap(),
        view.project(point(b.0, b.1), rect()).unwrap(),
    )
}
#[test]
fn rectangle_captures_distinct_edges_and_whole_faces_with_window_crossing_and_inverse() {
    let (mut doc, id) = fixture();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let view = Viewport {
            display_mode: mode,
            ..Viewport::new(ViewKind::Top)
        };
        let first = selection(&view, (0.5, 2.5), (5.5, 5.5));
        let both = selection(&view, (0.5, 2.5), (8.5, 8.5));
        let cross = selection(&view, (9.5, 4.5), (4.5, 3.5));
        let whole = selection(&view, (0.5, 0.5), (9.5, 9.5));
        let edges = |area, crossing, inverted| {
            view.components_in_rectangle(
                rect(),
                area,
                &doc,
                ComponentPickFilter::Edges,
                crossing,
                inverted,
            )
        };
        assert_eq!(
            edges(first, false, false),
            vec![ComponentPick {
                object: id,
                kind: ComponentSelectionKind::BrepEdge,
                index: 1
            }]
        );
        assert_eq!(edges(both, false, false).len(), 2);
        assert_eq!(edges(cross, true, false).len(), 2);
        assert_eq!(edges(whole, false, false).len(), 3);
        // The window also crosses the outer edge at x=1. Its inverse
        // excludes both crossed edges and leaves only the second hole.
        assert_eq!(edges(first, false, true).len(), 1);
        assert_eq!(edges(first, true, true).len(), 2);
        assert!(
            view.components_in_rectangle(
                rect(),
                first,
                &doc,
                ComponentPickFilter::Faces,
                false,
                false
            )
            .is_empty()
        );
        assert_eq!(
            view.components_in_rectangle(
                rect(),
                whole,
                &doc,
                ComponentPickFilter::Faces,
                false,
                false
            )
            .len(),
            1
        );
        assert_eq!(
            view.components_in_rectangle(
                rect(),
                cross,
                &doc,
                ComponentPickFilter::Faces,
                true,
                false
            )
            .len(),
            1
        );
        let in_hole = selection(&view, (3.5, 3.5), (4.5, 4.5));
        assert!(
            view.components_in_rectangle(
                rect(),
                in_hole,
                &doc,
                ComponentPickFilter::Faces,
                true,
                false
            )
            .is_empty()
        );
    }
    let view = Viewport::new(ViewKind::Top);
    doc.set_objects_locked([id], true).unwrap();
    assert!(
        view.components_in_rectangle(
            rect(),
            selection(&view, (0., 0.), (10., 10.)),
            &doc,
            ComponentPickFilter::Any,
            false,
            true
        )
        .is_empty()
    );
}
#[test]
fn subobject_gesture_keeps_press_modifiers_until_release_and_does_not_select_parent() {
    let (doc, id) = fixture();
    let mut view = Viewport::new(ViewKind::Top);
    let context = egui::Context::default();
    let mut frame = |mut events: Vec<egui::Event>, modifiers| {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
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
                            component_preselection: true,
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
    let (_, area) = frame(vec![], egui::Modifiers::NONE);
    let pointer = Viewport::new(ViewKind::Top)
        .project(point(4., 3.), area)
        .unwrap();
    let sub = egui::Modifiers {
        ctrl: true,
        shift: true,
        command: true,
        ..Default::default()
    };
    let button = |pressed, modifiers| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    frame(
        vec![egui::Event::PointerMoved(pointer), button(true, sub)],
        sub,
    );
    let (output, _) = frame(
        vec![button(false, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        output.component_click,
        Some(ComponentClick {
            picks: vec![ComponentPick {
                object: id,
                kind: ComponentSelectionKind::BrepEdge,
                index: 1
            }],
            preselection: true,
            modifiers: sub,
        })
    );
    assert!(
        output.selection_click.is_none()
            && output.picked_point.is_none()
            && output.edge_click.is_none()
    );
}

#[test]
fn rectangle_gesture_dispatches_once_after_release_without_object_selection() {
    for (preselection, press_modifiers) in [
        (true, egui::Modifiers::CTRL | egui::Modifiers::SHIFT),
        (false, egui::Modifiers::NONE),
        (false, egui::Modifiers::CTRL),
        (false, egui::Modifiers::SHIFT),
        (false, egui::Modifiers::CTRL | egui::Modifiers::SHIFT),
        (false, egui::Modifiers::ALT),
    ] {
        let (doc, id) = fixture();
        let mut view = Viewport::new(ViewKind::Top);
        let context = egui::Context::default();
        let sub = egui::Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Default::default()
        };
        let mut frame = |mut events: Vec<egui::Event>, modifiers| {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
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
                                component_preselection: preselection,
                                component_pick: (!preselection)
                                    .then_some(ComponentPickFilter::Edges),
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
        let (_, area) = frame(vec![], egui::Modifiers::NONE);
        let start = Viewport::new(ViewKind::Top)
            .project(point(0.5, 2.5), area)
            .unwrap();
        let end = Viewport::new(ViewKind::Top)
            .project(point(5.5, 5.5), area)
            .unwrap();
        let button = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        };
        let modifiers = if preselection { sub } else { press_modifiers };
        let (output, _) = frame(
            vec![
                egui::Event::PointerMoved(start),
                button(start, true, modifiers),
            ],
            modifiers,
        );
        assert!(output.component_click.is_none() && output.component_window.is_none());
        frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
        let (output, _) = frame(
            vec![button(end, false, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            output.component_window,
            Some(ComponentWindow {
                picks: vec![ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::BrepEdge,
                    index: 1
                }],
                preselection,
                modifiers,
                crossing: false,
                inverted: false
            })
        );
        assert!(
            output.selection_click.is_none()
                && output.selection_window.is_none()
                && output.component_click.is_none()
        );
    }
}

#[test]
fn command_click_retains_press_modifiers_after_keyboard_release() {
    for modifiers in [
        egui::Modifiers::CTRL,
        egui::Modifiers::SHIFT,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
        egui::Modifiers::ALT,
    ] {
        let (doc, id) = fixture();
        let mut view = Viewport::new(ViewKind::Top);
        let context = egui::Context::default();
        let mut frame = |mut events: Vec<egui::Event>, modifiers| {
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
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
                                component_pick: Some(ComponentPickFilter::Edges),
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
        let (_, area) = frame(vec![], egui::Modifiers::NONE);
        let pointer = Viewport::new(ViewKind::Top)
            .project(point(4., 3.), area)
            .unwrap();
        let button = |pressed, modifiers| egui::Event::PointerButton {
            pos: pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        };
        frame(
            vec![egui::Event::PointerMoved(pointer), button(true, modifiers)],
            modifiers,
        );
        let (output, _) = frame(
            vec![button(false, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            output.component_click,
            Some(ComponentClick {
                picks: vec![ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::BrepEdge,
                    index: 1
                }],
                preselection: false,
                modifiers
            })
        );
        assert!(output.selection_click.is_none() && output.picked_point.is_none());
    }
}

#[test]
fn extraction_face_filter_picks_subd_regions_and_excludes_meshes() {
    use viboceros_geometry::{MeshFace, SubD, TriangleMesh};
    let mut doc = Document::default();
    let points = vec![point(0., 0.), point(4., 0.), point(4., 3.), point(0., 3.)];
    let subd = SubD::try_from_polygons(points.clone(), vec![vec![0, 1, 2, 3]]).unwrap();
    let id = doc.add_geometry(Geometry::SubD(subd)).unwrap();
    let mesh = TriangleMesh::try_from_face_records(
        vec![point(8., 0.), point(12., 0.), point(12., 3.), point(8., 3.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap();
    doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let view = Viewport {
            display_mode: mode,
            ..Viewport::new(ViewKind::Top)
        };
        let pick = ComponentPick {
            object: id,
            index: 0,
            kind: ComponentSelectionKind::SubDFace,
        };
        assert_eq!(
            view.pick_components(
                view.project(point(2., 1.5), rect()).unwrap(),
                rect(),
                &doc,
                ComponentPickFilter::SurfaceAndSubDFaces
            ),
            [pick]
        );
        assert!(
            view.pick_components(
                view.project(point(10., 1.5), rect()).unwrap(),
                rect(),
                &doc,
                ComponentPickFilter::SurfaceAndSubDFaces
            )
            .is_empty()
        );
        assert_eq!(
            view.components_in_rectangle(
                rect(),
                selection(&view, (-1., -1.), (13., 4.)),
                &doc,
                ComponentPickFilter::SurfaceAndSubDFaces,
                false,
                false
            ),
            [pick]
        );
        assert!(
            view.pick_components(
                view.project(point(2., 1.5), rect()).unwrap(),
                rect(),
                &doc,
                ComponentPickFilter::Faces
            )
            .is_empty()
        );
    }
}

#[test]
fn subd_extrusion_filter_prefers_naked_edges_and_excludes_other_geometry() {
    use viboceros_geometry::{MeshFace, SubD, TriangleMesh};
    let mut doc = Document::default();
    let source = SubD::try_from_polygons(
        vec![
            point(0., 0.),
            point(4., 0.),
            point(4., 3.),
            point(0., 3.),
            point(8., 0.),
            point(8., 3.),
        ],
        vec![vec![0, 1, 2, 3], vec![1, 4, 5, 2]],
    )
    .unwrap();
    let lines = source.display_edge_polylines().unwrap();
    let boundary = source
        .edge_face_counts()
        .iter()
        .position(|&n| n == 1)
        .unwrap();
    let id = doc.add_geometry(Geometry::SubD(source.clone())).unwrap();
    doc.add_geometry(Geometry::Mesh(
        TriangleMesh::try_from_face_records(
            vec![
                point(10., 0.),
                point(14., 0.),
                point(14., 3.),
                point(10., 3.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
        )
        .unwrap(),
    ))
    .unwrap();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        for cage in [true, false] {
            doc.set_subd_control_net(cage);
            let view = Viewport {
                display_mode: mode,
                ..Viewport::new(ViewKind::Top)
            };
            let edge_point = if cage {
                point(2., 0.)
            } else {
                let l = &lines[boundary];
                l[l.len() / 2]
            };
            let filter = ComponentPickFilter::SubDExtrude;
            assert_eq!(
                view.pick_components(
                    view.project(edge_point, rect()).unwrap(),
                    rect(),
                    &doc,
                    filter
                ),
                [ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::SubDEdge,
                    index: boundary
                }]
            );
            assert_eq!(
                view.pick_components(
                    view.project(point(2., 1.5), rect()).unwrap(),
                    rect(),
                    &doc,
                    filter
                ),
                [ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::SubDFace,
                    index: 0
                }]
            );
            assert!(
                view.pick_components(
                    view.project(point(12., 1.5), rect()).unwrap(),
                    rect(),
                    &doc,
                    filter
                )
                .is_empty()
            );
            let picks = view.components_in_rectangle(
                rect(),
                selection(&view, (-1., -1.), (15., 4.)),
                &doc,
                filter,
                false,
                false,
            );
            assert_eq!(
                picks
                    .iter()
                    .filter(|p| p.kind == ComponentSelectionKind::SubDFace)
                    .count(),
                2
            );
            assert_eq!(
                picks
                    .iter()
                    .filter(|p| p.kind == ComponentSelectionKind::SubDEdge)
                    .count(),
                6
            );
            assert!(picks.iter().all(|p| p.object == id));
        }
    }
}

#[path = "inset_tests.rs"]
mod inset_tests;
