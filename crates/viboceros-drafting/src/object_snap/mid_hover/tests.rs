use super::*;
use crate::{ObjectSnapKind, ObjectSnapModes};
use viboceros_document::Document;
use viboceros_geometry::CurveSegment3;
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, Ellipse3, LineSegment, NurbsCurve, NurbsSurface,
    PointCloudProjection, PolyCurve3, Polyline3, Vector3, WeightedPoint3,
};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn line(a: Point3, b: Point3) -> LineSegment {
    LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap()
}
fn curve() -> NurbsCurve {
    NurbsCurve::try_new(
        2,
        vec![p(2., -4.), p(3., -4.), p(8., -4.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn hit(
    cache: &mut ObjectSnapCache,
    doc: &Document,
    aim: Point3,
    modes: ObjectSnapModes,
) -> Option<super::super::ObjectSnap> {
    cache
        .nearest_projected_with_modes(
            doc,
            [aim.x(), aim.y()],
            0.05,
            |p| Some([p.x(), p.y()]),
            modes,
        )
        .unwrap()
}
fn only_mid() -> ObjectSnapModes {
    ObjectSnapModes::only(ObjectSnapKind::Mid)
}

#[test]
fn mid_only_hover_covers_analytic_composite_surface_and_brep_segments() {
    let arc =
        CircularArc3::try_from_three_points(p(2., -4.), p(4., -2.), p(6., -4.), Tolerance::DEFAULT)
            .unwrap();
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(2., -4.),
            p(3., -4.),
            p(8., -4.),
            p(2., -8.),
            p(3., -8.),
            p(8., -8.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for (geometry, aim, target) in [
        (
            Geometry::Line(line(p(2., -4.), p(8., -4.))),
            p(2.8, -4.),
            p(5., -4.),
        ),
        (Geometry::NurbsCurve(curve()), p(2.8, -4.), p(5., -4.)),
        (Geometry::Arc(arc), p(2.4, -2.8), p(4., -2.)),
        (
            Geometry::Polyline(
                Polyline3::try_new(vec![p(2., -4.), p(8., -4.), p(8., -8.)], Tolerance::DEFAULT)
                    .unwrap(),
            ),
            p(2.8, -4.),
            p(5., -4.),
        ),
        (
            Geometry::PolyCurve(
                PolyCurve3::try_new(vec![
                    CurveSegment3::NurbsCurve(curve()),
                    line(p(8., -4.), p(8., -8.)).into(),
                ])
                .unwrap(),
            ),
            p(2.8, -4.),
            p(5., -4.),
        ),
        (
            Geometry::PolyCurve(
                PolyCurve3::try_new(vec![
                    CurveSegment3::Arc(arc),
                    line(p(6., -4.), p(9., -4.)).into(),
                ])
                .unwrap(),
            ),
            p(2.4, -2.8),
            p(4., -2.),
        ),
        (
            Geometry::NurbsSurface(surface.clone()),
            p(2.8, -4.),
            p(5., -4.),
        ),
        (
            Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()),
            p(2.8, -4.),
            p(5., -4.),
        ),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        for _ in 0..3 {
            let snap = hit(&mut cache, &doc, aim, only_mid()).unwrap();
            assert_eq!(snap.kind(), ObjectSnapKind::Mid);
            assert_eq!(snap.object_id(), id);
            assert!(snap.point().distance_to(target).unwrap() < 1e-10);
            assert!(snap.distance() < 1e-8);
            assert!(
                hit(
                    &mut cache,
                    &doc,
                    aim,
                    only_mid().with(ObjectSnapKind::Point, true)
                )
                .is_none()
            );
        }
        doc.set_objects_locked([id], true).unwrap();
        assert!(hit(&mut cache, &doc, aim, only_mid()).is_some());
        doc.set_objects_visibility([id], false).unwrap();
        assert!(hit(&mut cache, &doc, aim, only_mid()).is_none());
    }
}

#[test]
fn circle_and_ellipse_mid_are_opposite_the_stored_seam() {
    let x = Vector3::try_new(1., 0., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let y = Vector3::try_new(0., 1., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let z = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for (geometry, aim, target) in [
        (
            Geometry::Circle(Circle3::try_new(p(4., -4.), 2., z, Tolerance::DEFAULT).unwrap()),
            p(5.6, -2.8),
            p(2., -4.),
        ),
        (
            Geometry::Ellipse(
                Ellipse3::try_new(p(4., -4.), 3., 2., x, y, Tolerance::DEFAULT).unwrap(),
            ),
            p(6.4, -2.8),
            p(1., -4.),
        ),
    ] {
        let mut doc = Document::default();
        doc.add_geometry(geometry).unwrap();
        let mut cache = ObjectSnapCache::default();
        assert!(
            hit(&mut cache, &doc, aim, only_mid())
                .unwrap()
                .point()
                .distance_to(target)
                .unwrap()
                < 1e-10
        );
        let mixed = only_mid().with(ObjectSnapKind::Point, true);
        assert!(hit(&mut cache, &doc, aim, mixed).is_none());
        let snap = hit(&mut cache, &doc, target, mixed).unwrap();
        assert_eq!(snap.kind(), ObjectSnapKind::Mid);
        assert!(snap.point().distance_to(target).unwrap() < 1e-10);
    }
}

#[test]
fn rational_mid_remains_half_arc_length_across_extreme_parameter_frames() {
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
    let aim = curve.evaluate(0.01).unwrap();
    let expected = p(2_f64.sqrt(), 2_f64.sqrt());
    for domain in [
        0.0..=1.,
        100.0..=104.,
        1.0..=1_f64.next_up(),
        0.0..=f64::from_bits(8),
        -Real::MAX..=Real::MAX,
    ] {
        let mut doc = Document::default();
        doc.add_geometry(Geometry::NurbsCurve(
            curve.try_reparameterized(domain).unwrap(),
        ))
        .unwrap();
        let snap = hit(&mut ObjectSnapCache::default(), &doc, aim, only_mid()).unwrap();
        assert!(snap.point().distance_to(expected).unwrap() < 1e-9);
        assert!(snap.distance() < 1e-9);
    }
}

#[test]
fn axis_aligned_hover_retains_local_precision_at_large_world_origins() {
    for origin in [0., 2_f64.powi(40)] {
        let mut doc = Document::default();
        doc.add_geometry(Geometry::Line(line(
            p(origin + 2., origin - 4.),
            p(origin + 8., origin - 4.),
        )))
        .unwrap();
        let snap = ObjectSnapCache::default()
            .nearest_axis_aligned_with_modes(
                &doc,
                PointCloudProjection::Xy,
                p(origin, origin),
                [2.8, -4.],
                0.05,
                only_mid(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(snap.point(), p(origin + 5., origin - 4.));
        assert!(snap.distance() < 1e-12);
    }
}

#[test]
fn discontinuous_nurbs_never_create_a_hover_chord_through_a_jump() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., -1.), p(-4., 1.), p(4., -1.), p(4., 1.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let mut doc = Document::default();
    doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    // Even if a midpoint were available, no native span passes through this cursor.
    assert!(hit(&mut ObjectSnapCache::default(), &doc, p(0., 0.), only_mid()).is_none());
}

#[test]
fn unprojectable_mid_target_is_not_admitted_by_a_visible_curve() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Line(line(p(2., -4.), p(8., -4.))))
        .unwrap();
    let snap = ObjectSnapCache::default()
        .nearest_projected_with_modes(
            &doc,
            [2.8, -4.],
            0.05,
            |p| (p.x() < 4.).then_some([p.x(), p.y()]),
            only_mid(),
        )
        .unwrap();
    assert!(snap.is_none());
}

#[test]
fn competing_segments_rank_by_curve_distance_not_distance_to_their_midpoints() {
    for composite in [false, true] {
        let mut doc = Document::default();
        let id = if composite {
            doc.add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![p(2., -2.), p(8., -2.), p(8., -2.2), p(-2.4, -2.2)],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap()
        } else {
            let id = doc
                .add_geometry(Geometry::Line(line(p(2., -2.), p(8., -2.))))
                .unwrap();
            doc.add_geometry(Geometry::Line(line(p(2.8, -2.2), p(3.2, -2.2))))
                .unwrap();
            id
        };
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_modes(
                &doc,
                [2.8, -2.],
                0.5,
                |p| Some([p.x(), p.y()]),
                only_mid(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(snap.object_id(), id);
        assert_eq!(snap.point(), p(5., -2.));
    }
}

#[test]
#[ignore = "manual release-mode Mid hover timing diagnostic"]
fn mid_hover_scene_timing() {
    for shape in ["line", "nurbs", "brep"] {
        for count in [1, 100, 1000] {
            let mut doc = Document::default();
            for index in 0..count {
                let offset = 20. * index as Real;
                let geometry = match shape {
                    "line" => Geometry::Line(line(p(offset + 2., -4.), p(offset + 8., -4.))),
                    "nurbs" => Geometry::NurbsCurve(
                        NurbsCurve::try_new(
                            2,
                            vec![
                                p(offset + 2., -4.),
                                p(offset + 3., -4.),
                                p(offset + 8., -4.),
                            ],
                            vec![0., 0., 0., 1., 1., 1.],
                        )
                        .unwrap(),
                    ),
                    _ => Geometry::Brep(
                        Brep::try_box(
                            viboceros_geometry::Frame3::try_from_points(
                                p(0., 0.),
                                p(1., 0.),
                                p(0., 1.),
                                Tolerance::DEFAULT,
                            )
                            .unwrap(),
                            [[offset + 2., offset + 8.], [-8., -4.], [0., 1.]],
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                    ),
                };
                doc.add_geometry(geometry).unwrap();
            }
            let mut cache = ObjectSnapCache::default();
            let mut run = || hit(&mut cache, &doc, p(2.8, -4.), only_mid());
            assert!(run().is_some());
            let start = std::time::Instant::now();
            for _ in 0..100 {
                std::hint::black_box(run());
            }
            eprintln!(
                "Mid hover, {shape}, {count} objects: {:?}/query",
                start.elapsed() / 100
            );
        }
    }
}
