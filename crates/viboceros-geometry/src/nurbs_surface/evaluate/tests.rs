use crate::{
    AffineTransform3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};

mod jets;
mod sides;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn rational_plane() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (p(0.0, 0.0, 0.0), 1.0),
            (p(4.0, 0.0, 0.0), 2.0),
            (p(0.0, 3.0, 0.0), 1.0),
            (p(4.0, 3.0, 0.0), 2.0),
        ]
        .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap()
}

fn near(actual: Vector3, expected: [f64; 3]) {
    for (a, e) in actual.to_array().into_iter().zip(expected) {
        assert!((a - e).abs() < 2e-12, "{a:e} != {e:e}");
    }
}

#[test]
fn rational_surface_derivatives_are_invariant_under_large_translation() {
    let source = rational_plane();
    let offset = Vector3::try_new(1e12, -2e12, 3e12).unwrap();
    let translated = source
        .transformed(AffineTransform3::from_translation(offset))
        .unwrap();
    for (u, v) in [(0.25, 0.625), (1.0 / 3.0, 0.4), (0.75, 0.2)] {
        let expected = source.evaluate_with_derivatives(u, v).unwrap();
        let actual = translated.evaluate_with_derivatives(u, v).unwrap();
        near(actual.1, [8.0 / (1.0 + u).powi(2), 0.0, 0.0]);
        near(actual.2, [0.0, 3.0, 0.0]);
        assert_eq!(actual.1, expected.1);
        assert_eq!(actual.2, expected.2);
        assert_eq!(actual.0, expected.0.translated(offset).unwrap());
    }
}

#[test]
fn a_far_away_constant_rational_surface_has_exactly_zero_partials() {
    let point = p(1e12, -2e12, 3e12);
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [1.0, 2.0, 3.0, 4.0]
            .map(|w| WeightedPoint3::try_new(point, w).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let actual = surface.evaluate_with_derivatives(1.0 / 3.0, 0.4).unwrap();
    assert_eq!(actual.0, point);
    assert_eq!(actual.1.to_array(), [0.0; 3]);
    assert_eq!(actual.2.to_array(), [0.0; 3]);
    assert!(
        surface
            .normal_at(1.0 / 3.0, 0.4, Tolerance::DEFAULT)
            .is_err()
    );
}

#[test]
fn endpoint_knots_straddling_the_active_domain_choose_nonempty_spans() {
    for knots in [
        vec![-1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
        vec![-1.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0, 3.0],
    ] {
        let row = [
            p(-9.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(2.0, 0.0, 0.0),
            p(9.0, 0.0, 0.0),
        ];
        let reference = NurbsCurve::try_new(2, row.to_vec(), knots.clone()).unwrap();
        let surface = NurbsSurface::try_new_rational(
            2,
            1,
            5,
            2,
            row.into_iter()
                .chain(row.map(|p| {
                    p.translated(Vector3::try_new(0.0, 3.0, 0.0).unwrap())
                        .unwrap()
                }))
                .map(|p| WeightedPoint3::try_new(p, 1.0).unwrap())
                .collect(),
            knots,
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        for u in [0.0, 0.5, 1.0, 1.5, 2.0] {
            let expected = reference.evaluate_with_derivative(u).unwrap();
            let actual = surface.evaluate_with_derivatives(u, 0.4).unwrap();
            assert!(
                actual
                    .0
                    .distance_to(
                        expected
                            .0
                            .translated(Vector3::try_new(0.0, 1.2, 0.0).unwrap())
                            .unwrap()
                    )
                    .unwrap()
                    < 2e-12
            );
            near(actual.1, expected.1.to_array());
            near(actual.2, [0.0, 3.0, 0.0]);
        }
    }
}
