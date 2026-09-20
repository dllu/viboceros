use super::*;
use crate::nurbs::exact::{Rational, rational};
use num_traits::Zero;

fn p(values: [Real; 3]) -> Point3 {
    Point3::try_from(values).unwrap()
}

fn check_bounds(target: Point3, point: Point3) {
    let exact: Rational = target
        .to_array()
        .into_iter()
        .zip(point.to_array())
        .map(|(a, b)| {
            let d = rational(a) - rational(b);
            &d * &d
        })
        .fold(Rational::zero(), |sum, square| sum + square);
    let bounds = PointDistance::new(target, point);
    assert!(bounds.lower.is_finite() && bounds.lower >= 0.);
    assert!(!bounds.upper.is_nan() && bounds.upper >= bounds.lower);
    assert!(
        rational(bounds.lower) <= exact,
        "{target:?}, {point:?}, {bounds:?}"
    );
    if bounds.upper.is_finite() {
        assert!(
            exact <= rational(bounds.upper),
            "{target:?}, {point:?}, {bounds:?}"
        );
    }
}

#[test]
fn candidate_distance_bounds_enclose_independent_exact_squared_distances() {
    let values = [
        -Real::MAX,
        -1e200,
        -1.,
        -Real::MIN_POSITIVE,
        -Real::from_bits(1),
        -0.,
        0.,
        Real::from_bits(1),
        Real::MIN_POSITIVE,
        1.,
        1e200,
        Real::MAX,
    ];
    for (i, &a) in values.iter().enumerate() {
        for (j, &b) in values.iter().enumerate() {
            check_bounds(p([a, b, values[(i + j) % values.len()]]), p([b, a, 0.]));
        }
    }
    let mut state = 419_u64;
    for _ in 0..1024 {
        let mut number = || loop {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = Real::from_bits(state);
            if x.is_finite() {
                break x;
            }
        };
        let target = p(std::array::from_fn(|_| number()));
        let a = p(std::array::from_fn(|_| number()));
        let b = p(std::array::from_fn(|_| number()));
        check_bounds(target, a);
        let ca = PointDistance::new(target, a);
        let cb = PointDistance::new(target, b);
        assert_eq!(ca.compare(&cb, target), target.compare_distances(a, b));
    }
}

#[test]
fn candidate_order_uses_disjoint_bounds_and_preserves_exact_ties() {
    let target = p([0.; 3]);
    let near = PointDistance::new(target, p([1., 0., 0.]));
    let far = PointDistance::new(target, p([2., 0., 0.]));
    assert!(near.upper < far.lower);
    assert_eq!(near.compare(&far, target), Ordering::Less);
    assert_eq!(far.compare(&near, target), Ordering::Greater);
    let mut candidates = [
        p([1., 0., 0.]),
        p([0., -1., 0.]),
        p([-1., 0., 0.]),
        p([0., 0., 1.]),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, point)| (i, PointDistance::new(target, point)))
    .collect::<Vec<_>>();
    candidates.sort_by(|a, b| a.1.compare(&b.1, target));
    assert_eq!(
        candidates.iter().map(|c| c.0 as Real).collect::<Vec<_>>(),
        vec![0., 1., 2., 3.]
    );
}
