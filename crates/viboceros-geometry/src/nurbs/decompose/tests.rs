use super::*;
use crate::{CurveEvaluationSide, Point3, WeightedPoint3};

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn compare_parts(source: &NurbsCurve, expected_domains: &[[Real; 2]]) -> Vec<NurbsCurve> {
    let parts = source.try_split_at_full_order_knots().unwrap();
    assert_eq!(parts.len(), expected_domains.len());
    for (part, &[a, b]) in parts.iter().zip(expected_domains) {
        assert_eq!(part.domain(), a..=b);
        assert_eq!(part.full_order_knots().count(), 0);
        for i in 0..=32 {
            let t = part.parameter_at(i as Real / 32.0).unwrap();
            let side = if i == 32 {
                CurveEvaluationSide::Left
            } else {
                CurveEvaluationSide::Right
            };
            let actual = part.evaluate_with_second_derivative(t).unwrap();
            let expected = source
                .evaluate_with_second_derivative_on_side(t, side)
                .unwrap();
            assert!(actual.0.distance_to(expected.0).unwrap() < 2e-13);
            for (a, b) in actual
                .1
                .to_array()
                .into_iter()
                .chain(actual.2.to_array())
                .zip(
                    expected
                        .1
                        .to_array()
                        .into_iter()
                        .chain(expected.2.to_array()),
                )
            {
                assert!((a - b).abs() < 2e-12);
            }
        }
    }
    parts
}

#[test]
fn full_order_blocks_keep_extreme_independent_weights_and_control_points() {
    for gap in [0.0, 10.0] {
        let controls = [
            (0.0, 0.0),
            (1.0, 1.0),
            (2.0, 0.0),
            (2.0 + gap, 0.0),
            (3.0 + gap, 2.0),
            (4.0 + gap, 0.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let scale = if i < 3 {
                2.0_f64.powi(-700)
            } else {
                -2.0_f64.powi(700)
            };
            WeightedPoint3::try_new(point(x, y), scale * if i % 3 == 1 { 0.5 } else { 1.0 })
                .unwrap()
        })
        .collect::<Vec<_>>();
        let source = NurbsCurve::try_new_rational(
            2,
            controls.clone(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        assert_eq!(source.full_order_knots().collect::<Vec<_>>(), [1.0]);
        let parts = compare_parts(&source, &[[0.0, 1.0], [1.0, 2.0]]);
        assert_eq!(parts[0].control_points(), &controls[..3]);
        assert_eq!(parts[1].control_points(), &controls[3..]);
    }
}

#[test]
fn closed_decomposition_keeps_the_original_seam_and_parameter_order() {
    let source = NurbsCurve::try_new(
        1,
        vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(0.0, 1.0),
            point(0.0, 0.0),
        ],
        vec![-7.0, -7.0, 0.0, 0.0, 2.0, 2.0, 13.0, 13.0],
    )
    .unwrap();
    assert!(source.is_closed().unwrap());
    compare_parts(&source, &[[-7.0, 0.0], [0.0, 2.0], [2.0, 13.0]]);
}

#[test]
fn unclamped_ends_are_clamped_without_changing_the_active_locus() {
    let source = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(2.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 3.0),
            point(4.0, 0.0),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 1.0, 1.0, 2.0, 3.0, 4.0],
    )
    .unwrap();
    let parts = compare_parts(&source, &[[0.0, 1.0], [1.0, 2.0]]);
    for part in parts {
        assert!(
            part.knots()[..=2]
                .iter()
                .all(|k| k == part.domain().start())
        );
        assert!(part.knots()[3..].iter().all(|k| k == part.domain().end()));
    }
}

#[test]
fn sources_without_full_order_knots_are_unchanged() {
    let source = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
            point(0.0, 0.0),
            point(1.0, 0.0),
        ],
        vec![-1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    assert_eq!(
        source.try_split_at_full_order_knots().unwrap(),
        vec![source]
    );
}

#[test]
fn many_independent_blocks_are_sliced_once_in_parameter_order() {
    let count = 1024;
    let source = NurbsCurve::try_new(
        1,
        (0..count)
            .flat_map(|i| [point(i as Real, 0.0), point(i as Real + 1.0, 0.0)])
            .collect(),
        (0..=count).flat_map(|i| [i as Real; 2]).collect(),
    )
    .unwrap();
    let parts = source.try_split_at_full_order_knots().unwrap();
    assert_eq!(parts.len(), count);
    for (i, part) in parts.iter().enumerate() {
        assert_eq!(part.domain(), i as Real..=i as Real + 1.0);
        assert_eq!(
            part.control_points(),
            &source.control_points()[2 * i..2 * i + 2]
        );
    }
}
