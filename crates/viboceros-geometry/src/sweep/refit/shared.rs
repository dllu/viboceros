//! Reconstruct smooth fitted sections together on a dyadic parameter grid.
use super::*;

const MAX_DEPTH: usize = 24;
const MAX_INTERVALS: usize = (MAX_SECTION_CONTROLS - 2) / 2;

#[derive(Clone, Copy)]
pub(super) enum Subdivision {
    Bracketed,
    Image,
}

pub(super) fn needs_grid(curve: &NurbsCurve) -> Result<bool, GeometryError> {
    // Four exact quarter spans already use the common parameter grid.
    let knots = curve.interior_knot_groups();
    Ok(curve.control_points().len() > 4
        && knots.iter().all(|&(_, n)| n < 3)
        && !(knots == [(0.25, 2), (0.5, 2), (0.75, 2)] && curve.is_closed()?))
}

pub(super) fn reconstruct(
    originals: &[SweepSection],
    fits: Vec<NurbsCurve>,
    tolerance: Real,
    subdivision: Subdivision,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let sources = fits
        .into_iter()
        .map(|curve| {
            if curve.interior_knot_groups().iter().any(|&(_, n)| n >= 3) {
                return Err(unsupported());
            }
            if curve.domain() == (0. ..=1.) {
                Ok(curve)
            } else {
                curve.try_reparameterized(0. ..=1.)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let limit = tolerance * 0.5;
    let mut cuts = vec![0.];
    let mut pending = vec![(0., 1., 0)];
    while let Some((start, end, depth)) = pending.pop() {
        let mut uncertain = false;
        let mut split = false;
        for source in &sources {
            match classify(source, start, end, limit, subdivision)? {
                IntervalFit::Within => {}
                IntervalFit::Beyond => {
                    split = true;
                    break;
                }
                IntervalFit::Unresolved => uncertain = true,
            }
        }
        if split {
            if depth == MAX_DEPTH || cuts.len() + pending.len() >= MAX_INTERVALS {
                return Err(unsupported());
            }
            let middle = start.midpoint(end);
            pending.extend([(middle, end, depth + 1), (start, middle, depth + 1)]);
        } else if uncertain {
            // Measurements place the native decision between these tests in
            // the captured cases. An unresolved interval needs further work
            // on the native rule before it can choose a subdivision.
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "Sweep1 Refit could not resolve the shared profile subdivision at this tolerance",
            });
        } else {
            cuts.push(end);
        }
    }
    originals
        .iter()
        .zip(&sources)
        .map(|(section, source)| {
            // Reconstruct all sections at every selected cut from the fitted
            // reference, including cuts introduced by a different section.
            let result = g1::hermite_cubics(source, &cuts)?;
            let reconstructed = match subdivision {
                Subdivision::Bracketed => source.parameter_curve_deviation_bound(&result, limit)?,
                Subdivision::Image => g1::deviation_bound_at_knots(source, &result, &cuts, limit)?,
            };
            if reconstructed.is_none() || !g1::within_deviation(&section.curve, &result, tolerance)?
            {
                return Err(unsupported());
            }
            Ok(result)
        })
        .collect()
}

enum IntervalFit {
    Within,
    Beyond,
    Unresolved,
}

fn classify(
    source: &NurbsCurve,
    start: Real,
    end: Real,
    limit: Real,
    subdivision: Subdivision,
) -> Result<IntervalFit, GeometryError> {
    let candidate = g1::hermite_cubics(source, &[start, end])?;
    let piece = source.try_trimmed(start..=end)?;
    match piece.parameter_curve_deviation_bound(&candidate, limit) {
        Ok(Some(_)) => return Ok(IntervalFit::Within),
        Ok(None) | Err(GeometryError::CurveApproximationLimit) => {}
        Err(error) => return Err(error),
    }
    if exceeds_distance_at_station(source, start..=end, &candidate, limit)? {
        Ok(IntervalFit::Beyond)
    } else if matches!(subdivision, Subdivision::Image)
        && g1::within_deviation(&piece, &candidate, limit)?
    {
        // Closed loops can retain the interval even when the fitted chart
        // moves faster across it than the Hermite candidate.
        Ok(IntervalFit::Within)
    } else {
        Ok(IntervalFit::Unresolved)
    }
}

fn exceeds_distance_at_station(
    source: &NurbsCurve,
    domain: RangeInclusive<Real>,
    candidate: &NurbsCurve,
    limit: Real,
) -> Result<bool, GeometryError> {
    let search = candidate.prepare_closest_parameter_search()?;
    for fraction in [0.5, 0.25, 0.75, 0.125, 0.375, 0.625, 0.875] {
        let t = domain.start() + fraction * (domain.end() - domain.start());
        let p = source.evaluate(t)?;
        let closest = search.closest_parameter(p, Tolerance::DEFAULT)?;
        let distance = candidate.evaluate(closest)?.distance_to(p)?;
        let reserve = (distance - limit) * 0.25;
        if reserve <= 0. {
            continue;
        }
        // Closest-point evaluation proposes a separator. Certify the rounded
        // sample and its distance to the whole candidate before splitting.
        if !source.positions_within_distance(&[(t, ParameterSide::Right, p)], reserve)? {
            continue;
        }
        let point_curve = NurbsCurve::try_new(1, vec![p; 2], vec![0., 0., 1., 1.])?;
        let tolerance = Tolerance::try_new(
            reserve,
            Tolerance::DEFAULT.relative(),
            Tolerance::DEFAULT.angular(),
        )?;
        if point_curve
            .minimum_distance_with_curve_seeded(candidate, tolerance, [0., closest])?
            .is_some_and(|distance| {
                (distance.upper_distance - distance.maximum_error).next_down()
                    > (limit + reserve).next_up()
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}
