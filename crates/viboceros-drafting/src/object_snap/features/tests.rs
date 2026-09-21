use super::*;
use viboceros_document::Geometry;
use viboceros_geometry::{
    Circle3, CircularArc3, CurveSegment3, Ellipse3, LineSegment, NurbsCurve, PolyCurve3, Polyline3,
    Tolerance, Vector3,
};

#[test]
fn every_mode_subset_preserves_enabled_feature_order_and_values() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let t = Tolerance::DEFAULT;
    let axis = |x, y, z| {
        Vector3::try_new(x, y, z)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
    };
    let circle = Circle3::try_new(p(0., 0.), 2., axis(0., 0., 1.), t).unwrap();
    let fixtures = [
        (
            Geometry::Line(LineSegment::try_new(p(0., 0.), p(2., 0.), t).unwrap()),
            vec![End, End, Mid],
        ),
        (Geometry::Circle(circle), vec![Mid, Quad, Quad, Quad, Quad]),
        (
            Geometry::Arc(
                CircularArc3::try_from_three_points(p(0., 0.), p(1., 1.), p(2., 0.), t).unwrap(),
            ),
            vec![End, End, Mid],
        ),
        (
            Geometry::Ellipse(
                Ellipse3::try_new(p(0., 0.), 2., 1., axis(1., 0., 0.), axis(0., 1., 0.), t)
                    .unwrap(),
            ),
            vec![Mid, Quad, Quad, Quad, Quad],
        ),
        (
            Geometry::Polyline(
                Polyline3::try_new(vec![p(0., 0.), p(2., 0.), p(2., 2.)], t).unwrap(),
            ),
            vec![End, End, End, Mid, Mid],
        ),
        (
            Geometry::NurbsCurve(
                NurbsCurve::try_new(1, vec![p(0., 0.), p(2., 0.)], vec![0., 0., 1., 1.]).unwrap(),
            ),
            vec![End, End],
        ),
        // A composite stores a circle as a full arc, with End/Mid but no Quad.
        (
            Geometry::PolyCurve(
                PolyCurve3::try_new(vec![CurveSegment3::Arc(
                    CircularArc3::try_from_circle_sweep(circle, std::f64::consts::TAU).unwrap(),
                )])
                .unwrap(),
            ),
            vec![End, End, Mid],
        ),
    ];
    let kinds = [
        ObjectSnapKind::Point,
        End,
        Mid,
        ObjectSnapKind::Center,
        Quad,
    ];
    for (geometry, labels) in fixtures {
        let source = geometry.curve_ref().unwrap();
        let mut all = Vec::new();
        curve(source, ObjectSnapModes::ALL, &mut |k, p| all.push((k, p)));
        assert_eq!(all.iter().map(|(k, _)| *k).collect::<Vec<_>>(), labels);
        for mask in 0..32 {
            let modes = kinds
                .iter()
                .enumerate()
                .fold(ObjectSnapModes::NONE, |m, (i, &k)| {
                    m.with(k, mask & (1 << i) != 0)
                });
            let expected: Vec<_> = all
                .iter()
                .copied()
                .filter(|(k, _)| modes.contains(*k))
                .collect();
            let mut actual = Vec::new();
            curve(source, modes, &mut |k, p| actual.push((k, p)));
            assert_eq!(actual, expected, "mask {mask} geometry {geometry:?}");
        }
    }
}
