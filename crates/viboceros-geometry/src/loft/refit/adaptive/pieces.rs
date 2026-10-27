//! Adaptive endpoint-frame proposals; complete image certificates admit them.
use super::*;

struct Interval {
    start: Real,
    end: Real,
    from: Real,
    to: Real,
    depth: usize,
}

pub(super) enum SplitClock {
    IntervalLength,
    SourceLookup,
}

// Raw pieces are provisional. Only the caller can certify their complete
// assembly or a subsequent reconstruction against the original source.
pub(super) struct Candidate {
    pub curve: NurbsCurve,
    pub breaks: Vec<Real>,
    pub source_parameters: Vec<Real>,
}

pub(super) fn propose(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
) -> Result<Option<AdaptiveLinearRefitProposal>, GeometryError> {
    let Some(candidate) = construct(source, tolerance, angle, SplitClock::IntervalLength)? else {
        return Ok(None);
    };
    let maximum_deviation_bound =
        if let Some(bound) = source.parameter_curve_deviation_bound(&candidate.curve, tolerance)? {
            bound
        } else if crate::curve_fit::g1::within_deviation(source, &candidate.curve, tolerance)? {
            tolerance
        } else {
            return Ok(None);
        };
    Ok(Some(AdaptiveLinearRefitProposal {
        curve: candidate.curve,
        arc_length_breaks: candidate.breaks,
        maximum_deviation_bound,
    }))
}

pub(super) fn construct(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
    clock: SplitClock,
) -> Result<Option<Candidate>, GeometryError> {
    if source.is_closed()? || super::super::super::parameterize::has_tangent_breaks(source, angle)?
    {
        return Ok(None);
    }
    let lookup = match clock {
        SplitClock::IntervalLength => None,
        SplitClock::SourceLookup => {
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(source), Tolerance::DEFAULT)?;
            let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
            Some(sampler.parameters_at_incremental_normalized_lengths(&fractions, 1e-4)?)
        }
    };
    // A common-handle correction is qualified on smooth parameter spans.
    // Preserve subdivision across source C1 breaks, including suppressed
    // polyline corners and derivative-speed changes. An ending break also
    // matters: the constructor deliberately uses its original right tangent.
    let correction_breaks = if lookup.is_some() {
        source.parametric_discontinuity_parameters(crate::CurveParametricContinuity::C1)?
    } else {
        Vec::new()
    };
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
        let mut piece = crate::curve_fit::g1::initial_cubic_from_frames(
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
        let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
        let parameters = if let Some(lookup) = &lookup {
            fractions
                .iter()
                .map(|f| lookup_parameter(lookup, from + f * (to - from)))
                .collect()
        } else {
            let sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&trimmed), Tolerance::DEFAULT)?;
            // The legacy straight-region proxy has an intentionally precise
            // local clock; numerical NURBS checkpoint selection is separate.
            sampler.parameters_at_normalized_lengths(&fractions)?
        };
        // Samples propose a piece; a complete interval bound decides whether
        // it can be kept. A feature between stations must trigger refinement
        // rather than rejection of the whole assembled proposal at the end.
        let mut fits = sampled_fit(source, &piece, &parameters, tolerance * 0.5)?
            && crate::curve_fit::g1::within_deviation(&trimmed, &piece, tolerance)?;
        if !fits
            && lookup.is_some()
            && correction_breaks
                .get(correction_breaks.partition_point(|t| *t <= start))
                .is_none_or(|t| *t > end)
            && let Some(adjusted) = crate::curve_fit::g1::adjusted_cubic_at_point(
                &piece,
                source.evaluate(parameters[4])?,
                tolerance * 0.5,
            )?
            && sampled_fit(source, &adjusted, &parameters, tolerance * 0.5)?
            && crate::curve_fit::g1::within_deviation(&trimmed, &adjusted, tolerance)?
        {
            piece = adjusted;
            fits = true;
        }
        if fits {
            if pieces.len() == MAX_PIECES {
                return Ok(None);
            }
            pieces.push((to, end, piece));
        } else {
            if depth == MAX_DEPTH {
                return Ok(None);
            }
            let fraction = from + (to - from) * 0.5;
            let mut middle = if let Some(lookup) = &lookup {
                lookup_parameter(lookup, fraction)
            } else {
                parameters[4]
            };
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
    let mut source_parameters = vec![*domain.start()];
    for (end, parameter, curve) in pieces {
        controls.extend(
            curve
                .control_points()
                .iter()
                .copied()
                .skip(usize::from(!controls.is_empty())),
        );
        knots.extend([end; 3]);
        breaks.push(end);
        source_parameters.push(parameter);
    }
    knots.push(1.);
    let curve = NurbsCurve::try_new_rational(3, controls, knots)?;
    Ok(Some(Candidate {
        curve,
        breaks,
        source_parameters,
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

fn lookup_parameter(lookup: &[Real], fraction: Real) -> Real {
    let station = fraction * 8.;
    let index = (station.floor() as usize).min(7);
    let blend = station - index as Real;
    lookup[index].mul_add(1. - blend, lookup[index + 1] * blend)
}
