use super::*;
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, CurveSegment3, Ellipse3, PolyCurve3, Vector3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn circle() -> NurbsCurve {
    Circle3::try_new(
        p(4., -4., 0.),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}
fn query(
    cache: &mut ObjectSnapCache,
    doc: &Document,
    cursor: [Real; 2],
    mode: ObjectSnapKind,
) -> Option<ObjectSnap> {
    cache
        .nearest_projected_with_modes(
            doc,
            cursor,
            0.01,
            |p| Some([p.x(), p.y()]),
            ObjectSnapModes::only(mode),
        )
        .unwrap()
}

#[test]
fn circular_center_and_mid_are_independently_lazy_on_one_source_snapshot() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(circle())).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::End).is_none());
    assert!(cache.curves.is_empty());
    for _ in 0..3 {
        let hit = query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).unwrap();
        assert_eq!(hit.object_id(), id);
        assert!(hit.point().distance_to(p(4., -4., 0.)).unwrap() < 1e-10);
    }
    assert_eq!(cache.builds, 1);
    let feature = &cache.curves[&id].features[0];
    assert!(feature.midpoint.get().is_none());
    assert!(feature.conic_center.get().unwrap().is_some());
    let hit = query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Mid).unwrap();
    assert_eq!(hit.kind(), ObjectSnapKind::Mid);
    assert_eq!(cache.builds, 1);
    assert!(
        cache.curves[&id].features[0]
            .midpoint
            .get()
            .unwrap()
            .is_some()
    );
    assert!(query(&mut cache, &doc, [4., -4.], ObjectSnapKind::Center).is_none());
}

#[test]
fn far_circular_sources_are_rejected_before_center_recognition_and_integration() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(circle())).unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..3 {
        assert!(query(&mut cache, &doc, [100., 100.], ObjectSnapKind::Center).is_none());
        let feature = &cache.curves[&id].features[0];
        assert!(feature.midpoint.get().is_none());
        assert!(feature.conic_center.get().is_none());
    }
    assert_eq!(cache.builds, 1);
}

#[test]
fn circular_cache_retains_failed_slots_and_refreshes_edits_undo_tolerance_and_removal() {
    let curve = circle();
    let mut controls = curve.control_points().to_vec();
    controls[1] =
        viboceros_geometry::WeightedPoint3::try_new(p(6.25, -2., 0.), controls[1].weight())
            .unwrap();
    let noncircle =
        NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec()).unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_some());
    doc.replace_object_geometries([(id, Geometry::NurbsCurve(noncircle))])
        .unwrap();
    for _ in 0..3 {
        assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_none());
    }
    assert_eq!(cache.builds, 2);
    assert_eq!(
        cache.curves[&id].features[0].conic_center.get(),
        Some(&None)
    );
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_some());
    assert_eq!(cache.builds, 3);
    doc.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_some());
    assert_eq!(cache.builds, 4);
    doc.set_objects_locked([id], true).unwrap();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_some());
    doc.set_objects_visibility([id], false).unwrap();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_none());
    doc.set_objects_visibility([id], true).unwrap();
    doc.replace_object_geometries([(id, Geometry::Point(p(4., -4., 0.)))])
        .unwrap();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_none());
    assert!(cache.curves.is_empty());
    doc.delete_object(id).unwrap();
    assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_none());
}

