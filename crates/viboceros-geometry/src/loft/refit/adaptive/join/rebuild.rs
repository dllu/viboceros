//! Rebuild a retimed C1 region on independently selected dyadic intervals.
use super::*;

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
        let candidate = interpolate(source, &[start, end])?;
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
    Ok(Some(interpolate(source, &cuts)?))
}

fn interpolate(source: &NurbsCurve, cuts: &[Real]) -> Result<NurbsCurve, GeometryError> {
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
