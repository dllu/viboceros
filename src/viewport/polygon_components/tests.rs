use super::*;
use viboceros_geometry::{MeshFace, MeshNgon, SubD};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}
fn subd() -> SubD {
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
fn polygon_picking_and_windows_cover_control_faces_in_each_display_and_projection() {
    for kind in [ViewKind::Top, ViewKind::Perspective] {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut doc = Document::default();
            let source = subd();
            let display = source.tessellate_with_normals(2).unwrap();
            let f =
                display.mesh.faces()[display.face_sources.iter().position(|&i| i == 0).unwrap()];
            let points = f
                .indices()
                .iter()
                .map(|&i| display.mesh.vertices()[i as usize].to_array())
                .collect::<Vec<_>>();
            let target = Point3::try_from(std::array::from_fn(|i| {
                points.iter().map(|p| p[i]).sum::<f64>() / 4.
            }))
            .unwrap();
            let id = doc.add_geometry(Geometry::SubD(source)).unwrap();
            let mut view = Viewport::new(kind);
            view.display_mode = mode;
            view.last_rect = Some(rect());
            view.zoom_extents(&doc, ZoomExtentsBorders::default())
                .unwrap();
            let pointer = view.project(target, rect()).unwrap();
            let picks =
                view.pick_components(pointer, rect(), &doc, ComponentPickFilter::MeshAndSubDFaces);
            assert_eq!(
                picks,
                vec![ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::SubDFace,
                    index: 0
                }],
                "{kind:?} {mode:?}"
            );
            for crossing in [false, true] {
                let all = view.components_in_rectangle(
                    rect(),
                    rect(),
                    &doc,
                    ComponentPickFilter::MeshAndSubDFaces,
                    crossing,
                    false,
                );
                assert_eq!(all.len(), 2, "{kind:?} {mode:?}");
                assert!(
                    all.iter()
                        .all(|p| p.kind == ComponentSelectionKind::SubDFace)
                );
                assert!(
                    view.components_in_rectangle(
                        rect(),
                        rect(),
                        &doc,
                        ComponentPickFilter::MeshAndSubDFaces,
                        crossing,
                        true
                    )
                    .is_empty()
                );
            }
            let object = doc.object(id).unwrap();
            let cached = view.display_cache.borrow_mut().get(object, doc.tolerance());
            let outline = cached.component_face_outline(0);
            assert!(!outline.is_empty());
            let pointer = outline.as_ptr();
            assert_eq!(pointer, cached.component_face_outline(0).as_ptr());
            doc.set_objects_visibility([id], false).unwrap();
            assert!(
                view.pick_components(
                    view.project(target, rect()).unwrap(),
                    rect(),
                    &doc,
                    ComponentPickFilter::MeshAndSubDFaces
                )
                .is_empty()
            );
            doc.set_objects_visibility([id], true).unwrap();
            doc.set_objects_locked([id], true).unwrap();
            assert!(
                view.components_in_rectangle(
                    rect(),
                    rect(),
                    &doc,
                    ComponentPickFilter::MeshAndSubDFaces,
                    true,
                    false
                )
                .is_empty()
            );
        }
    }
}
#[test]
fn logical_mesh_ngons_share_boundaries_and_emit_one_pick_per_region() {
    let mut doc = Document::default();
    let mesh = TriangleMesh::try_from_face_records(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(4., 0., 0.),
            p(0., 3., 0.),
            p(2., 3., 0.),
            p(4., 3., 0.),
        ],
        vec![MeshFace::Quad([0, 1, 4, 3]), MeshFace::Quad([1, 2, 5, 4])],
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(
        vec![0, 1, 2, 5, 4, 3],
        vec![0, 1],
    )])
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let display = view
        .display_cache
        .borrow_mut()
        .get(doc.object(id).unwrap(), doc.tolerance());
    assert_eq!(display.component_face_outline(0).len(), 6);
    assert_eq!(
        display.component_face_outline(0).as_ptr(),
        display.component_face_outline(1).as_ptr()
    );
    let picks = view.components_in_rectangle(
        rect(),
        rect(),
        &doc,
        ComponentPickFilter::MeshAndSubDFaces,
        false,
        false,
    );
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].index, 0);
    assert!(
        picks
            .iter()
            .all(|p| p.kind == ComponentSelectionKind::MeshFace)
    );
}
#[test]
fn polygon_regions_follow_new_geometry_snapshots_and_archive_face_indices() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::SubD(subd())).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let before = view
        .display_cache
        .borrow_mut()
        .get(doc.object(id).unwrap(), doc.tolerance());
    let _ = before.component_face_outline(0);
    let Geometry::SubD(source) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    let (refined, children) = source.subdivided_faces_with_selection(&[0]).unwrap();
    let read = SubD::try_from_archive(refined.archive()).unwrap();
    assert_eq!(read.faces(), refined.faces());
    doc.replace_object_geometries([(id, Geometry::SubD(read))])
        .unwrap();
    let after = view
        .display_cache
        .borrow_mut()
        .get(doc.object(id).unwrap(), doc.tolerance());
    assert!(!std::rc::Rc::ptr_eq(&before, &after));
    for i in children {
        assert!(!after.component_face_outline(i).is_empty());
    }
}