#[test]
fn failed_polycurve_conic_recognition_cannot_shift_another_leafs_target() {
    let ellipse = Ellipse3::try_new(
        p(4., -4., 0.),
        2.,
        1.,
        Vector3::try_new(1., 0., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Vector3::try_new(0., 1., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let ellipse = ellipse
        .try_trimmed(*ellipse.domain().start()..=ellipse.parameter_at(0.5).unwrap())
        .unwrap();
    // Alter just one of the two quarter spans: it is no longer a single conic.
    // The unmodified second span still passes through the query location.
    let mut controls = ellipse.control_points().to_vec();
    controls[1] =
        viboceros_geometry::WeightedPoint3::try_new(p(6.25, -3., 0.), controls[1].weight())
            .unwrap();
    let nonconic = NurbsCurve::try_new_rational(2, controls, ellipse.knots().to_vec()).unwrap();
    let circle = circle();
    let circle = circle
        .try_trimmed(circle.parameter_at(0.5).unwrap()..=*circle.domain().end())
        .unwrap();
    let polycurve = PolyCurve3::try_new(vec![
        CurveSegment3::NurbsCurve(nonconic),
        CurveSegment3::NurbsCurve(circle),
    ])
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::PolyCurve(polycurve)).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert!(query(&mut cache, &doc, [2.4, -3.4], ObjectSnapKind::Center).is_none());
    let hit = query(&mut cache, &doc, [2.4, -5.2], ObjectSnapKind::Center).unwrap();
    assert!(hit.point().distance_to(p(4., -4., 0.)).unwrap() < 1e-10);
    assert_eq!(
        cache.curves[&id].features[0].conic_center.get(),
        Some(&None)
    );
    assert!(
        cache.curves[&id].features[1]
            .conic_center
            .get()
            .unwrap()
            .is_some()
    );
}

#[test]
fn circular_edges_and_surface_boundaries_capture_only_the_original_arc() {
    let arc = CircularArc3::try_from_three_points(
        p(2., -4., 0.),
        p(2.4, -2.8, 0.),
        p(4., -2., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let surface = NurbsSurface::try_extruded_curve(
        &arc,
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 3.).unwrap(),
    )
    .unwrap();
    let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    for geometry in [
        Geometry::NurbsCurve(arc),
        Geometry::NurbsSurface(surface),
        Geometry::Brep(brep),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        let hit = query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).unwrap();
        assert!((hit.point().x() - 4.).abs() < 1e-10 && (hit.point().y() + 4.).abs() < 1e-10);
        assert!(query(&mut cache, &doc, [5.6, -5.2], ObjectSnapKind::Center).is_none());
        assert!(query(&mut cache, &doc, [4., -4.], ObjectSnapKind::Center).is_none());
    }
}

#[test]
fn elliptic_sources_share_lazy_features_and_capture_only_the_original_arc() {
    let arc = NurbsCurve::try_new_rational(
        2,
        vec![
            viboceros_geometry::WeightedPoint3::try_new(p(2., -4., 7.), 1.).unwrap(),
            viboceros_geometry::WeightedPoint3::try_new(
                p(2., -3., 7.),
                std::f64::consts::FRAC_1_SQRT_2,
            )
            .unwrap(),
            viboceros_geometry::WeightedPoint3::try_new(p(4., -3., 7.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let surface = NurbsSurface::try_extruded_curve(
        &arc,
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 3.).unwrap(),
    )
    .unwrap();
    let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    let polycurve = PolyCurve3::try_new(vec![CurveSegment3::NurbsCurve(arc.clone())]).unwrap();
    for geometry in [
        Geometry::NurbsCurve(arc.clone()),
        Geometry::PolyCurve(polycurve),
        Geometry::NurbsSurface(surface),
        Geometry::Brep(brep),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        assert!(query(&mut cache, &doc, [100., 100.], ObjectSnapKind::Center).is_none());
        let features =
            cache.geometry_curves(id, doc.object(id).unwrap().geometry(), doc.tolerance());
        assert!(
            features
                .iter()
                .all(|f| f.conic_center.get().is_none() && f.midpoint.get().is_none())
        );
        for _ in 0..3 {
            let hit = query(&mut cache, &doc, [2.4, -3.4], ObjectSnapKind::Center).unwrap();
            assert_eq!(hit.object_id(), id);
            assert!((hit.point().x() - 4.).abs() < 1e-10 && (hit.point().y() + 4.).abs() < 1e-10);
            assert!((hit.point().z() - 7.).abs() < 1e-10 || (hit.point().z() - 10.).abs() < 1e-10);
        }
        let features =
            cache.geometry_curves(id, doc.object(id).unwrap().geometry(), doc.tolerance());
        assert!(features.iter().all(|f| f.midpoint.get().is_none()));
        assert!(
            features
                .iter()
                .any(|f| f.conic_center.get().is_some_and(Option::is_some))
        );
        assert!(query(&mut cache, &doc, [4., -4.], ObjectSnapKind::Center).is_none());
        assert!(query(&mut cache, &doc, [5.6, -4.6], ObjectSnapKind::Center).is_none());
        doc.set_objects_visibility([id], false).unwrap();
        assert!(query(&mut cache, &doc, [2.4, -3.4], ObjectSnapKind::Center).is_none());
    }
}

#[test]
#[ignore = "local circular NURBS hover timing; not a Rhino comparison or frame budget"]
fn circular_nurbs_hover_scene_timing() {
    use std::time::Instant;
    let curve = circle();
    for count in [1, 100, 1000] {
        let mut doc = Document::default();
        for i in 0..count {
            let controls = curve
                .control_points()
                .iter()
                .map(|control| {
                    let point = control.point();
                    viboceros_geometry::WeightedPoint3::try_new(
                        p(point.x() + i as Real * 20., point.y(), point.z()),
                        control.weight(),
                    )
                    .unwrap()
                })
                .collect();
            doc.add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
                    .unwrap(),
            ))
            .unwrap();
        }
        let mut cache = ObjectSnapCache::default();
        let start = Instant::now();
        assert!(query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).is_some());
        let cold = start.elapsed();
        let start = Instant::now();
        for _ in 0..100 {
            let hit = query(&mut cache, &doc, [2.4, -2.8], ObjectSnapKind::Center).unwrap();
            assert!(hit.point().distance_to(p(4., -4., 0.)).unwrap() < 1e-10);
            std::hint::black_box(hit);
        }
        eprintln!(
            "Circular NURBS sources={count}: cold={cold:?}, warm/query={:?}",
            start.elapsed() / 100
        );
    }
}
