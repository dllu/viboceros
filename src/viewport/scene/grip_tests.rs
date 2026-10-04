use super::*;
use viboceros_document::ControlPointId;
use viboceros_geometry::Vector3;

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
