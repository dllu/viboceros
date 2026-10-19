use std::collections::BTreeSet;
use viboceros_geometry::{
    CurveSegment3, GeometryError, LineSegment, NurbsCurve, Point3, PolyCurve3, Tolerance, Vector3,
    WeightedPoint3,
};
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
#[test]
fn shared_grips_preserve_rational_shape_and_parent_parameters_with_independent_midpoint() {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let nurbs = NurbsCurve::try_new_rational(
        2,
        [(p(1., 0.), 1.), (p(1., 1.), w), (p(0., 1.), 1.)]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let line = LineSegment::try_new(p(0., 1.), p(2., 1.), Tolerance::DEFAULT).unwrap();
    let source = PolyCurve3::try_with_segment_domains(
        vec![CurveSegment3::NurbsCurve(nurbs), CurveSegment3::Line(line)],
        vec![-3., 7., 11.],
    )
    .unwrap();
    let original = source.clone();
    let indices = BTreeSet::from([1]);
    assert_eq!(source.with_mapped_grips(&indices, Ok).unwrap(), source);
    let result = source
        .with_mapped_grips(&indices, |p| {
            p.translated(Vector3::try_new(1., 0., 0.).unwrap())
        })
        .unwrap();
    let expected = p((1. + 4. * w) / (2. + 2. * w), (1. + 2. * w) / (2. + 2. * w));
    assert!(result.evaluate(2.).unwrap().distance_to(expected).unwrap() < 1e-12);
    assert_eq!(result.parameters(), source.parameters());
    assert_eq!(result.evaluate(11.).unwrap(), p(2., 1.));
    assert_eq!(source, original);
    assert!(matches!(
        source.with_mapped_grips(&[4].into_iter().collect(), Ok),
        Err(GeometryError::InvalidControlPointIndex { index: 4, count: 4 })
    ));
}
#[test]
fn nonadjacent_identical_locations_remain_independent_grips() {
    let first = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(2., 1.), p(0., 0.)],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let second = LineSegment::try_new(p(0., 0.), p(3., 0.), Tolerance::DEFAULT).unwrap();
    let source = PolyCurve3::try_new(vec![
        CurveSegment3::NurbsCurve(first),
        CurveSegment3::Line(second),
    ])
    .unwrap();
    assert_eq!(
        source.extract_point_locations().unwrap(),
        vec![p(0., 0.), p(2., 1.), p(0., 0.), p(3., 0.)]
    );
    let moved = source
        .with_mapped_grips(&[0].into_iter().collect(), |_| Ok(p(-1., 0.)))
        .unwrap();
    assert_eq!(
        moved.extract_point_locations().unwrap(),
        vec![p(-1., 0.), p(2., 1.), p(0., 0.), p(3., 0.)]
    );
}
