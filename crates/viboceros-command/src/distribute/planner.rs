//! One-dimensional spacing plans, independent of selection and document edits.

use super::Mode;
use viboceros_geometry::{GeometryError, Vector3};

// The caller supplies at least three finite intervals in leading-edge order.
pub(super) fn offsets(
    intervals: &[[f64; 2]],
    mode: Mode,
    explicit: Option<f64>,
) -> Result<(Vec<f64>, f64), GeometryError> {
    let center = |[a, b]: [f64; 2]| a.midpoint(b);
    let count = intervals.len();
    let divisor = (count - 1) as f64;
    let spacing = explicit.unwrap_or_else(|| match mode {
        Mode::Center => {
            divided_difference(center(intervals[count - 1]), center(intervals[0]), divisor)
        }
        Mode::Gap => {
            // Average existing gaps avoids subtracting two large total widths.
            let mut sum = 0.;
            let mut correction = 0.;
            for pair in intervals.windows(2) {
                let gap = divided_difference(pair[1][0], pair[0][1], divisor);
                let adjusted = gap - correction;
                let next = sum + adjusted;
                correction = (next - sum) - adjusted;
                sum = next;
            }
            sum
        }
    });
    let mut offsets = vec![0.; count];
    let mut next_min = intervals[0][1] + spacing;
    for i in 1..count {
        offsets[i] = match mode {
            Mode::Center => spacing.mul_add(i as f64, center(intervals[0])) - center(intervals[i]),
            Mode::Gap => next_min - intervals[i][0],
        };
        if mode == Mode::Gap && i + 1 < count {
            // Advance from the translated far end rather than materializing
            // an interval width, which may overflow for finite endpoints.
            // The compensated sum also permits cancellation with spacing.
            next_min = Vector3::try_new(intervals[i][1], offsets[i], spacing)?
                .dot(Vector3::try_new(1., 1., 1.)?)?;
        }
    }
    if explicit.is_none() {
        offsets[count - 1] = 0.;
    }
    // Reuse the validated scalar/vector boundary; no nonfinite displacement
    // is allowed to reach document geometry or to be hidden by a no-op.
    Vector3::try_new(spacing, 0., 0.)?;
    for value in &offsets {
        Vector3::try_new(*value, 0., 0.)?;
    }
    Ok((offsets, spacing))
}

// Keep ordinary subtraction rounding, but scale first when the difference
// alone overflows. Distribution has at least three units, so divisor >= 2.
fn divided_difference(a: f64, b: f64, divisor: f64) -> f64 {
    let difference = a - b;
    if difference.is_finite() {
        difference / divisor
    } else {
        a / divisor - b / divisor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_match_independent_integer_layout_equations() {
        let mut state = 0x3b76_8d24_69af_102eu64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state >> 32) as i64
        };
        for count in [3, 5, 9] {
            // A common integer denominator represents centers and automatic
            // spacing exactly. Power-of-two counts-minus-one keep every
            // expected result exactly representable in binary floating point.
            let denominator = 2 * (count - 1) as i64;
            for _ in 0..512 {
                let mut starts = (0..count).map(|_| next() % 41 - 20).collect::<Vec<_>>();
                starts.sort_unstable();
                let intervals = starts
                    .iter()
                    .map(|start| [*start * denominator, (*start + next() % 11) * denominator])
                    .collect::<Vec<_>>();
                for mode in [Mode::Gap, Mode::Center] {
                    for explicit in [None, Some(-3), Some(0), Some(5)] {
                        let center = |[a, b]: [i64; 2]| (a + b) / 2;
                        let total_width: i64 = intervals.iter().map(|[a, b]| b - a).sum();
                        let spacing = explicit.map(|value| value * denominator).unwrap_or_else(
                            || match mode {
                                Mode::Center => {
                                    (center(intervals[count - 1]) - center(intervals[0]))
                                        / (count - 1) as i64
                                }
                                Mode::Gap => {
                                    (intervals[count - 1][1] - intervals[0][0] - total_width)
                                        / (count - 1) as i64
                                }
                            },
                        );
                        let expected = (0..count)
                            .map(|i| match mode {
                                Mode::Center => {
                                    center(intervals[0]) + i as i64 * spacing - center(intervals[i])
                                }
                                Mode::Gap => {
                                    intervals[0][0]
                                        + intervals[..i].iter().map(|[a, b]| b - a).sum::<i64>()
                                        + i as i64 * spacing
                                        - intervals[i][0]
                                }
                            })
                            .collect::<Vec<_>>();
                        for exponent in [-500, 0, 500] {
                            let scale = 2_f64.powi(exponent);
                            let to_real = |value: i64| value as f64 / denominator as f64 * scale;
                            let floating = intervals
                                .iter()
                                .map(|pair| pair.map(to_real))
                                .collect::<Vec<_>>();
                            let (actual, actual_spacing) = offsets(
                                &floating,
                                mode,
                                explicit.map(|value| value as f64 * scale),
                            )
                            .unwrap();
                            assert_eq!(actual_spacing, to_real(spacing));
                            assert_eq!(
                                actual,
                                expected.iter().copied().map(to_real).collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gap_planner_does_not_require_representable_interval_widths() {
        for huge in [2_f64.powi(1023), f64::MAX] {
            let intervals = [[-huge; 2], [-huge, huge], [huge; 2], [huge; 2]];
            for spacing in [None, Some(0.)] {
                let (offsets, actual_spacing) =
                    super::offsets(&intervals, Mode::Gap, spacing).unwrap();
                assert_eq!(actual_spacing, 0.);
                assert_eq!(offsets, [0.; 4]);
            }
        }
    }

    #[test]
    fn spacing_plans_retain_finite_extreme_gaps_and_subnormal_centers() {
        for huge in [2_f64.powi(1023), f64::MAX] {
            let (offsets, spacing) =
                super::offsets(&[[-huge; 2], [0.; 2], [huge; 2]], Mode::Center, None).unwrap();
            assert_eq!(spacing, huge);
            assert_eq!(offsets, [0.; 3]);
            let (offsets, spacing) =
                super::offsets(&[[-huge; 2], [huge; 2], [huge; 2]], Mode::Gap, None).unwrap();
            assert_eq!(spacing, huge);
            assert_eq!(offsets, [0., -huge, 0.]);
        }
        let intervals = [1, 3, 5].map(|bits| [f64::from_bits(bits); 2]);
        let (offsets, spacing) = super::offsets(&intervals, Mode::Center, None).unwrap();
        assert_eq!(spacing, f64::from_bits(2));
        assert_eq!(offsets, [0.; 3]);
        for mode in [Mode::Center, Mode::Gap] {
            assert!(super::offsets(&[[0.; 2], [1.; 2], [2.; 2]], mode, Some(f64::MAX)).is_err());
        }
    }

    #[test]
    fn spacing_plans_use_leading_edge_order_and_pin_automatic_end_objects() {
        let intervals = [[0., 2.], [5., 9.], [16., 17.]];
        for (mode, spacing, expected) in [
            (Mode::Center, None, vec![0., 1.75, 0.]),
            (Mode::Gap, None, vec![0., 2., 0.]),
            (Mode::Center, Some(3.), vec![0., -3., -9.5]),
            (Mode::Gap, Some(3.), vec![0., 0., -4.]),
            (Mode::Center, Some(0.), vec![0., -6., -15.5]),
            (Mode::Gap, Some(-3.), vec![0., -6., -16.]),
        ] {
            let (actual, _) = offsets(&intervals, mode, spacing).unwrap();
            assert_eq!(actual, expected);
        }
    }
}
