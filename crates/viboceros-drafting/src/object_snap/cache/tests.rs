use super::*;
use viboceros_geometry::{Brep, NurbsSurface};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn curve(shift: Real) -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![
            p(shift + 2., -2., 0.),
            p(shift + 3., -2., 0.),
            p(shift + 8., -2., 0.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn query(cache: &mut ObjectSnapCache, doc: &Document, x: Real) -> Option<ObjectSnap> {
    cache
        .nearest_projected(doc, [x, -2.], 0.1, |p| Some([p.x(), p.y()]))
        .unwrap()
}

#[test]
fn nonuniform_nurbs_mid_is_half_arc_length_not_parameter_midpoint() {
    let c = curve(0.);
    assert_eq!(c.evaluate(0.5).unwrap(), p(4., -2., 0.));
    for domain in [0.0..=1., 100.0..=104., 0.0..=1e-200, -Real::MAX..=Real::MAX] {
        let mut doc = Document::default();
        doc.add_geometry(Geometry::NurbsCurve(c.try_reparameterized(domain).unwrap()))
            .unwrap();
        let mut cache = ObjectSnapCache::default();
        let hit = query(&mut cache, &doc, 5.).unwrap();
        assert_eq!(hit.kind(), ObjectSnapKind::Mid);
        assert!(hit.point().distance_to(p(5., -2., 0.)).unwrap() < 1e-11);
        assert!(query(&mut cache, &doc, 4.).is_none());
        assert_eq!(cache.builds, 1);
    }
}

#[test]
fn brep_edge_midpoints_use_arc_length_and_cache_only_spatial_edge_curves() {
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(10., 0., 0.),
            p(0., 0., 3.),
            p(2., 0., 3.),
            p(10., 0., 3.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let brep = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        brep.edges()[0].curve().evaluate(0.5).unwrap(),
        p(3.5, 0., 0.)
    );
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..10 {
        let hit = cache
            .nearest_projected(&doc, [5., 0.], 0.1, |p| Some([p.x(), p.z()]))
            .unwrap()
            .unwrap();
        assert_eq!(hit.kind(), ObjectSnapKind::Mid);
        assert!(hit.point().distance_to(p(5., 0., 0.)).unwrap() < 1e-11);
    }
    assert_eq!(cache.builds, 1);
    assert_eq!(cache.curves[&id].features.len(), 4);
}

#[test]
fn midpoint_cache_invalidates_on_edits_undo_tolerance_conversion_and_deletion() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve(0.))).unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..10 {
        assert!(query(&mut cache, &doc, 5.).is_some());
    }
    assert_eq!(cache.builds, 1);
    doc.replace_object_geometries([(id, Geometry::NurbsCurve(curve(10.)))])
        .unwrap();
    assert!(query(&mut cache, &doc, 15.).is_some());
    assert_eq!(cache.builds, 2);
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, 5.).is_some());
    assert_eq!(cache.builds, 3);
    doc.set_tolerance(Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap());
    assert!(query(&mut cache, &doc, 5.).is_some());
    assert_eq!(cache.builds, 4);
    doc.set_objects_visibility([id], false).unwrap();
    assert!(query(&mut cache, &doc, 5.).is_none());
    assert_eq!(cache.builds, 4);
    doc.set_objects_visibility([id], true).unwrap();
    doc.replace_object_geometries([(id, Geometry::Point(p(5., -2., 0.)))])
        .unwrap();
    assert_eq!(
        query(&mut cache, &doc, 5.).unwrap().kind(),
        ObjectSnapKind::Point
    );
    assert!(cache.curves.is_empty());
    doc.replace_object_geometries([(id, Geometry::NurbsCurve(curve(0.)))])
        .unwrap();
    assert!(query(&mut cache, &doc, 5.).is_some());
    assert_eq!(cache.builds, 5);
    doc.delete_object(id).unwrap();
    assert!(query(&mut cache, &doc, 5.).is_none());
    assert!(cache.curves.is_empty());
}

#[test]
fn cached_axis_aligned_and_projected_queries_agree_with_stateless_queries() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsCurve(curve(0.))).unwrap();
    let mut cache = ObjectSnapCache::default();
    for i in 0..=20 {
        let cursor = [Real::from(i) * 0.5, -2.];
        let expected =
            nearest_object_snap_projected(&doc, cursor, 0.3, |p| Some([p.x(), p.y()])).unwrap();
        assert_eq!(
            cache
                .nearest_projected(&doc, cursor, 0.3, |p| Some([p.x(), p.y()]))
                .unwrap(),
            expected
        );
        assert_eq!(
            cache
                .nearest_axis_aligned(&doc, PointCloudProjection::Xy, p(0., 0., 0.), cursor, 0.3)
                .unwrap(),
            expected
        );
    }
    assert_eq!(cache.builds, 1);
}

#[test]
fn near_reuses_source_snapshots_without_initializing_mid_or_center_slots() {
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve(0.))).unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..5 {
        let hit = cache
            .nearest_projected_with_modes(
                &doc,
                [3., -2.1],
                0.2,
                |p| Some([p.x(), p.y()]),
                ObjectSnapModes::only(ObjectSnapKind::Near),
            )
            .unwrap()
            .unwrap();
        assert!((hit.point().x() - 3.).abs() < 1e-10);
        let feature = &cache.curves[&id].features[0];
        assert!(feature.midpoint.get().is_none());
        assert!(feature.conic_center.get().is_none());
    }
    assert_eq!(cache.builds, 1);
    assert_eq!(cache.source_comparisons, 0);
}
