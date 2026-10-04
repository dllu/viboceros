use super::*;
use viboceros_document::ControlPointId;
use viboceros_geometry::Vector3;

#[test]
fn collapsed_mesh_grip_preview_and_committed_scene_render_in_all_modes() {
    let mut doc = Document::default();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let source = TriangleMesh::try_new_faces(
        vec![p(2., 0.), p(0., 2.), p(-2., 0.), p(0., -2.), p(2., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(source.clone())).unwrap();
    doc.enable_control_points([id]).unwrap();
    let grips = [0, 2].map(|index| ControlPointId { object: id, index });
    let owners = [id];
    let transform = AffineTransform3::try_nonuniform_scale(p(0., 0.), [-1., 1., 1.]).unwrap();
    let preview = TransformedObjects {
        sources: &owners,
        grips: &grips,
        copy: false,
        reference_sources: true,
        draw_source_faces: false,
        reversing: false,
        reference: None,
        transform,
    };
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let before = format!("{doc:?}");
    for mut view in Viewport::standard_views() {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            view.display_mode = mode;
            view.object_scene_with_object_preview(
                rect,
                &doc,
                None,
                &[],
                Some(ObjectPreview::Affine(preview)),
            );
            let cached = view.cached_scene.borrow();
            let objects = &cached.as_ref().unwrap().key.objects;
            let posed = objects.iter().find(|o| o.highlighted).unwrap();
            let Geometry::Mesh(mesh) = &*posed.geometry.geometry else {
                panic!("mesh")
            };
            assert_eq!(mesh.faces(), source.faces());
            assert_eq!(mesh.vertices()[2], mesh.vertices()[4]);
            assert_eq!(mesh.area().unwrap(), 8.);
            assert_eq!(mesh.topology().edge_count(), 4);
        }
    }
    assert_eq!(format!("{doc:?}"), before);
    doc.transform_objects_and_grips(
        [],
        grips,
        transform,
        false,
        viboceros_document::CopyGroupPolicy::Preserve,
    )
    .unwrap();
    for mut view in Viewport::standard_views() {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            view.display_mode = mode;
            view.object_scene_with_object_preview(rect, &doc, None, &[], None);
            assert!(view.cached_scene.borrow().is_some());
        }
    }
}

#[test]
fn partial_grip_scene_draws_edited_geometry_without_a_second_affine_map() {
    let mut doc = Document::default();
    let points =
        [(2., 0.), (0., 2.), (-2., 0.), (0., -2.)].map(|(x, y)| Point3::try_new(x, y, 0.).unwrap());
    let id = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    doc.enable_control_points([id]).unwrap();
    let grips = [ControlPointId {
        object: id,
        index: 0,
    }];
    let owners = [id];
    let preview = TransformedObjects {
        sources: &owners,
        grips: &grips,
        copy: false,
        reference_sources: true,
        draw_source_faces: false,
        reversing: false,
        reference: None,
        transform: AffineTransform3::from_translation(Vector3::try_new(1., 2., 3.).unwrap()),
    };
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let before = format!("{doc:?}");
    for mut view in Viewport::standard_views() {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            view.display_mode = mode;
            view.object_scene_with_object_preview(
                rect,
                &doc,
                None,
                &[],
                Some(ObjectPreview::Affine(preview)),
            );
            let cached = view.cached_scene.borrow();
            let objects = &cached.as_ref().unwrap().key.objects;
            assert_eq!(objects.len(), 2);
            let posed = objects.iter().find(|o| o.highlighted).unwrap();
            assert!(posed.transform.is_none());
            assert!(!posed.reversing);
            let Geometry::NurbsCurve(c) = &*posed.geometry.geometry else {
                panic!("curve")
            };
            assert_eq!(
                c.control_points()[0].point(),
                Point3::try_new(3., 2., 3.).unwrap()
            );
            assert_eq!(c.control_points()[1].point(), points[1]);
        }
    }
    assert_eq!(format!("{doc:?}"), before);
}
