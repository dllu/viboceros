//! Adaptive endpoint-frame proposals; complete image certificates admit them.
use super::*;

struct Interval {
    start: Real,
    end: Real,
    from: Real,
    to: Real,
    depth: usize,
}

pub(super) fn propose(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
) -> Result<Option<AdaptiveLinearRefitProposal>, GeometryError> {
    if source.is_closed()? || super::super::super::parameterize::has_tangent_breaks(source, angle)?
    {
        return Ok(None);
    }
    let domain = source.domain();
    let mut pending = vec![Interval {
        start: *domain.start(),
        end: *domain.end(),
        from: 0.,
        to: 1.,
        depth: 0,
    }];
    let mut pieces = Vec::new();
    let mut attempts = 0;
    while let Some(interval) = pending.pop() {
        attempts += 1;
        if attempts > 2 * MAX_PIECES - 1 {
            return Ok(None);
        }
        let Interval {
            start,
            end,
            from,
            to,
            depth,
        } = interval;
        let piece = crate::curve_fit::g1::initial_cubic_from_frames(
            source.evaluate(start)?,
            source.evaluate(end)?,
            source.tangent_at_on_side(start, ParameterSide::Right)?,
            source.tangent_at_on_side(
                end,
                if end == *domain.end() {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                },
            )?,
        )?;
        let trimmed = source.try_trimmed(start..=end)?;
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&trimmed), Tolerance::DEFAULT)?;
        let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
        let parameters = sampler.parameters_at_incremental_normalized_lengths(&fractions, 1e-4)?;
        let fits = sampled_fit(source, &piece, &parameters, tolerance * 0.5)?;
        if fits {
            if pieces.len() == MAX_PIECES {
                return Ok(None);
            }
            pieces.push((to, piece));
        } else {
            if depth == MAX_DEPTH {
                return Ok(None);
            }
            let mut middle = parameters[4];
            // Retain an exact natural boundary when inversion differs only by
            // rounding; its original right-hand tangent remains authoritative.
            let roundoff = 16. * Real::EPSILON * (end - start);
            if let Some(&knot) = source
                .knots()
                .iter()
                .find(|&&k| k > start && k < end && (k - middle).abs() <= roundoff)
            {
                middle = knot;
            }
            let fraction = from + (to - from) * 0.5;
            if middle <= start || middle >= end || fraction <= from || fraction >= to {
                return Ok(None);
            }
            pending.push(Interval {
                start: middle,
                end,
                from: fraction,
                to,
                depth: depth + 1,
            });
            pending.push(Interval {
                start,
                end: middle,
                from,
                to: fraction,
                depth: depth + 1,
            });
        }
    }
    let mut controls = Vec::with_capacity(3 * pieces.len() + 1);
    let mut knots = vec![0.; 4];
    let mut breaks = vec![0.];
    for (end, curve) in pieces {
        controls.extend(
            curve
                .control_points()
                .iter()
                .copied()
                .skip(usize::from(!controls.is_empty())),
        );
        knots.extend([end; 3]);
        breaks.push(end);
    }
    knots.push(1.);
    let curve = NurbsCurve::try_new_rational(3, controls, knots)?;
    let maximum_deviation_bound =
        if let Some(bound) = source.parameter_curve_deviation_bound(&curve, tolerance)? {
            bound
        } else if crate::curve_fit::g1::within_deviation(source, &curve, tolerance)? {
            tolerance
        } else {
            return Ok(None);
        };
    Ok(Some(AdaptiveLinearRefitProposal {
        curve,
        arc_length_breaks: breaks,
        maximum_deviation_bound,
    }))
}

fn sampled_fit(
    source: &NurbsCurve,
    piece: &NurbsCurve,
    parameters: &[Real],
    limit: Real,
) -> Result<bool, GeometryError> {
    let search = piece.prepare_closest_parameter_search()?;
    for &parameter in &parameters[1..parameters.len() - 1] {
        let point = source.evaluate(parameter)?;
        let t = search.closest_parameter(point, Tolerance::DEFAULT)?;
        if point.distance_to(piece.evaluate(t)?)? > limit {
            return Ok(false);
        }
    }
    Ok(true)
}
