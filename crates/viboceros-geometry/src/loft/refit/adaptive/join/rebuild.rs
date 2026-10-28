//! Rebuild a retimed C1 region on independently selected dyadic intervals.
use super::*;

pub(super) fn propose(
    source: &NurbsCurve,
    tolerance: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    propose_impl(source, tolerance, None)
}

pub(super) fn propose_image(
    source: &NurbsCurve,
    original: &NurbsCurve,
    tolerance: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    propose_impl(source, tolerance, Some(original))
}

fn propose_impl(
    source: &NurbsCurve,
    tolerance: Real,
    original: Option<&NurbsCurve>,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let geometric = original.is_some();
    let reference_search = original
        .map(NurbsCurve::prepare_closest_parameter_search)
        .transpose()?;
    let mut pending = vec![(0., 1., 0)];
    let mut cuts = vec![0.];
    let mut attempts = 0;
    while let Some((start, end, depth)) = pending.pop() {
        attempts += 1;
        if attempts > 2 * MAX_PIECES - 1 {
            return Ok(None);
        }
        let candidate = interpolate(source, &[start, end])?;
        let trimmed = source.try_trimmed(start..=end)?;
        let search = if geometric {
            Some(candidate.prepare_closest_parameter_search()?)
        } else {
            None
        };
        let mut sampled = true;
        for i in 1..8 {
            let fraction = i as Real / 8.;
            let parameter = start + fraction * (end - start);
            let point = source.evaluate(parameter)?;
            let target = if let Some(search) = &search {
                search.closest_parameter(point, Tolerance::DEFAULT)?
            } else {
                fraction
            };
            if point.distance_to(candidate.evaluate(target)?)? > tolerance * 0.5 {
                sampled = false;
                break;
            }
            if let (Some(original), Some(search)) = (original, &reference_search) {
                let point = candidate.evaluate(fraction)?;
                let parameter = search.closest_parameter(point, Tolerance::DEFAULT)?;
                if point.distance_to(original.evaluate(parameter)?)? > tolerance * 0.5 {
                    sampled = false;
                    break;
                }
            }
        }
        // Samples only choose intervals. The complete proxy-curve certificate
        // must cover the points between; the caller separately certifies the
        // assembled result against its original source.
        let certified = if sampled {
            let bound = if geometric {
                crate::curve_fit::g1::deviation_bound(&trimmed, &candidate, tolerance * 0.5)
            } else {
                trimmed.parameter_curve_deviation_bound(&candidate, tolerance * 0.5)
            };
            match bound {
                Ok(bound) => bound.is_some(),
                Err(GeometryError::CurveApproximationLimit) => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        if certified {
            if cuts.len() > MAX_PIECES {
                return Ok(None);
            }
            cuts.push(end);
        } else {
            if depth == MAX_DEPTH {
                return Ok(None);
            }
            let middle = start.midpoint(end);
            if middle <= start || middle >= end {
                return Ok(None);
            }
            pending.push((middle, end, depth + 1));
            pending.push((start, middle, depth + 1));
        }
    }
    Ok(Some(interpolate(source, &cuts)?))
}

pub(super) fn interpolate(source: &NurbsCurve, cuts: &[Real]) -> Result<NurbsCurve, GeometryError> {
    let jets = cuts
        .iter()
        .map(|t| source.evaluate_with_derivative(*t))
        .collect::<Result<Vec<_>, _>>()?;
    let mut controls = vec![jets[0].0];
    let mut knots = vec![0.; 4];
    for (i, pair) in cuts.windows(2).enumerate() {
        let width = (pair[1] - pair[0]) / 3.;
        controls.push(jets[i].0.translated(jets[i].1.scaled(width)?)?);
        controls.push(jets[i + 1].0.translated(jets[i + 1].1.scaled(-width)?)?);
        if i > 0 {
            knots.extend([pair[0]; 2]);
        }
    }
    controls.push(jets.last().unwrap().0);
    knots.extend([1.; 4]);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests;
