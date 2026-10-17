//! Optional integration diagnostics, separate from precise query brackets.
use super::*;
use crate::integration::cash_karp;

/// One accepted integration step in the original curve's parameter domain.
/// Distances use Cash–Karp quadrature (or exact constant-speed/linear spans).
/// The embedded step error is an estimate, not a guaranteed geometric bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcLengthCheckpoint {
    parameter: Real,
    distance: Real,
    estimated_step_error: Real,
}
impl ArcLengthCheckpoint {
    pub fn parameter(self) -> Real {
        self.parameter
    }
    pub fn distance(self) -> Real {
        self.distance
    }
    pub fn estimated_step_error(self) -> Real {
        self.estimated_step_error
    }
}

impl ArcLengthSampler<'_> {
    /// Retain accepted adaptive integration checkpoints for numerical analysis.
    ///
    /// Each variable-speed span uses the embedded Cash–Karp 5/4 rule, an initial
    /// normalized parameter step of 0.01 and bounded adaptive step growth. Speed
    /// is scaled by the span's precise length to avoid model-unit overflow or
    /// underflow in error control. Constant-speed and linear spans contribute
    /// their existing exact lengths. Span boundaries are retained, and returned
    /// parameters belong to the original domain, which can quantize them.
    ///
    /// `fractional_tolerance` must be finite and strictly between zero and one.
    /// Integration is limited to 65,536 attempted steps across the entire curve
    /// and 1,048,576 returned checkpoints. An error returns no partial trace.
    /// Checkpoints alone do not define an accurate length inverse or a native
    /// Flow lookup. In particular, linear interpolation between them has no
    /// geometric error guarantee. Precise queries use the sampler's brackets.
    pub fn integration_checkpoints(
        &self,
        fractional_tolerance: Real,
    ) -> Result<Vec<ArcLengthCheckpoint>, GeometryError> {
        self.integration_checkpoints_in_frame(fractional_tolerance, true, true)
    }

    pub(super) fn integration_checkpoints_in_frame(
        &self,
        fractional_tolerance: Real,
        source_parameters: bool,
        retain_all: bool,
    ) -> Result<Vec<ArcLengthCheckpoint>, GeometryError> {
        if !fractional_tolerance.is_finite()
            || fractional_tolerance <= 0.
            || fractional_tolerance >= 1.
        {
            return Err(GeometryError::InvalidArcLengthIntegrationTolerance);
        }
        let map = |t| {
            if source_parameters {
                self.source_parameter(t)
            } else {
                Ok(t)
            }
        };
        let domain = if source_parameters {
            self.source_curve().domain()
        } else {
            self.curve().domain()
        };
        let mut out = vec![ArcLengthCheckpoint {
            parameter: *domain.start(),
            distance: 0.,
            estimated_step_error: 0.,
        }];
        let (mut sum, mut correction) = (0., 0.);
        let mut attempted_steps = 0;
        for span in &self.spans {
            let prefix = sum + correction;
            let start = map(span.start)?;
            if out.last().unwrap().parameter != start {
                push(&mut out, start, prefix, 0.)?;
            }
            if !span.variable_speed {
                neumaier_add(&mut sum, &mut correction, span.length);
                push(&mut out, map(span.end)?, sum + correction, 0.)?;
                continue;
            }
            let width = span.end - span.start;
            let trace = cash_karp::integrate(
                fractional_tolerance,
                cash_karp::MAX_STEPS.saturating_sub(attempted_steps),
                |q| {
                    let t = if q == 1. {
                        span.end
                    } else {
                        stable_lerp(span.start, span.end, q)
                    };
                    let side = if t == span.end {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    };
                    let speed = self
                        .curve()
                        .evaluate_with_derivative_on_side(t, side)?
                        .1
                        .length()?;
                    scaled_ratio(speed, width, span.length)
                },
            )?;
            attempted_steps += trace.attempted_steps;
            let mut saved = 0.;
            for node in trace.checkpoints.iter().skip(1) {
                if !retain_all && node.parameter - saved <= 0.01 && node.parameter != 1. {
                    continue;
                }
                saved = node.parameter;
                let t = if node.parameter == 1. {
                    span.end
                } else {
                    stable_lerp(span.start, span.end, node.parameter)
                };
                push(
                    &mut out,
                    map(t)?,
                    prefix + node.integral * span.length,
                    node.error * span.length,
                )?;
            }
            neumaier_add(
                &mut sum,
                &mut correction,
                trace.checkpoints.last().unwrap().integral * span.length,
            );
            out.last_mut().unwrap().distance = sum + correction;
        }
        if out.last().unwrap().parameter != *domain.end() {
            push(&mut out, *domain.end(), sum + correction, 0.)?;
        }
        Ok(out)
    }
}

