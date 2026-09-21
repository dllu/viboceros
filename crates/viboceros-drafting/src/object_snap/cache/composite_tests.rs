use super::*;
use viboceros_geometry::{Brep, CircularArc3, CurveSegment3, LineSegment, PolyCurve3, Polyline3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn line(a: Point3, b: Point3) -> LineSegment {
    LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()
}
fn nurbs() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![p(2., -2.), p(3., -2.), p(8., -2.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn query(cache: &mut ObjectSnapCache, doc: &Document, target: Point3) -> Option<ObjectSnap> {
    cache
        .nearest_projected(doc, [target.x(), target.y()], 1e-5, |p| {
            Some([p.x(), p.y()])
        })
        .unwrap()
}
fn assert_hit(cache: &mut ObjectSnapCache, doc: &Document, target: Point3, kind: ObjectSnapKind) {
    let hit = query(cache, doc, target).unwrap();
    assert_eq!(hit.kind(), kind);
    assert!(hit.point().distance_to(target).unwrap() < 1e-10);
}
fn surface(knots: Vec<Real>) -> NurbsSurface {
    NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(2., -2.),
            p(3., -2.),
            p(8., -2.),
            p(2., -6.),
            p(3., -6.),
            p(8., -6.),
        ],
        knots,
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn polycurve_and_polyline_supply_each_segment_midpoint_not_whole_curve_midpoint() {
    let points = vec![p(2., -2.), p(8., -2.), p(8., -6.)];
    let polyline = Polyline3::try_new(points.clone(), Tolerance::DEFAULT).unwrap();
    let composite =
        PolyCurve3::try_new(vec![line(points[0], points[1]), line(points[1], points[2])]).unwrap();
    let polyline_leaf = PolyCurve3::try_new(vec![
        CurveSegment3::Polyline(polyline.clone()),
        line(points[2], p(3., -6.)).into(),
    ])
    .unwrap();
    for geometry in [
        Geometry::Polyline(polyline),
        Geometry::PolyCurve(composite),
        Geometry::PolyCurve(polyline_leaf),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        for _ in 0..4 {
            for &target in &points {
                assert_hit(&mut cache, &doc, target, ObjectSnapKind::End);
                assert_eq!(query(&mut cache, &doc, target).unwrap().object_id(), id);
            }
            for target in [p(5., -2.), p(8., -4.)] {
                assert_hit(&mut cache, &doc, target, ObjectSnapKind::Mid);
            }
            assert!(query(&mut cache, &doc, p(7., -2.)).is_none());
        }
        assert!(cache.midpoints.is_empty()); // Analytic leaves require no cached integrations.
    }
}

#[test]
fn polycurve_nurbs_midpoints_are_cached_by_leaf_geometry_not_outer_parameter_map() {
    let segments = vec![
        CurveSegment3::NurbsCurve(nurbs()),
        line(p(8., -2.), p(8., -6.)).into(),
    ];
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::PolyCurve(
            PolyCurve3::try_with_segment_domains(segments.clone(), vec![100., 101., 200.]).unwrap(),
        ))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..5 {
        assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    }
    assert_eq!(cache.builds, 1);
    assert_eq!(cache.midpoints[&id].curves.len(), 1);
    assert!(query(&mut cache, &doc, p(4., -2.)).is_none());
    doc.replace_object_geometries([(
        id,
        Geometry::PolyCurve(
            PolyCurve3::try_with_segment_domains(
                segments,
                vec![-Real::MAX / 2., 0., Real::MAX / 2.],
            )
            .unwrap(),
        ),
    )])
    .unwrap();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    assert_eq!(cache.builds, 1);
    // Adding another NURBS leaf must not pass a prefix-only cache comparison.
    let extra = NurbsCurve::try_new(1, vec![p(8., -2.), p(8., -6.)], vec![0., 0., 1., 1.]).unwrap();
    doc.replace_object_geometries([(
        id,
        Geometry::PolyCurve(PolyCurve3::try_new(vec![nurbs(), extra]).unwrap()),
    )])
    .unwrap();
    assert_hit(&mut cache, &doc, p(8., -4.), ObjectSnapKind::Mid);
    assert_eq!(cache.builds, 2);
    assert_eq!(cache.midpoints[&id].curves.len(), 2);
    doc.undo().unwrap();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    assert_eq!(cache.builds, 3);
    assert_eq!(cache.midpoints[&id].curves.len(), 1);
}

#[test]
fn curved_rational_leaf_midpoint_matches_independent_circle_bisector() {
    use viboceros_geometry::WeightedPoint3;
    // A projectively reparameterized quarter circle: weights [1,sqrt(2),4]
    // move the half-arc-length point from t=1/2 to t=1/3 without changing the locus.
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(2., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., 2.), 2_f64.sqrt()).unwrap(),
            WeightedPoint3::try_new(p(0., 2.), 4.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let expected = p(2_f64.sqrt(), 2_f64.sqrt());
    assert!(curve.evaluate(0.5).unwrap().distance_to(expected).unwrap() > 0.5);
    for domain in [0.0..=1., 100.0..=104., 0.0..=1e-100] {
        let leaf = curve.try_reparameterized(domain).unwrap();
        let composite = PolyCurve3::try_new(vec![
            CurveSegment3::NurbsCurve(leaf),
            line(p(0., 2.), p(-3., 2.)).into(),
        ])
        .unwrap();
        let mut doc = Document::default();
        doc.add_geometry(Geometry::PolyCurve(composite)).unwrap();
        let mut cache = ObjectSnapCache::default();
        for _ in 0..5 {
            assert_hit(&mut cache, &doc, expected, ObjectSnapKind::Mid);
        }
        assert_eq!(cache.builds, 1);
    }
}

#[test]
fn collapsed_surface_boundary_does_not_suppress_other_snap_features() {
    let surface =
        NurbsSurface::try_bilinear([p(0., 0.), p(10., 0.), p(0., 5.), p(0., 5.)]).unwrap();
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert_hit(&mut cache, &doc, p(5., 0.), ObjectSnapKind::Mid);
    assert_hit(&mut cache, &doc, p(0., 2.5), ObjectSnapKind::Mid);
    assert_hit(&mut cache, &doc, p(5., 2.5), ObjectSnapKind::Mid);
    assert_hit(&mut cache, &doc, p(0., 5.), ObjectSnapKind::End);
    assert_eq!(cache.builds, 1);
}

#[test]
fn mixed_arc_polycurve_retains_arc_mid_and_junctions_without_inventing_leaf_centers() {
    let arc =
        CircularArc3::try_from_three_points(p(2., -4.), p(4., -2.), p(6., -4.), Tolerance::DEFAULT)
            .unwrap();
    let composite = PolyCurve3::try_new(vec![
        CurveSegment3::Arc(arc),
        line(p(6., -4.), p(9., -4.)).into(),
    ])
    .unwrap();
    let mut doc = Document::default();
    doc.add_geometry(Geometry::PolyCurve(composite)).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert_hit(&mut cache, &doc, p(4., -2.), ObjectSnapKind::Mid);
    assert_hit(&mut cache, &doc, p(7.5, -4.), ObjectSnapKind::Mid);
    assert_hit(&mut cache, &doc, p(6., -4.), ObjectSnapKind::End);
    assert!(query(&mut cache, &doc, p(4., -4.)).is_none());
}

#[test]
fn nonuniform_surface_boundary_features_match_natural_brep_without_uv_center_mid() {
    let surface = surface(vec![0., 0., 0., 1., 1., 1.]);
    assert_eq!(surface.evaluate(0.5, 0.5).unwrap(), p(4., -4.));
    let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    for geometry in [Geometry::NurbsSurface(surface), Geometry::Brep(brep)] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        for _ in 0..5 {
            for target in [p(5., -2.), p(8., -4.), p(5., -6.), p(2., -4.)] {
                assert_hit(&mut cache, &doc, target, ObjectSnapKind::Mid);
            }
            for target in [p(4., -4.), p(4., -2.), p(5., -4.)] {
                assert!(query(&mut cache, &doc, target).is_none());
            }
        }
        assert_eq!(cache.builds, 1);
    }
}

#[test]
fn unclamped_surface_uses_evaluated_natural_boundaries_not_control_rows() {
    let surface = surface(vec![-2., -1., 0., 1., 2., 3.]);
    assert_eq!(surface.evaluate(0., 0.).unwrap(), p(2.5, -2.));
    assert_eq!(surface.evaluate(1., 0.).unwrap(), p(5.5, -2.));
    // The uniform quadratic basis at 0.5 is [1/8, 3/4, 1/8].
    assert_eq!(surface.evaluate(0.5, 0.).unwrap(), p(3.5, -2.));
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    let mut cache = ObjectSnapCache::default();
    for target in [p(4., -2.), p(5.5, -4.), p(4., -6.), p(2.5, -4.)] {
        assert_hit(&mut cache, &doc, target, ObjectSnapKind::Mid);
    }
    assert!(query(&mut cache, &doc, p(5., -2.)).is_none());
}

#[test]
fn surface_cache_invalidates_edits_undo_tolerance_and_evicts_converted_or_deleted_sources() {
    let original = surface(vec![0., 0., 0., 1., 1., 1.]);
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::NurbsSurface(original)).unwrap();
    let mut cache = ObjectSnapCache::default();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    let modified = surface(vec![-2., -1., 0., 1., 2., 3.]);
    doc.replace_object_geometries([(id, Geometry::NurbsSurface(modified))])
        .unwrap();
    assert_hit(&mut cache, &doc, p(4., -2.), ObjectSnapKind::Mid);
    doc.undo().unwrap();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    doc.set_tolerance(Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap());
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    assert_eq!(cache.builds, 4);
    doc.set_objects_visibility([id], false).unwrap();
    assert!(query(&mut cache, &doc, p(5., -2.)).is_none());
    assert_eq!(cache.builds, 4);
    doc.set_objects_visibility([id], true).unwrap();
    doc.replace_object_geometries([(id, Geometry::NurbsCurve(nurbs()))])
        .unwrap();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    assert!(cache.surfaces.is_empty());
    assert_eq!(cache.midpoints.len(), 1);
    doc.undo().unwrap();
    assert_hit(&mut cache, &doc, p(5., -2.), ObjectSnapKind::Mid);
    assert!(cache.midpoints.is_empty());
    assert_eq!(cache.surfaces.len(), 1);
    doc.delete_object(id).unwrap();
    assert!(query(&mut cache, &doc, p(5., -2.)).is_none());
    assert!(cache.surfaces.is_empty());
}
