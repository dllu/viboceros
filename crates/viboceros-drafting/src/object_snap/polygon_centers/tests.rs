use super::*;
use crate::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes};
use viboceros_document::Document;
use viboceros_document::{ColorRgb, ReplacementHistory};
use viboceros_geometry::{CurveSegment3, PolyCurve3, Polyline3, WeightedPoint3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn polygon_snapshots_avoid_repeated_source_scans_and_refresh_equal_replacements_once() {
    let polyline = polyline(corners(0.));
    let surface = NurbsSurface::try_bilinear([
        p(2., -2., 0.),
        p(8., -2., 0.),
        p(3., -5., 0.),
        p(6., -8., 0.),
    ])
    .unwrap();
    for geometry in [
        Geometry::Polyline(polyline.clone()),
        Geometry::NurbsCurve(CurveRef::Polyline(&polyline).to_nurbs().unwrap()),
        Geometry::NurbsSurface(surface.clone()),
        Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry.clone()).unwrap();
        let mut cache = ObjectSnapCache::default();
        let expected = capture(&mut cache, &doc, [2.8, -2.]).unwrap();
        assert_eq!(expected.point(), p(4.75, -4.25, 0.));
        let mut clone = doc.clone();
        clone
            .set_objects_color([id], Some(ColorRgb::new(10, 20, 30)))
            .unwrap();
        for _ in 0..8 {
            assert_eq!(capture(&mut cache, &clone, [2.8, -2.]), Some(expected));
            assert_eq!(capture(&mut cache, &doc, [2.8, -2.]), Some(expected));
        }
        assert_eq!(cache.polygons.source_comparisons, 0);
        assert_eq!(cache.polygons.builds, 1);
        doc.replace_object_geometries_with_history(
            [(id, geometry)],
            ReplacementHistory::EveryReplacement,
        )
        .unwrap();
        for _ in 0..8 {
            assert_eq!(capture(&mut cache, &doc, [2.8, -2.]), Some(expected));
        }
        assert_eq!(cache.polygons.source_comparisons, 1);
        assert_eq!(cache.polygons.builds, 1);
        assert!(
            cache.polygons.entries[&id]
                .source
                .shares_storage_with(doc.object(id).unwrap().geometry_snapshot())
        );
    }
}
fn corners(z: Real) -> Vec<Point3> {
    vec![
        p(2., -2., 0.),
        p(8., -2., 0.),
        p(6., -8., z),
        p(3., -5., 0.),
        p(2., -2., 0.),
    ]
}
fn polyline(points: Vec<Point3>) -> Polyline3 {
    Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap()
}
fn capture(
    cache: &mut ObjectSnapCache,
    doc: &Document,
    cursor: [Real; 2],
) -> Option<super::super::ObjectSnap> {
    cache
        .nearest_projected_with_modes(
            doc,
            cursor,
            0.05,
            |p| Some([p.x(), p.y()]),
            ObjectSnapModes::only(ObjectSnapKind::Center),
        )
        .unwrap()
}

#[test]
fn polygon_center_is_corner_mean_including_nonplanar_collinear_and_repeated_corners() {
    let mut repeated = corners(2.);
    repeated.insert(4, p(8., -2., 0.));
    let mut collinear = corners(0.);
    collinear.insert(1, p(4., -2., 0.));
    for (vertices, expected) in [
        (corners(0.), p(4.75, -4.25, 0.)),
        (corners(2.), p(4.75, -4.25, 0.5)),
        (repeated, p(5.4, -3.8, 0.4)),
        (collinear, p(4.6, -3.8, 0.)),
    ] {
        for reversed in [false, true] {
            let mut points = vertices.clone();
            if reversed {
                points.reverse();
            }
            let mut doc = Document::default();
            let id = doc
                .add_geometry(Geometry::Polyline(polyline(points)))
                .unwrap();
            let snap = capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.]).unwrap();
            assert_eq!(snap.object_id(), id);
            assert_eq!(snap.kind(), ObjectSnapKind::Center);
            assert!(snap.point().distance_to(expected).unwrap() < 1e-12);
        }
    }
}

