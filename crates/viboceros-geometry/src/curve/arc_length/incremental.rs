//! Independent and incremental inverses with quadrature checkpoints as brackets.
use super::*;
use crate::integration::cash_karp;

#[derive(Clone, Copy, Debug)]
struct Node {
    parameter: Real,
    distance: Real,
}

#[derive(Clone, Copy, PartialEq)]
enum InversePolicy {
    Incremental,
    Independent,
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
        self.quadrature_inverse(fractions, fractional_tolerance, InversePolicy::Incremental)
    }

    /// Numerical scalar inverse using quadrature checkpoints and the requested
    /// fractional stopping error. This is separate from the precise distance
    /// inverse and from the prefix-dependent incremental batch contract.
    /// See [`Self::parameters_at_quadrature_normalized_lengths`] for limits.
    pub fn parameter_at_quadrature_normalized_length(
        &self,
        fraction: Real,
        fractional_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        Ok(self.parameters_at_quadrature_normalized_lengths(&[fraction], fractional_tolerance)?[0])
    }

    /// Independent numerical scalar queries, sharing one quadrature trace.
    /// Arbitrary order and repetitions are accepted; earlier fractions do not
    /// alter later roots. Every refinement retains its closest integration
    /// checkpoint within the stopping error. This intentionally differs
    /// from incremental batch inversion, which accepts the refined endpoint.
    ///
    /// Tolerance must be finite and strictly between zero and one. Endpoints
    /// are exact. Preparation and inverse refinement each allow 65,536 steps;
    /// all queries share the inverse budget, with 80 bracket updates per root.
    /// The existing 1,048,576-query limit applies. Failure yields no partial
    /// result or source mutation. These estimates are not distance certificates;
    /// use precise queries for order-independent geometry with tighter accuracy.
    pub fn parameters_at_quadrature_normalized_lengths(
        &self,
        fractions: &[Real],
        fractional_tolerance: Real,
    ) -> Result<Vec<Real>, GeometryError> {
        self.quadrature_inverse(fractions, fractional_tolerance, InversePolicy::Independent)
    }

    fn quadrature_inverse(
        &self,
        fractions: &[Real],
        fractional_tolerance: Real,
        policy: InversePolicy,
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
        if policy == InversePolicy::Incremental && fractions.windows(2).any(|p| p[0] > p[1]) {
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
        let quadrature_line = |s: &ParameterSpan| {
            matches!(s.linear,
            Some(LinearSpan::Rational {controls}) if controls[0].weight()!=controls[1].weight())
        };
        if self.spans.iter().all(|s| !s.variable_speed)
            && (policy == InversePolicy::Incremental || !self.spans.iter().any(quadrature_line))
        {
            let mut parameters = self.parameters_at_normalized_lengths(fractions)?;
            if policy == InversePolicy::Independent {
                for (&fraction, parameter) in fractions.iter().zip(&mut parameters) {
                    if fraction == 0. || fraction == 1. {
                        continue;
                    }
                    let distance = fraction * self.total_length;
                    let index = self
                        .spans
                        .partition_point(|s| s.cumulative_start <= distance)
                        .saturating_sub(1);
                    let span = &self.spans[index];
                    if span.cumulative_start == distance {
                        *parameter = self.source_parameter(span.start)?;
                    }
                }
            }
            return Ok(parameters);
        }
        let trace = self.integration_trace(
            fractional_tolerance,
            false,
            false,
            policy == InversePolicy::Independent,
        )?;
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
                } else if policy == InversePolicy::Independent && near.distance == target {
                    nodes[nodes.partition_point(|n| n.distance <= target) - 1]
                } else {
                    near
                };
                previous = node;
                out.push(self.source_parameter(node.parameter)?);
                continue;
            }
            if policy == InversePolicy::Incremental && previous.distance > lower.distance {
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
                let accepted = if policy == InversePolicy::Independent {
                    prefix
                        .iter()
                        .skip(1)
                        .min_by(|a, b| {
                            (a.distance - target)
                                .abs()
                                .total_cmp(&(b.distance - target).abs())
                        })
                        .filter(|n| (n.distance - target).abs() <= error)
                } else {
                    prefix
                        .last()
                        .filter(|n| (n.distance - target).abs() <= error)
                };
                if let Some(node) = accepted {
                    found = Some(node.parameter);
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
mod tests;
