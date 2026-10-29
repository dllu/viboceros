use super::*;
use viboceros_geometry::{MeshFace, SubD};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn merge_getter_clicks_and_windows_cover_all_six_polygon_component_kinds() {
    let points = vec![
        p(0., 0., 0.),
        p(2., 0., 0.),
        p(4., 0., 0.),
        p(0., 3., 0.),
        p(2., 3., 0.),
        p(4., 3., 0.),
    ];
    let faces = vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]];
    for subd in [false, true] {
        for projection in [ViewKind::Top, ViewKind::Perspective] {
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let mut doc = Document::default();
                // Control-net display keeps the clicked vertex/edge/face points
                // common across mesh and SubD, exercising the shared filter.
                doc.set_subd_control_net(true);
                let (geometry, kinds) = if subd {
                    (
                        Geometry::SubD(
                            SubD::try_from_polygons(points.clone(), faces.clone()).unwrap(),
                        ),
                        [
                            ComponentSelectionKind::SubDVertex,
                            ComponentSelectionKind::SubDEdge,
                            ComponentSelectionKind::SubDFace,
                        ],
                    )
                } else {
                    (
                        Geometry::Mesh(
                            TriangleMesh::try_from_face_records(
                                points.clone(),
                                vec![MeshFace::Quad([0, 1, 4, 3]), MeshFace::Quad([1, 2, 5, 4])],
                            )
                            .unwrap(),
                        ),
                        [
                            ComponentSelectionKind::MeshVertex,
                            ComponentSelectionKind::MeshEdge,
                            ComponentSelectionKind::MeshFace,
                        ],
                    )
                };
                let id = doc.add_geometry(geometry).unwrap();
                let mut view = Viewport::new(projection);
                view.display_mode = mode;
                view.last_rect = Some(rect());
                view.zoom_extents(&doc, ZoomExtentsBorders::default())
                    .unwrap();
                for (point, kind) in [points[0], p(1., 0., 0.), p(1., 1.5, 0.)]
                    .into_iter()
                    .zip(kinds)
                {
                    let pointer = view.project(point, rect()).unwrap();
                    let picks = view.pick_components(
                        pointer,
                        rect(),
                        &doc,
                        ComponentPickFilter::MeshAndSubDComponents,
                    );
                    assert!(!picks.is_empty(), "{subd} {projection:?} {mode:?} {kind:?}");
                    assert!(
                        picks.iter().all(|p| p.object == id && p.kind == kind),
                        "{picks:?}"
                    );
                }
                for crossing in [false, true] {
                    let all = view.components_in_rectangle(
                        rect(),
                        rect(),
                        &doc,
                        ComponentPickFilter::MeshAndSubDComponents,
                        crossing,
                        false,
                    );
                    for (kind, count) in kinds.into_iter().zip([6, 7, 2]) {
                        assert_eq!(
                            all.iter().filter(|p| p.kind == kind).count(),
                            count,
                            "{subd} {projection:?} {mode:?} {kind:?}"
                        );
                    }
                    assert_eq!(all.len(), 15);
                    assert!(
                        view.components_in_rectangle(
                            rect(),
                            rect(),
                            &doc,
                            ComponentPickFilter::MeshAndSubDComponents,
                            crossing,
                            true
                        )
                        .is_empty()
                    );
                }
                doc.set_objects_locked([id], true).unwrap();
                assert!(
                    view.components_in_rectangle(
                        rect(),
                        rect(),
                        &doc,
                        ComponentPickFilter::MeshAndSubDComponents,
                        true,
                        false
                    )
                    .is_empty()
                );
                assert!(
                    view.pick_components(
                        view.project(points[0], rect()).unwrap(),
                        rect(),
                        &doc,
                        ComponentPickFilter::MeshAndSubDComponents
                    )
                    .is_empty()
                );
            }
        }
    }
}

#[test]
fn merge_filter_excludes_brep_faces_and_edges_from_clicks_and_windows() {
    use viboceros_geometry::{Brep, Polyline3};
    let mut doc = Document::default();
    let boundary = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(4., 0., 0.),
            p(4., 4., 0.),
            p(0., 4., 0.),
            p(0., 0., 0.),
        ],
        doc.tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    doc.add_geometry(Geometry::Brep(
        Brep::try_planar_face_with_holes(&boundary, &[], doc.tolerance()).unwrap(),
    ))
    .unwrap();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = mode;
        view.last_rect = Some(rect());
        view.zoom_extents(&doc, ZoomExtentsBorders::default())
            .unwrap();
        for point in [p(2., 2., 0.), p(2., 0., 0.)] {
            assert!(
                view.pick_components(
                    view.project(point, rect()).unwrap(),
                    rect(),
                    &doc,
                    ComponentPickFilter::MeshAndSubDComponents
                )
                .is_empty()
            );
        }
        assert!(
            view.components_in_rectangle(
                rect(),
                rect(),
                &doc,
                ComponentPickFilter::MeshAndSubDComponents,
                true,
                false
            )
            .is_empty()
        );
    }
}
