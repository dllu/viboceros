use super::*;
use crate::{AffineTransform3, Point3, Vector3, WeightedPoint3};
fn normal() -> UnitVector3 {
    Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
}
fn circle() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            [3., 0.],
            [3., 3.],
            [0., 3.],
            [-3., 3.],
            [-3., 0.],
            [-3., -3.],
            [0., -3.],
            [3., -3.],
            [3., 0.],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, [x, y])| {
            WeightedPoint3::try_new(
                Point3::try_new(x, y, 0.).unwrap(),
                2. * if i % 2 == 0 { 1. } else { 0.5_f64.sqrt() },
            )
            .unwrap()
        })
        .collect(),
        vec![0., 0., 0., 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1., 1., 1.],
    )
    .unwrap()
}
#[test]
fn a_circular_strip_retains_gauges_and_has_linear_radial_rulings() {
    let source = circle();
    let original = source.clone();
    let b = try_smooth_ribbon(&source, -0.5, false, normal(), Tolerance::DEFAULT).unwrap();
    assert_eq!(b.faces().len(), 1);
    let s = b.faces()[0].surface();
    for (row, p) in s
        .control_points()
        .chunks_exact(2)
        .zip(source.control_points())
    {
        assert_eq!(row[0].weight(), p.weight());
        assert_eq!(row[1].weight(), p.weight());
    }
    for u in [0., 0.2, 0.5] {
        for v in [0.1, 0.3, 0.6, 0.9] {
            let radius = s
                .evaluate(u, v)
                .unwrap()
                .distance_to(Point3::try_new(0., 0., 0.).unwrap())
                .unwrap();
            assert!((radius - (3. + u)).abs() < 1e-12);
        }
    }
    assert_eq!(source, original);
}
#[test]
fn a_rational_line_retains_its_nonuniform_clock_and_translates_exactly() {
    let source = NurbsCurve::try_new_rational(
        3,
        [(0., 2.), (0.2, 0.7), (4., 1.3), (5., 3.)]
            .into_iter()
            .map(|(x, w)| WeightedPoint3::try_new(Point3::try_new(x, 0., 0.).unwrap(), w).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .try_reparameterized(-4. ..=8.)
    .unwrap();
    let b = try_smooth_ribbon(&source, 0.15, false, normal(), Tolerance::DEFAULT).unwrap();
    let s = b.faces()[0].surface();
    assert_eq!(s.domain_v(), source.domain());
    let expected = source
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 0.15, 0.).unwrap(),
        ))
        .unwrap();
    assert!(
        s.isocurve_v(*s.domain_u().end())
            .unwrap()
            .parameter_curve_deviation_bound(&expected, 1e-12)
            .unwrap()
            .is_some()
    );
}
#[test]
fn invalid_distances_and_collapsed_circular_offsets_fail_before_ruled_construction() {
    let source = circle();
    for distance in [0., f64::NAN, 3., 4.] {
        let result = try_smooth_ribbon_with_ruled(
            &source,
            distance,
            false,
            normal(),
            Tolerance::DEFAULT,
            &mut |_, _| -> Result<NurbsSurface, GeometryError> {
                panic!("invalid offset reached ruled backend")
            },
        );
        assert!(result.is_err());
    }
}
