use super::*;
use crate::{CircularArc3, LineSegment};

#[derive(Debug, PartialEq)]
enum Failure {
    Geometry(GeometryError),
    Producer,
}
impl From<GeometryError> for Failure {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn curve(points: Vec<Point3>, knots: Vec<Real>) -> NurbsCurve {
    NurbsCurve::try_new(2, points, knots).unwrap()
}

#[test]
fn candidate_measurement_and_final_extraction_failures_remain_producer_errors() {
    let first = curve(
        vec![p(0., 0.), p(2., 0.), p(2., 2.)],
        vec![0., 0., 0., 1., 1., 1.],
    );
    let alternatives = [
        CurveSegment3::Line(
            LineSegment::try_new(p(2., 2.), p(6., 2.), Tolerance::DEFAULT).unwrap(),
        ),
        CurveSegment3::Arc(
            CircularArc3::try_from_three_points(
                p(2., 2.),
                p(2. + 2_f64.sqrt(), 4. - 2_f64.sqrt()),
                p(4., 4.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        CurveSegment3::NurbsCurve(curve(
            vec![p(2., 2.), p(4., 2.), p(4., 4.)],
            vec![0., 0., 0., 1., 1., 1.],
        )),
    ];
    for second in alternatives {
        for nth in [1, 2] {
            let source = PolyCurve3::try_new(vec![
                CurveSegment3::NurbsCurve(first.clone()),
                second.clone(),
            ])
            .unwrap();
            let original = source.clone();
            let mut count = 0;
            let mut trim = |curve: &NurbsCurve, range| {
                count += 1;
                if count == nth {
                    Err(Failure::Producer)
                } else {
                    Ok(curve.try_trimmed(range)?)
                }
            };
            assert_eq!(
                source.try_fillet_corners_with_nurbs(0.5, Tolerance::DEFAULT, &mut trim),
                Err(Failure::Producer)
            );
            assert_eq!(count, nth);
            assert_eq!(source, original);
        }
    }
}

#[test]
fn sharp_knot_extraction_is_delegated_and_invalid_radius_stays_a_geometry_error() {
    let leaf = curve(
        vec![p(0., 0.), p(2., 0.), p(2., 2.), p(4., 2.), p(4., 4.)],
        vec![0., 0., 0., 1., 1., 2., 2., 2.],
    );
    let source = PolyCurve3::try_new(vec![CurveSegment3::NurbsCurve(leaf)]).unwrap();
    let mut count = 0;
    let mut trim = |_: &NurbsCurve, _| {
        count += 1;
        Err(Failure::Producer)
    };
    assert!(matches!(
        source.try_fillet_corners_with_nurbs(0., Tolerance::DEFAULT, &mut trim),
        Err(Failure::Geometry(_))
    ));
    assert_eq!(count, 0);
    let mut trim = |_: &NurbsCurve, _| Err(Failure::Producer);
    assert_eq!(
        source.try_fillet_corners_with_nurbs(0.5, Tolerance::DEFAULT, &mut trim),
        Err(Failure::Producer)
    );
}

#[test]
fn smooth_unselected_straight_leaves_keep_original_classes_and_parameter_domains() {
    let prefix = CurveSegment3::NurbsCurve(
        NurbsCurve::try_new(1, vec![p(-1., 0.), p(0., 0.)], vec![-20., -20., -10., -10.]).unwrap(),
    );
    let before = CurveSegment3::NurbsCurve(curve(
        vec![p(0., 0.), p(2., 0.), p(2., 2.)],
        vec![0., 0., 0., 1., 1., 1.],
    ));
    let after = CurveSegment3::NurbsCurve(curve(
        vec![p(2., 2.), p(4., 2.), p(4., 4.)],
        vec![0., 0., 0., 1., 1., 1.],
    ));
    let suffix = CurveSegment3::Polyline(
        crate::Polyline3::try_new(vec![p(4., 4.), p(4., 5.), p(4., 6.)], Tolerance::DEFAULT)
            .unwrap(),
    )
    .try_reparameterized(40. ..=80.)
    .unwrap();
    let source = PolyCurve3::try_new(vec![prefix, before, after, suffix]).unwrap();
    for source in [source.clone(), source.reversed().unwrap()] {
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.segments().first(), source.segments().first());
        assert_eq!(result.segments().last(), source.segments().last());
    }
}
