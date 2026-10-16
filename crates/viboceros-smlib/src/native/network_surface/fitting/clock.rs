//! Private C2 arc-length-to-output proposals; these never certify geometry.
use super::*;

pub(super) struct Clock {
    pieces: Vec<IntervalClock>,
    /// Output-parameter derivative with respect to local model arc length.
    pub slopes: Vec<f64>,
}

enum IntervalClock {
    Whole(NurbsCurve),
    Local {
        left: NurbsCurve,
        right: NurbsCurve,
        a: f64,
        delta: f64,
        width: f64,
    },
}

fn quintic(a: f64, b: f64, d0: f64, d1: f64) -> Result<NurbsCurve, Error> {
    let left = d0 / 5.;
    let right = d1 / 5.;
    let controls = [a, a + left, a + 2. * left, b - 2. * right, b - right, b];
    if controls.iter().any(|s| !s.is_finite()) || controls.windows(2).any(|p| p[0] >= p[1]) {
        return Err(invalid("nonmonotone stored arc-length clock"));
    }
    Ok(NurbsCurve::try_new(
        5,
        controls
            .into_iter()
            .map(|x| Point3::try_new(x, 0., 0.))
            .collect::<Result<Vec<_>, _>>()?,
        vec![0., 0., 0., 0., 0., 0., 1., 1., 1., 1., 1., 1.],
    )?)
}

impl Clock {
    pub fn new(
        stations: &[CurveFitStation],
        lengths: &[f64],
        policy: GordonCurveFitClock,
    ) -> Result<Self, Error> {
        let secants = stations
            .windows(2)
            .zip(lengths)
            .map(|(p, &length)| (p[1].fit_parameter - p[0].fit_parameter) / length)
            .collect::<Vec<_>>();
        if lengths.iter().any(|s| !s.is_finite() || *s <= 0.)
            || secants.iter().any(|s| !s.is_finite() || *s <= 0.)
        {
            return Err(invalid("collapsed arc-length clock"));
        }
        let mut slopes = vec![secants[0]];
        slopes.extend(secants.windows(2).map(|p| p[0].min(p[1])));
        slopes.push(*secants.last().unwrap());
        let mut pieces = Vec::with_capacity(lengths.len());
        for (j, &length) in lengths.iter().enumerate() {
            let a = stations[j].fit_parameter;
            let b = stations[j + 1].fit_parameter;
            // Quintic Hermite pieces share first derivatives and zero second
            // derivatives at each contact. The smaller adjacent secant keeps
            // all Bernstein increments positive, proving monotonicity of each
            // stored proposal. Decline floating collapse rather than clipping.
            let interval = match policy {
                GordonCurveFitClock::WholeInterval => {
                    IntervalClock::Whole(quintic(a, b, length * slopes[j], length * slopes[j + 1])?)
                }
                GordonCurveFitClock::Local {
                    transition_fraction: w,
                } => {
                    let delta = b - a;
                    let left_end = if w == 0.5 {
                        a.midpoint(b)
                    } else {
                        w.mul_add(delta, a)
                    };
                    let right_start = if w == 0.5 {
                        left_end
                    } else {
                        (1. - w).mul_add(delta, a)
                    };
                    if left_end <= a
                        || right_start < left_end
                        || right_start >= b
                        || (w < 0.5 && left_end == right_start)
                    {
                        return Err(invalid("local clock transition collapsed"));
                    }
                    IntervalClock::Local {
                        left: quintic(a, left_end, w * (length * slopes[j]), w * delta)?,
                        right: quintic(right_start, b, w * delta, w * (length * slopes[j + 1]))?,
                        a,
                        delta,
                        width: w,
                    }
                }
            };
            pieces.push(interval);
        }
        Ok(Self { pieces, slopes })
    }

