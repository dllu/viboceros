use super::*;
use crate::app::extract_points_native as native;

#[test]
fn subd_display_cache_uses_native_control_locations_and_retains_component_ordinals() {
    for row in native::display_rows().into_iter().filter(|r| {
        r["value"]["input"]["flat"] == true && r["value"]["input"]["sources"][0]["mode"].is_null()
    }) {
        let (mut doc, ids) = native::setup(&row);
        let id = ids[0];
        let snapshot = doc.object(id).unwrap().geometry_snapshot().clone();
        let mut cache = DisplayCache::default();
        let object = doc.object(id).unwrap();
        let smooth = cache.get(object, doc.tolerance(), false);
        let flat = cache.get(object, doc.tolerance(), true);
        assert!(!Rc::ptr_eq(&smooth, &flat));
        assert!(Rc::ptr_eq(&flat, &cache.get(object, doc.tolerance(), true)));
        let Geometry::SubD(subd) = object.geometry() else {
            panic!()
        };
        let mesh = flat.mesh().unwrap();
        assert_eq!(mesh.face_count(), subd.faces().len());
        assert_eq!(
            flat.face_sources().unwrap(),
            (0..subd.faces().len()).collect::<Vec<_>>()
        );
        let expected = &row["value"]["subd_witness"]["sources"][0]["vertices"];
        for p in mesh.vertices() {
            assert!(expected.as_array().unwrap().iter().any(|v| {
                let q = serde_json::from_value::<[f64; 3]>(v["control"].clone()).unwrap();
                p.to_array()
                    .iter()
                    .zip(q)
                    .all(|(a, b)| (a - b).abs() < 1e-7)
            }));
        }
        assert_eq!(flat.edges().len(), subd.edges().len());
        for (index, edge) in subd.edges().iter().enumerate() {
            assert_eq!(
                flat.edges()[index],
                vec![edge.map(|i| subd.control_points()[i as usize])]
            );
        }
        for (i, n) in flat.normals().iter().enumerate() {
            let expected = mesh.face_normal(i).unwrap().as_vector().to_array();
            assert!(
                n.iter()
                    .all(|n| n.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-12))
            );
        }
        let control_point = subd.control_points()[0];
        doc.set_subd_control_net(true);
        assert!(
            doc.object(id)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(&snapshot)
        );
        assert!(!doc.can_undo());
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
                let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.));
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                view.last_rect = Some(rect);
                view.zoom_extents(&doc, ZoomExtentsBorders::default())
                    .unwrap();
                if let Some(pointer) = view.project_selection_point(control_point, rect) {
                    assert!(
                        view.pick_polygon_vertices(pointer, rect, &doc)
                            .contains(&ComponentPick {
                                object: id,
                                kind: viboceros_command::ComponentSelectionKind::SubDVertex,
                                index: 0,
                            }),
                        "{} {kind:?} {mode:?}",
                        row["id"]
                    );
                }
            }
        }
    }
}

#[test]
fn subd_flat_ngon_fans_keep_one_component_and_non_subds_keep_their_cache() {
    let points = (0..5)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / 5.;
            Point3::try_new(t.cos(), t.sin(), 0.).unwrap()
        })
        .chain([[3., 0., 0.], [4., 0., 0.], [3., 1., 0.]].map(|p| Point3::try_from(p).unwrap()))
        .collect();
    let subd = viboceros_geometry::SubD::try_from_polygons(
        points,
        vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7]],
    )
    .unwrap();
    let display =
        DisplayGeometry::in_appearance(Geometry::SubD(subd).into(), 1, Tolerance::DEFAULT, true);
    assert_eq!(display.face_sources().unwrap(), [0, 0, 0, 0, 0, 1]);
    assert_eq!(display.component_face_region(0), Some(0));
    assert_eq!(display.component_face_region(1), Some(1));
    assert_eq!(display.component_face_outline(0).len(), 5);
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    let mut cache = DisplayCache::default();
    let object = doc.object(id).unwrap();
    let a = cache.get(object, doc.tolerance(), false);
    let b = cache.get(object, doc.tolerance(), true);
    assert!(Rc::ptr_eq(&a, &b));
}

#[test]
fn subd_display_whole_deformation_preview_rebuilds_when_appearance_changes() {
    let row = native::display_rows()
        .into_iter()
        .find(|r| r["id"] == "display_subd_flat_whole")
        .unwrap();
    let (mut doc, ids) = native::setup(&row);
    let source = doc.object(ids[0]).unwrap().geometry_snapshot().clone();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let cache = RefCell::new(super::super::bend_preview::BendPreviewCache::default());
    let displays = RefCell::new(DisplayCache::default());
    let preview = super::super::bend_preview::BendPreview {
        sources: &ids,
        start: p(0., 0., 0.),
        end: p(4., 0., 0.),
        options: viboceros_command::bend::BendOptions {
            angle: Some(45.),
            ..Default::default()
        },
        last_point: Some(p(4., 3., 0.)),
        cache: &cache,
    };
    doc.set_subd_control_net(true);
    let flat = preview.resolve(None, &doc, &displays).0.unwrap();
    assert_eq!(
        flat.objects[&ids[0]].geometry.mesh().unwrap().face_count(),
        6
    );
    doc.set_subd_control_net(false);
    let smooth = preview.resolve(None, &doc, &displays).0.unwrap();
    assert!(!Rc::ptr_eq(&flat, &smooth));
    assert!(
        smooth.objects[&ids[0]]
            .geometry
            .mesh()
            .unwrap()
            .face_count()
            > 6
    );
    assert!(
        doc.object(ids[0])
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&source)
    );
    assert!(!doc.can_undo());
}
