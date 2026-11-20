//! Rebuild a retimed C1 region on independently selected dyadic intervals.
use super::*;
use crate::curve_fit::g1::hermite_cubics;

pub(super) fn propose(
    source: &NurbsCurve,
    tolerance: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let mut pending = vec![(0., 1., 0)];
    let mut cuts = vec![0.];
    let mut attempts = 0;
    while let Some((start, end, depth)) = pending.pop() {
        attempts += 1;
        if attempts > 2 * MAX_PIECES - 1 {
            return Ok(None);
        }
        let candidate = hermite_cubics(source, &[start, end])?;
        let trimmed = source.try_trimmed(start..=end)?;
        let mut sampled = true;
        for i in 1..8 {
            let fraction = i as Real / 8.;
            let parameter = start + fraction * (end - start);
            if source
                .evaluate(parameter)?
                .distance_to(candidate.evaluate(fraction)?)?
                > tolerance * 0.5
            {
                sampled = false;
                break;
            }
        }
        // Matching parameter samples only propose an interval. The complete
        // polynomial/rational certificate must also cover the points between.
        let certified = if sampled {
            match trimmed.parameter_curve_deviation_bound(&candidate, tolerance * 0.5) {
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
    Ok(Some(hermite_cubics(source, &cuts)?))
}

#[cfg(test)]
mod tests;