#[test]
fn rational_degree_one_and_mixed_linear_polycurve_encodings_keep_the_same_center() {
    let line = polyline(corners(0.));
    let rational = NurbsCurve::try_new_rational(
        1,
        line.vertices()
            .iter()
            .zip([1., 2., 4., 2., 1.])
            .map(|(&p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap();
    let segments: Vec<_> = line
        .segments()
        .enumerate()
        .map(|(i, l)| {
            if i == 1 {
                CurveSegment3::NurbsCurve(l.to_nurbs().unwrap())
            } else {
                CurveSegment3::Line(l)
            }
        })
        .collect();
    for geometry in [
        Geometry::NurbsCurve(rational),
        Geometry::PolyCurve(PolyCurve3::try_new(segments).unwrap()),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        let snap = capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.]).unwrap();
        assert_eq!(snap.point(), p(4.75, -4.25, 0.));
    }
}

#[test]
fn open_gapped_and_discontinuous_boundaries_do_not_supply_centers() {
    let mut open = corners(0.);
    open.pop();
    let mut gap = corners(0.);
    *gap.last_mut().unwrap() = p(2., -2.0000001, 0.);
    let jump = NurbsCurve::try_new(
        1,
        vec![
            p(2., -2., 0.),
            p(8., -2., 0.),
            p(6., -8., 0.),
            p(3., -5., 0.),
            p(2., -2., 0.),
        ],
        vec![0., 0., 1., 1., 2., 3., 3.],
    )
    .unwrap();
    for geometry in [
        Geometry::Polyline(polyline(open)),
        Geometry::Polyline(polyline(gap)),
        Geometry::NurbsCurve(jump),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        assert!(capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.]).is_none());
    }
}

#[test]
fn boundary_hover_is_required_and_direct_features_suppress_center_on_the_same_object() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Polyline(polyline(corners(0.))))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    assert!(capture(&mut cache, &doc, [4.75, -4.25]).is_none());
    let snap = cache
        .nearest_projected(&doc, [2., -2.], 0.05, |p| Some([p.x(), p.y()]))
        .unwrap()
        .unwrap();
    assert_eq!(snap.kind(), ObjectSnapKind::End);
    assert_eq!(
        capture(&mut cache, &doc, [2., -2.]).unwrap().point(),
        p(4.75, -4.25, 0.)
    );
    doc.set_objects_locked([id], true).unwrap();
    assert!(capture(&mut cache, &doc, [2.8, -2.]).is_some());
    doc.set_objects_visibility([id], false).unwrap();
    assert!(capture(&mut cache, &doc, [2.8, -2.]).is_none());
}

#[test]
fn plane_surfaces_and_trimmed_faces_use_the_actual_outer_boundary_and_exclude_holes() {
    let points = corners(0.);
    let surface = NurbsSurface::try_bilinear([points[0], points[1], points[3], points[2]]).unwrap();
    let outer = polyline(points.clone()).to_native_nurbs().unwrap();
    let face = Brep::try_planar_face(&outer, Tolerance::DEFAULT).unwrap();
    let hole = vec![
        p(4., -3., 0.),
        p(4., -4., 0.),
        p(5., -4., 0.),
        p(5., -3., 0.),
    ];
    let mut hole = hole;
    hole.push(hole[0]);
    let holed = Brep::try_planar_face_with_holes(
        &outer,
        &[polyline(hole).to_native_nurbs().unwrap()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for geometry in [Geometry::NurbsSurface(surface), Geometry::Brep(face)] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        let snap = capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.]).unwrap();
        assert_eq!(snap.point(), p(4.75, -4.25, 0.));
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Brep(holed)).unwrap();
    assert!(capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.]).is_none());
}

#[test]
fn polygon_cache_reuses_targets_and_tracks_geometry_undo_and_deletion() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Polyline(polyline(corners(0.))))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    for _ in 0..3 {
        assert!(capture(&mut cache, &doc, [2.8, -2.]).is_some());
    }
    assert_eq!(cache.polygons.builds, 1);
    doc.replace_object_geometries([(id, Geometry::Polyline(polyline(corners(2.))))])
        .unwrap();
    assert_eq!(
        capture(&mut cache, &doc, [2.8, -2.]).unwrap().point().z(),
        0.5
    );
    assert_eq!(cache.polygons.builds, 2);
    doc.undo().unwrap();
    assert_eq!(
        capture(&mut cache, &doc, [2.8, -2.]).unwrap().point().z(),
        0.
    );
    assert_eq!(cache.polygons.builds, 3);
    doc.delete_objects([id]).unwrap();
    assert!(capture(&mut cache, &doc, [2.8, -2.]).is_none());
    assert!(cache.polygons.entries.is_empty());
}

