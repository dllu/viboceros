//! Sequential inverse queries with quadrature checkpoints as root brackets.
use super::*;
use crate::integration::cash_karp;

#[derive(Clone, Copy, Debug)]
struct Node {
    parameter: Real,
    distance: Real,
}

impl ArcLengthSampler<'_> {
    /// Invert nondecreasing normalized lengths using incremental quadrature.
    ///
    /// Each accepted approximate point becomes the next query's origin. The
    /// stopping error is `fractional_tolerance * integrated_total_length` for
    /// each increment. A polycurve uses its containing component's length.
    /// Errors can accumulate. Results depend on earlier
    /// fractions in the batch. Use `parameters_at_normalized_lengths` for
    /// precise, order-independent geometry queries.
    ///
    /// The method reuses Cash–Karp integration, retains span boundaries and
    /// checkpoints separated by more than 1% of each integration interval,
    /// and tightens a root's brackets with checkpoints from every refinement.
    /// Source-domain endpoints and repeated fractions are retained exactly.
    /// Validate the entire batch before work; no partial result or cache
    /// mutation escapes on failure. Tolerance must lie strictly in (0, 1).
    /// Preparation permits 65,536 integration steps. All inverse refinements
    /// share a separate 65,536-step budget, with 80 bracket updates per query.
    pub fn parameters_at_incremental_normalized_lengths(
        &self,
        fractions: &[Real],
        fractional_tolerance: Real,
    ) -> Result<Vec<Real>, GeometryError> {
        if fractions.len() > MAX_LOOKUP_NODES {
            return Err(GeometryError::ArcLengthBatchResourceLimit {
                maximum: MAX_LOOKUP_NODES,
            });
        }
        require_finite(
            fractions.iter().copied(),
            "incremental arc-length fractions",
        )?;
        if let Some(&fraction) = fractions.iter().find(|&&f| !(0. ..=1.).contains(&f)) {
            return Err(GeometryError::ArcLengthFractionOutOfDomain { fraction });
        }
        if fractions.windows(2).any(|p| p[0] > p[1]) {
            return Err(GeometryError::ArcLengthFractionsNotIncreasing);
        }
        if !fractional_tolerance.is_finite()
            || fractional_tolerance <= 0.
            || fractional_tolerance >= 1.
        {
            return Err(GeometryError::InvalidArcLengthIntegrationTolerance);
        }
        if fractions.is_empty() {
            return Ok(Vec::new());
        }
        if self.spans.iter().all(|s| !s.variable_speed) {
            return self.parameters_at_normalized_lengths(fractions);
        }
        let trace = self.integration_checkpoints_in_frame(fractional_tolerance, false, false)?;
        let nodes = trace
            .iter()
            .map(|p| Node {
                parameter: p.parameter(),
                distance: p.distance(),
            })
            .collect::<Vec<_>>();
        let total = nodes.last().unwrap().distance;
        require_finite([total], "incremental arc length")?;
        if total <= 0. {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let component_lengths = if let CurveRef::PolyCurve(curve) = self.curve() {
            let prefix = |t| {
                nodes[nodes
                    .partition_point(|n| n.parameter <= t)
                    .saturating_sub(1)]
                .distance
            };
            Some(
                curve
                    .parameters()
                    .windows(2)
                    .map(|p| prefix(p[1]) - prefix(p[0]))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let mut previous = nodes[0];
        let mut last_fraction = None;
        let mut out = Vec::with_capacity(fractions.len());
        let mut step_budget = cash_karp::MAX_STEPS;
        for &fraction in fractions {
            if last_fraction == Some(fraction) {
                out.push(*out.last().unwrap());
                continue;
            }
            last_fraction = Some(fraction);
            let target = fraction * total;
            let index = nodes
                .partition_point(|n| n.distance < target)
                .clamp(1, nodes.len() - 1);
            let mut lower = nodes[index - 1];
            let mut upper = nodes[index];
            let local_length = if let (CurveRef::PolyCurve(curve), Some(lengths)) =
                (self.curve(), &component_lengths)
            {
                lengths[curve.segment_index(lower.parameter, ParameterSide::Right)?]
            } else {
                total
            };
            let error = fractional_tolerance * local_length;
            require_finite([error], "incremental arc-length error")?;
            let near = if (target - lower.distance).abs() < (target - upper.distance).abs() {
                lower
            } else {
                upper
            };
            if fraction == 0. || fraction == 1. || (target - near.distance).abs() <= error {
                let node = if fraction == 0. {
                    nodes[0]
                } else if fraction == 1. {
                    *nodes.last().unwrap()
                } else {
                    near
                };
                previous = node;
                out.push(self.source_parameter(node.parameter)?);
                continue;
            }
            if previous.distance > lower.distance {
                lower = previous;
            }
            let mut found = None;
            for _ in 0..80 {
                let f = (target - lower.distance) / (upper.distance - lower.distance);
                let t = stable_lerp(lower.parameter, upper.parameter, f);
                if t <= lower.parameter || t >= upper.parameter {
                    found = Some(t.clamp(lower.parameter, upper.parameter));
                    break;
                }
                let prefix = self.incremental_prefix(
                    lower,
                    t,
                    fractional_tolerance,
                    total,
                    &mut step_budget,
                )?;
                let value = prefix.last().unwrap().distance;
                if (value - target).abs() <= error {
                    found = Some(t);
                    break;
                }
                if value < target {
                    lower = *prefix.last().unwrap();
                } else {
                    for node in prefix.into_iter().skip(1) {
                        if node.distance < target {
                            lower = node;
                        } else {
                            upper = node;
                            break;
                        }
                    }
                }
            }
            let parameter = found.ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
            previous = Node {
                parameter,
                distance: target,
            };
            out.push(self.source_parameter(parameter)?);
        }
        Ok(out)
    }

    fn incremental_prefix(
        &self,
        origin: Node,
        end: Real,
        fractional_tolerance: Real,
        scale: Real,
        budget: &mut usize,
    ) -> Result<Vec<Node>, GeometryError> {
        let width = end - origin.parameter;
        let trace = cash_karp::integrate(fractional_tolerance, *budget, |q| {
            let t = if q == 1. {
                end
            } else {
                stable_lerp(origin.parameter, end, q)
            };
            scaled_ratio(self.speed(t)?, width, scale)
        })?;
        *budget = budget.saturating_sub(trace.attempted_steps);
        let mut out = vec![origin];
        let mut saved = 0.;
        for p in trace.checkpoints.iter().skip(1) {
            if p.parameter - saved > 0.01 || p.parameter == 1. {
                let parameter = if p.parameter == 1. {
                    end
                } else {
                    stable_lerp(origin.parameter, end, p.parameter)
                };
                let distance = origin.distance + p.integral * scale;
                require_finite([parameter, distance], "incremental length prefix")?;
                out.push(Node {
                    parameter,
                    distance,
                });
                saved = p.parameter;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsCurve, WeightedPoint3};

    fn circle(scale: Real, domain: [Real; 2]) -> NurbsCurve {
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
    fn incremental_queries_depend_on_the_prefix_and_preserve_its_results() {
        let curve = circle(1., [0., 1.]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let fractions = (0..=840).map(|i| i as Real / 840.).collect::<Vec<_>>();
        let all = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
            .unwrap();
        let prefix = sampler
            .parameters_at_incremental_normalized_lengths(&fractions[..421], 1e-6)
            .unwrap();
        let isolated = sampler
            .parameters_at_incremental_normalized_lengths(&[0.5], 1e-6)
            .unwrap();
        assert_eq!(&all[..421], prefix);
        assert!((all[420] - isolated[0]).abs() > 1e-5);
        assert_eq!(all[0], 0.);
        assert_eq!(all[840], 1.);
        assert!(all.windows(2).all(|p| p[0] <= p[1]));
        let repeated = sampler
            .parameters_at_incremental_normalized_lengths(&[0., 0., 0.5, 0.5, 1., 1.], 1e-6)
            .unwrap();
        assert_eq!(repeated[0], repeated[1]);
        assert_eq!(repeated[2], repeated[3]);
        assert_eq!(repeated[4], repeated[5]);
    }

    #[test]
    fn scales_and_source_domains_retain_the_same_incremental_inverse() {
        let fractions = [0., 0.125, 0.25, 0.5, 0.75, 0.875, 1.];
        let reference = circle(1., [0., 1.]);
        let reference =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&reference), Tolerance::DEFAULT)
                .unwrap()
                .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
                .unwrap();
        for scale in [1e-200, 1., 1e200] {
            let curve = circle(scale, [10., 40.]);
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT)
                    .unwrap();
            let actual = sampler
                .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
                .unwrap();
            for (a, b) in actual.iter().zip(&reference) {
                assert!(((a - 10.) / 30. - b).abs() < 1e-8);
            }
        }
        let start = 1e12_f64;
        let curve = circle(1., [start, start.next_up()]);
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        let actual = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
            .unwrap();
        assert_eq!(actual[0], start);
        assert_eq!(actual[6], start.next_up());
        assert!(actual.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn invalid_batches_fail_without_changing_the_precise_sampler() {
        let curve = circle(1., [0., 1.]);
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        sampler.prepare_budgeted_repeated_sampling(64).unwrap();
        let before = sampler
            .parameters_at_normalized_lengths(&[0., 0.2, 0.5, 1.])
            .unwrap();
        for fractions in [
            &[-0.1][..],
            &[1.1],
            &[Real::NAN],
            &[Real::INFINITY],
            &[0.5, 0.4],
        ] {
            assert!(
                sampler
                    .parameters_at_incremental_normalized_lengths(fractions, 1e-6)
                    .is_err()
            );
        }
        for tolerance in [0., 1., -1., Real::NAN, Real::INFINITY] {
            assert_eq!(
                sampler
                    .parameters_at_incremental_normalized_lengths(&[0.5], tolerance)
                    .err(),
                Some(GeometryError::InvalidArcLengthIntegrationTolerance)
            );
        }
        assert!(
            sampler
                .parameters_at_incremental_normalized_lengths(&[], 1e-6)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            sampler.parameters_at_incremental_normalized_lengths(
                &vec![0.; MAX_LOOKUP_NODES + 1],
                1e-6
            ),
            Err(GeometryError::ArcLengthBatchResourceLimit { .. })
        ));
        assert!(matches!(
            sampler.incremental_prefix(
                Node {
                    parameter: 0.,
                    distance: 0.
                },
                0.5,
                1e-6,
                1.,
                &mut 0
            ),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
        assert_eq!(
            before,
            sampler
                .parameters_at_normalized_lengths(&[0., 0.2, 0.5, 1.])
                .unwrap()
        );
    }

    #[test]
    fn span_boundaries_use_local_derivatives_and_retain_physical_lengths() {
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
        let fractions = [0., 0.1, 0.2, 0.3, 0.5, 0.75, 1.];
        let actual = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-8)
            .unwrap();
        for (f, t) in fractions.into_iter().zip(actual) {
            let expected = if f <= 0.2 {
                2.5 * f
            } else {
                0.5 + 0.625 * (f - 0.2)
            };
            assert!((t - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn composite_inverse_scales_each_component_error_by_its_own_length() {
        let arc = circle(1., [0., 1.]);
        let line = crate::LineSegment::try_new(
            Point3::try_new(0., 1., 0.).unwrap(),
            Point3::try_new(-1., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let poly = crate::PolyCurve3::try_new(vec![
            crate::CurveSegment3::NurbsCurve(arc.clone()),
            crate::CurveSegment3::Line(line),
        ])
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::PolyCurve(&poly), Tolerance::DEFAULT).unwrap();
        let single =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&arc), Tolerance::DEFAULT).unwrap();
        let length = single
            .integration_checkpoints(1e-6)
            .unwrap()
            .last()
            .unwrap()
            .distance();
        let fractions = [0., 0.1, 0.2, 0.3, 0.4, 0.5];
        let local = fractions.map(|f| f * (length + 1.) / length);
        let expected = single
            .parameters_at_incremental_normalized_lengths(&local, 1e-6)
            .unwrap();
        let actual = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, 1e-6)
            .unwrap();
        for (a, b) in actual.iter().zip(expected) {
            assert!((a - b).abs() < 1e-9);
        }
    }
}