    pub fn value(&self, interval: usize, fraction: f64) -> Result<f64, Error> {
        Ok(match &self.pieces[interval] {
            IntervalClock::Whole(curve) => curve.evaluate(fraction)?.x(),
            IntervalClock::Local {
                left,
                right,
                a,
                delta,
                width,
            } => {
                if fraction == 1. {
                    right.evaluate(1.)?.x()
                } else if fraction <= *width {
                    left.evaluate(fraction / width)?.x()
                } else if fraction >= 1. - width {
                    right.evaluate((fraction - (1. - width)) / width)?.x()
                } else {
                    fraction.mul_add(*delta, *a)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stations(parameters: &[f64]) -> Vec<CurveFitStation> {
        parameters
            .iter()
            .map(|&t| CurveFitStation {
                source_parameter: t,
                fit_parameter: t,
                point: Point3::try_new(0., 0., 0.).unwrap(),
            })
            .collect()
    }
    #[test]
    fn local_clocks_keep_affine_centers_and_match_arc_length_jets_at_contacts() {
        let stations = stations(&[0., 0.3, 1.]);
        let lengths = [2., 8.];
        let clock = Clock::new(
            &stations,
            &lengths,
            GordonCurveFitClock::Local {
                transition_fraction: 0.25,
            },
        )
        .unwrap();
        for (j, &length) in lengths.iter().enumerate() {
            let a = stations[j].fit_parameter;
            let b = stations[j + 1].fit_parameter;
            let delta = b - a;
            assert_eq!(clock.value(j, 0.).unwrap(), a);
            assert_eq!(clock.value(j, 1.).unwrap(), b);
            for r in [0.25, 0.375, 0.5, 0.625, 0.75] {
                assert_eq!(clock.value(j, r).unwrap(), r.mul_add(delta, a));
            }
            let IntervalClock::Local {
                left, right, width, ..
            } = &clock.pieces[j]
            else {
                panic!("local clock expected")
            };
            for (piece, t, slope) in [
                (left, 0., clock.slopes[j]),
                (left, 1., delta / length),
                (right, 0., delta / length),
                (right, 1., clock.slopes[j + 1]),
            ] {
                let (_, first, second) = piece.evaluate_with_second_derivative(t).unwrap();
                assert!((first.x() / (length * width) - slope).abs() < 1e-12);
                assert!(second.x().abs() / (length * width).powi(2) < 1e-11);
            }
            let mut previous = a;
            for i in 1..=512 {
                let value = clock.value(j, i as f64 / 512.).unwrap();
                assert!(value > previous && value <= b);
                previous = value;
            }
        }
    }
    #[test]
    fn half_windows_share_the_same_nonbinary_midpoint_and_endpoints() {
        let stations = stations(&[0., 0.2, 1.]);
        let clock = Clock::new(
            &stations,
            &[3., 7.],
            GordonCurveFitClock::Local {
                transition_fraction: 0.5,
            },
        )
        .unwrap();
        for (j, pair) in stations.windows(2).enumerate() {
            let expected = pair[0].fit_parameter.midpoint(pair[1].fit_parameter);
            let IntervalClock::Local { left, right, .. } = &clock.pieces[j] else {
                panic!("local clock expected")
            };
            assert_eq!(left.evaluate(1.).unwrap().x(), expected);
            assert_eq!(right.evaluate(0.).unwrap().x(), expected);
            assert_eq!(clock.value(j, 0.5).unwrap(), expected);
            assert_eq!(clock.value(j, 0.).unwrap(), pair[0].fit_parameter);
            assert_eq!(clock.value(j, 1.).unwrap(), pair[1].fit_parameter);
        }
    }
    #[test]
    fn nonbinary_windows_keep_literal_contacts_without_rounding_extrapolation() {
        let stations = stations(&[0., 0.3, 1.]);
        for transition_fraction in [0.1, 0.3, 0.5] {
            let clock = Clock::new(
                &stations,
                &[2., 8.],
                GordonCurveFitClock::Local {
                    transition_fraction,
                },
            )
            .unwrap();
            for (j, pair) in stations.windows(2).enumerate() {
                assert_eq!(clock.value(j, 0.).unwrap(), pair[0].fit_parameter);
                assert_eq!(clock.value(j, 1.).unwrap(), pair[1].fit_parameter);
                assert!(clock.value(j, 1_f64.next_down()).unwrap() <= pair[1].fit_parameter);
            }
        }
    }
    #[test]
    fn local_clock_declines_collapsed_transition_charts() {
        let stations = stations(&[0., 0.5, 0.5_f64.next_up(), 1.]);
        assert!(
            Clock::new(
                &stations,
                &[1., 1., 1.],
                GordonCurveFitClock::Local {
                    transition_fraction: 0.5
                }
            )
            .is_err()
        );
    }
}