#[test]
fn exact_corner_mean_avoids_overflow_and_retains_large_origin_offsets() {
    // The sum of x coordinates overflows; every edge and the mean are finite.
    let x = Real::MAX * 0.75;
    let step = 2_f64.powi(974);
    let feature = target(vec![
        p(x, 0., 0.),
        p(x + step, 0., 0.),
        p(x + step, 4., 0.),
        p(x, 4., 0.),
        p(x, 0., 0.),
    ])
    .unwrap();
    assert_eq!(feature.point, p(x + step * 0.5, 2., 0.));
    for origin in [0., 2_f64.powi(40)] {
        let points = corners(0.)
            .into_iter()
            .map(|p| Point3::try_new(p.x() + origin, p.y() + origin, p.z()).unwrap())
            .collect();
        assert_eq!(
            target(points).unwrap().point,
            p(origin + 4.75, origin - 4.25, 0.)
        );
    }
}

#[test]
fn collapsed_planar_surface_side_is_not_an_extra_corner() {
    let surface = NurbsSurface::try_bilinear([
        p(2., -2., 0.),
        p(8., -2., 0.),
        p(3., -8., 0.),
        p(3., -8., 0.),
    ])
    .unwrap();
    let brep = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
    for geometry in [Geometry::NurbsSurface(surface), Geometry::Brep(brep)] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        assert_eq!(
            capture(&mut ObjectSnapCache::default(), &doc, [2.8, -2.])
                .unwrap()
                .point(),
            p(13. / 3., -4., 0.)
        );
    }
}

#[test]
fn polygon_center_cache_rechecks_surface_planarity_tolerance_and_conversion() {
    let surface = |height| {
        NurbsSurface::try_bilinear([
            p(2., -2., 0.),
            p(8., -2., 0.),
            p(3., -5., 0.),
            p(6., -8., height),
        ])
        .unwrap()
    };
    for brep in [false, true] {
        let geometry = |surface| {
            if brep {
                Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap())
            } else {
                Geometry::NurbsSurface(surface)
            }
        };
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry(surface(0.))).unwrap();
        let mut cache = ObjectSnapCache::default();
        for _ in 0..3 {
            assert_eq!(
                capture(&mut cache, &doc, [2.8, -2.]).unwrap().point(),
                p(4.75, -4.25, 0.)
            );
        }
        assert_eq!(cache.polygons.builds, 1);
        doc.replace_object_geometries([(id, geometry(surface(0.001)))])
            .unwrap();
        for _ in 0..3 {
            assert!(capture(&mut cache, &doc, [2.8, -2.]).is_none());
        }
        assert_eq!(cache.polygons.builds, 2);
        doc.undo().unwrap();
        assert!(capture(&mut cache, &doc, [2.8, -2.]).is_some());
        assert_eq!(cache.polygons.builds, 3);
        doc.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
        assert!(capture(&mut cache, &doc, [2.8, -2.]).is_some());
        assert_eq!(cache.polygons.builds, 4);
        doc.replace_object_geometries([(id, Geometry::Point(p(0., 0., 0.)))])
            .unwrap();
        assert!(capture(&mut cache, &doc, [2.8, -2.]).is_none());
        assert!(cache.polygons.entries.is_empty());
    }
}

#[test]
fn surface_planarity_does_not_loosen_with_world_origin_distance() {
    for origin in [0., 2_f64.powi(40)] {
        for height in [0., 0.001] {
            let point = |x, y, z| p(origin + x, origin + y, z);
            let surface = NurbsSurface::try_bilinear([
                point(2., -2., 0.),
                point(8., -2., 0.),
                point(3., -5., 0.),
                point(6., -8., height),
            ])
            .unwrap();
            assert_eq!(
                surface_target(&surface, Tolerance::DEFAULT).is_some(),
                height == 0.
            );
        }
    }
}
