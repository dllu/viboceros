use super::*;

#[test]
fn batch_preserves_query_order_and_rejects_invalid_inputs() {
    let w = 0.5_f64.sqrt();
    let source = [[1., 0., 0., 1.], [1., 1., 0., w], [0., 1., 0., 1.]];
    let before = source;
    let knots = [0., 0., 0., 1., 1., 1.];
    let actual = curve_tangents(2, &source, &knots, &[1., 0., 0.5]).unwrap();
    for (actual, expected) in actual
        .into_iter()
        .zip([[-1., 0., 0.], [0., 1., 0.], [-w, w, 0.]])
    {
        for (a, b) in actual.unwrap().into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-15);
        }
    }
    assert_eq!(source, before);
    assert_eq!(curve_tangents(2, &source, &knots, &[]).unwrap(), vec![]);
    for params in [vec![f64::NAN], vec![-1.], vec![2.]] {
        assert!(curve_tangents(2, &source, &knots, &params).is_err());
    }
    for degree in [0, 3, 65] {
        assert!(curve_tangents(degree, &source, &knots, &[0.]).is_err());
    }
    let mut invalid = source;
    invalid[0][3] = 0.;
    assert!(curve_tangents(2, &invalid, &knots, &[0.]).is_err());
    assert!(curve_tangents(2, &source, &[0., 0., 1., 0., 1., 1.], &[0.]).is_err());
}

#[test]
fn a_curve_rejected_by_the_native_constructor_returns_an_error() {
    assert!(curve_tangents(1, &[[2., 3., 4., 1.]; 2], &[0., 0., 1., 1.], &[0., 0.5, 1.]).is_err());
}