fn push(
    out: &mut Vec<ArcLengthCheckpoint>,
    parameter: Real,
    distance: Real,
    estimated_step_error: Real,
) -> Result<(), GeometryError> {
    require_finite(
        [parameter, distance, estimated_step_error],
        "arc-length checkpoint",
    )?;
    if out.len() >= MAX_LOOKUP_NODES {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    out.push(ArcLengthCheckpoint {
        parameter,
        distance,
        estimated_step_error,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsCurve, WeightedPoint3};

    fn quarter_circle(scale: Real, domain: [Real; 2]) -> NurbsCurve {
        NurbsCurve::try_new_rational(
            2,
            [[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .zip([1., 0.5_f64.sqrt(), 1.])
                .map(|(p, w)| {
                    WeightedPoint3::try_new(Point3::try_from(p.map(|x| x * scale)).unwrap(), w)
                        .unwrap()
                })
                .collect(),
            vec![
                domain[0], domain[0], domain[0], domain[1], domain[1], domain[1],
            ],
        )
        .unwrap()
    }

    #[test]
    fn traces_retain_source_domains_and_scale_the_error_controller() {
        for scale in [1e-200, 1., 1e200] {
            for domain in [[0., 1.], [10., 40.]] {
                let curve = quarter_circle(scale, domain);
                let sampler =
                    ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT)
                        .unwrap();
                let trace = sampler.integration_checkpoints(1e-8).unwrap();
                assert_eq!(trace[0].parameter(), domain[0]);
                assert_eq!(trace.last().unwrap().parameter(), domain[1]);
                assert_eq!(trace[0].distance(), 0.);
                assert!(
                    trace.windows(2).all(|w| w[0].parameter() < w[1].parameter()
                        && w[0].distance() < w[1].distance())
                );
                assert!(
                    (trace.last().unwrap().distance() / scale - std::f64::consts::FRAC_PI_2).abs()
                        < 1e-8
                );
                assert!(trace.iter().all(
                    |n| n.estimated_step_error().is_finite() && n.estimated_step_error() >= 0.
                ));
            }
        }
    }

    #[test]
    fn diagnostic_tolerance_does_not_control_precise_queries() {
        let curve = quarter_circle(1., [0., 1.]);
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        sampler.prepare_budgeted_repeated_sampling(64).unwrap();
        let before = sampler
            .parameters_at_normalized_lengths(&[0.2, 0.5, 0.8])
            .unwrap();
        let trace = sampler.integration_checkpoints(1e-2).unwrap();
        assert!((trace.last().unwrap().distance() - sampler.total_length()).abs() > 1e-5);
        assert_eq!(
            before,
            sampler
                .parameters_at_normalized_lengths(&[0.2, 0.5, 0.8])
                .unwrap()
        );
        for tol in [0., 1., Real::NAN, Real::INFINITY] {
            assert_eq!(
                sampler.integration_checkpoints(tol).err(),
                Some(GeometryError::InvalidArcLengthIntegrationTolerance)
            );
        }
    }

    #[test]
    fn repeated_knots_use_the_interior_derivative_on_each_span() {
        let curve = NurbsCurve::try_new(
            2,
            [
                [0., 0., 0.],
                [0.5, 0., 0.],
                [1., 0., 0.],
                [1., 2., 0.],
                [1., 4., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
        )
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let trace = sampler.integration_checkpoints(1e-10).unwrap();
        assert_eq!(trace.len(), 9);
        let corner = trace.iter().find(|n| n.parameter() == 0.5).unwrap();
        assert!((corner.distance() - 1.).abs() < 1e-14);
        assert!((trace.last().unwrap().distance() - 5.).abs() < 1e-13);
    }

    #[test]
    fn source_domain_quantization_preserves_the_normalized_integral() {
        let start = 1e12_f64;
        let curve = quarter_circle(1., [start, start.next_up()]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let trace = sampler.integration_checkpoints(1e-8).unwrap();
        assert_eq!(trace[0].parameter(), start);
        assert_eq!(trace.last().unwrap().parameter(), start.next_up());
        assert!(
            trace
                .windows(2)
                .any(|w| w[0].parameter() == w[1].parameter())
        );
        assert!((trace.last().unwrap().distance() - std::f64::consts::FRAC_PI_2).abs() < 1e-8);
    }
}
