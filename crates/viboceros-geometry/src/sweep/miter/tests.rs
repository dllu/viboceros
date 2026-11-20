use super::*;
use crate::{Circle3, LineSegment, Polyline3, SurfaceIso};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn sweep_miters_join_a_rational_circle_without_sampling_the_trim() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let path = Polyline3::try_new(
        vec![point(0., 0., 0.), point(5., 0., 0.), point(5., 5., 0.)],
        tolerance,
    )
    .unwrap();
    let origin = point(0., 0., 0.);
    let normal = Vector3::try_new(1., 0., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let profile = Circle3::try_from_center_point(origin, point(0., 1.5, 0.), normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let guide = CurveRef::Polyline(&path);
    for untrimmed in [false, true] {
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve: profile.clone(),
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            untrimmed,
            tolerance,
        )
        .unwrap()
        .unwrap();
        assert_eq!(brep.faces().len(), 2);
        let middle = brep
            .edges()
            .iter()
            .find(|edge| {
                let ends = edge.vertices().map(|v| brep.vertices()[v].point());
                ends[0] == ends[1] && ends[0].distance_to(point(3.5, 1.5, 0.)).unwrap() < 1e-8
            })
            .expect("shared rational miter edge");
        for index in 0..=32 {
            let t = middle.curve().parameter_at(index as Real / 32.).unwrap();
            let p = middle.curve().evaluate(t).unwrap();
            assert!((p.x() + p.y() - 5.).abs() < 1e-9);
            assert!((p.y().hypot(p.z()) - 1.5).abs() < 1e-9);
        }
        let non_iso = brep
            .faces()
            .iter()
            .flat_map(|face| face.loops())
            .flat_map(|l| l.trims())
            .any(|trim| trim.iso() == SurfaceIso::NotIso);
        assert_eq!(non_iso, !untrimmed);
    }
}

#[test]
fn sweep_miter_sewing_matches_native_spatial_and_closed_polyline_areas() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    for (points, native_area, faces, edges) in [
        (
            vec![
                point(0., 0., 0.),
                point(5., 0., 0.),
                point(5., 5., 0.),
                point(5., 5., 5.),
            ],
            22.53469547164993,
            3,
            10,
        ),
        (
            vec![
                point(0., 0., 0.),
                point(5., 0., 0.),
                point(5., 5., 0.),
                point(0., 5., 0.),
                point(0., 0., 0.),
            ],
            25.238858928247906,
            4,
            12,
        ),
    ] {
        let rail = Polyline3::try_new(points, tolerance).unwrap();
        let guide = CurveRef::Polyline(&rail);
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve: LineSegment::try_new(point(0., 0., 0.), point(0., 1.5, 1.), tolerance)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            false,
            tolerance,
        )
        .unwrap()
        .unwrap();
        assert_eq!((brep.faces().len(), brep.edges().len()), (faces, edges));
        let area = brep.area(tolerance).unwrap();
        assert!((area - native_area).abs() < 1e-7, "{area}");
    }
}
