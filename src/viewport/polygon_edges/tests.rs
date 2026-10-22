use super::*;
use viboceros_geometry::{MeshFace, SubD};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}
fn source() -> SubD {
    SubD::try_from_polygons(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(4., 0., 1.),
            p(0., 3., 0.),
            p(2., 3., 0.),
            p(4., 3., 1.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
}
#[test]
fn smooth_subd_edges_and_vertices_pick_in_each_projection_and_display_mode() {
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
            let mut doc = Document::default();
            let subd = source();
            let id = doc.add_geometry(Geometry::SubD(subd.clone())).unwrap();
            let mut view = Viewport::new(kind);
            view.display_mode = mode;
            view.last_rect = Some(rect());
            view.zoom_extents(&doc, ZoomExtentsBorders::default())
                .unwrap();
            let lines = subd.display_edge_polylines().unwrap();
            for (index, line) in lines.iter().enumerate() {
                let target = line[line.len() / 2];
                if let Some(pointer) = view.project_selection_point(target, rect()) {
                    let picks =
                        view.pick_components(pointer, rect(), &doc, ComponentPickFilter::Crease);
                    // Edge-on views can collapse an edge onto a vertex; use
                    // the edge-only query to verify capture independently.
                    let expected = ComponentPick {
                        object: id,
                        kind: ComponentSelectionKind::SubDEdge,
                        index,
                    };
                    assert!(
                        view.pick_component_edges(pointer, rect(), &doc, false)
                            .contains(&expected),
                        "{kind:?} {mode:?} {index}"
                    );
                    assert!(!picks.is_empty());
                }
            }
            for (index, &point) in subd.surface_points().iter().enumerate() {
                if let Some(pointer) = view.project_selection_point(point, rect()) {
                    let expected = ComponentPick {
                        object: id,
                        kind: ComponentSelectionKind::SubDVertex,
                        index,
                    };
                    assert!(
                        view.pick_components(pointer, rect(), &doc, ComponentPickFilter::Crease)
                            .contains(&expected)
                    );
                }
            }
            let cached = view
                .display_cache
                .borrow_mut()
                .get(doc.object(id).unwrap(), doc.tolerance());
            let address = cached.edges().as_ptr();
            assert_eq!(address, cached.edges().as_ptr());
            let all = view.components_in_rectangle(
                rect(),
                rect(),
                &doc,
                ComponentPickFilter::Crease,
                false,
                false,
            );
            assert_eq!(all.len(), subd.edges().len() + subd.control_points().len());
            assert!(
                view.components_in_rectangle(
                    rect(),
                    rect(),
                    &doc,
                    ComponentPickFilter::Crease,
                    true,
                    true
                )
                .is_empty()
            );
            assert!(
                view.pick_components(
                    Pos2::new(-1., -1.),
                    rect(),
                    &doc,
                    ComponentPickFilter::Crease
                )
                .is_empty()
            );
            doc.set_objects_locked([id], true).unwrap();
            assert!(
                view.components_in_rectangle(
                    rect(),
                    rect(),
                    &doc,
                    ComponentPickFilter::Crease,
                    true,
                    false
                )
                .is_empty()
            );
        }
    }
}
#[test]
fn mesh_seams_keep_topology_edge_ordinals_and_brep_only_getters_stay_filtered() {
    let mut doc = Document::default();
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(2., 3., 0.),
            p(0., 3., 0.),
            p(2., 0., 0.),
            p(4., 0., 1.),
            p(4., 3., 1.),
            p(2., 3., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
    )
    .unwrap();
    let edges = mesh.topology_edge_points();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.last_rect = Some(rect());
    view.zoom_extents(&doc, ZoomExtentsBorders::default())
        .unwrap();
    for (index, &[a, b]) in edges.iter().enumerate() {
        let target = p(
            (a.x() + b.x()) / 2.,
            (a.y() + b.y()) / 2.,
            (a.z() + b.z()) / 2.,
        );
        let pointer = view.project(target, rect()).unwrap();
        assert!(
            view.pick_components(pointer, rect(), &doc, ComponentPickFilter::Crease)
                .contains(&ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::MeshEdge,
                    index
                })
        );
        assert!(
            view.pick_components(pointer, rect(), &doc, ComponentPickFilter::Edges)
                .is_empty()
        );
    }
    assert_eq!(edges.len(), 7);
    assert_eq!(
        view.components_in_rectangle(
            rect(),
            rect(),
            &doc,
            ComponentPickFilter::Crease,
            false,
            false
        )
        .len(),
        7
    );
}
#[test]
fn points_on_uses_the_visible_control_cage_and_hidden_grips_cannot_be_clicked() {
    let mut doc = Document::default();
    let subd = source();
    let id = doc.add_geometry(Geometry::SubD(subd.clone())).unwrap();
    doc.enable_control_points([id]).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.last_rect = Some(rect());
    view.zoom_extents(&doc, ZoomExtentsBorders::default())
        .unwrap();
    for (index, &point) in subd.control_points().iter().enumerate() {
        let pointer = view.project(point, rect()).unwrap();
        assert!(
            view.pick_components(pointer, rect(), &doc, ComponentPickFilter::Crease)
                .contains(&ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::SubDVertex,
                    index
                })
        );
    }
    doc.set_control_points_hidden(
        [viboceros_document::ControlPointId {
            object: id,
            index: 0,
        }],
        true,
    )
    .unwrap();
    let pointer = view.project(subd.control_points()[0], rect()).unwrap();
    assert!(
        !view
            .pick_components(pointer, rect(), &doc, ComponentPickFilter::Crease)
            .contains(&ComponentPick {
                object: id,
                kind: ComponentSelectionKind::SubDVertex,
                index: 0
            })
    );
    assert!(
        !view
            .components_in_rectangle(
                rect(),
                rect(),
                &doc,
                ComponentPickFilter::Crease,
                false,
                false
            )
            .contains(&ComponentPick {
                object: id,
                kind: ComponentSelectionKind::SubDVertex,
                index: 0
            })
    );
}
#[test]
fn real_pointer_events_pick_before_and_after_start_and_retain_press_modifiers() {
    for preselection in [false, true] {
        let mut doc = Document::default();
        let subd = source();
        let target = subd.surface_points()[0];
        let id = doc.add_geometry(Geometry::SubD(subd)).unwrap();
        let mut view = Viewport::new(ViewKind::Top);
        let context = egui::Context::default();
        let press = if preselection {
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
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
                                    .then_some(ComponentPickFilter::Crease),
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
        let pointer = Viewport::new(ViewKind::Top).project(target, area).unwrap();
        let button = |pressed, modifiers| egui::Event::PointerButton {
            pos: pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        };
        frame(
            vec![egui::Event::PointerMoved(pointer), button(true, press)],
            press,
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
                    kind: ComponentSelectionKind::SubDVertex,
                    index: 0
                }],
                preselection,
                modifiers: press
            })
        );
        assert!(output.selection_click.is_none() && output.picked_point.is_none());
    }
}
#[test]
fn a_subd_vertex_does_not_suppress_an_unrelated_crossing_mesh_edge() {
    let mut doc = Document::default();
    let subd = source();
    let target = subd.surface_points()[0];
    let subd_id = doc.add_geometry(Geometry::SubD(subd)).unwrap();
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            p(target.x() - 1., target.y(), target.z()),
            p(target.x() + 1., target.y(), target.z()),
            p(target.x(), target.y() + 2., target.z()),
        ],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let index = mesh
        .topology_edge_points()
        .iter()
        .position(|&[a, b]| a.y() == target.y() && b.y() == target.y())
        .unwrap();
    let mesh_id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pointer = view.project(target, rect()).unwrap();
    for filter in [ComponentPickFilter::Crease, ComponentPickFilter::Any] {
        let picks = view.pick_components(pointer, rect(), &doc, filter);
        assert!(picks.contains(&ComponentPick {
            object: subd_id,
            kind: ComponentSelectionKind::SubDVertex,
            index: 0
        }));
        assert!(picks.contains(&ComponentPick {
            object: mesh_id,
            kind: ComponentSelectionKind::MeshEdge,
            index
        }));
    }
}
