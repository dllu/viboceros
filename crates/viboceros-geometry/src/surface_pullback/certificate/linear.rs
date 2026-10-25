//! Certify complete straight spans before discarding their parameter clocks.
use super::*;

pub(crate) fn exact_linear_curve_vertices(
    source: &NurbsCurve,
) -> Result<Option<Vec<[Rational; 3]>>, GeometryError> {
    if source.degree() > MAX_DEGREE {
        return Ok(None);
    }
    let mut budget = Budget(MAX_WORK);
    let Some(spline) = curve::Spline::spatial(source, &mut budget)? else {
        return Ok(None);
    };
    let mut cuts = spline.cuts();
    cuts.dedup();
    let mut vertices = Vec::new();
    for pair in cuts.windows(2) {
        let controls = spline.extract(&pair[0], &pair[1], &mut budget)?;
        let points = controls
            .iter()
            .map(|h| {
                let point = std::array::from_fn(|i| &h[i] / &h[3]);
                for x in &point {
                    budget.check(x)?;
                }
                Ok(point)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let start = &points[0];
        let end = points.last().unwrap();
        if vertices.last().is_some_and(|p| p != start) {
            return Ok(None);
        }
        if !ordered(&points, start, end, &mut budget)? {
            return Ok(None);
        }
        if vertices.is_empty() {
            vertices.push(start.clone());
        }
        if start != end {
            vertices.push(end.clone());
            let n = vertices.len();
            if n >= 3
                && ordered(
                    &vertices[n - 3..],
                    &vertices[n - 3],
                    &vertices[n - 1],
                    &mut budget,
                )?
            {
                vertices.remove(n - 2);
            }
        }
    }
    Ok((vertices.len() >= 2).then_some(vertices))
}

fn ordered(
    points: &[[Rational; 3]],
    start: &[Rational; 3],
    end: &[Rational; 3],
    budget: &mut Budget,
) -> Result<bool, GeometryError> {
    let delta = std::array::from_fn::<_, 3, _>(|i| &end[i] - &start[i]);
    let Some(axis) = delta.iter().position(|x| !x.is_zero()) else {
        return Ok(points.iter().all(|p| p == start));
    };
    let mut previous = Rational::zero();
    for point in points {
        let t = (&point[axis] - &start[axis]) / &delta[axis];
        budget.check(&t)?;
        if t < previous || t > Rational::one() {
            return Ok(false);
        }
        for i in 0..3 {
            let difference = &point[i] - &start[i] - &t * &delta[i];
            budget.check(&difference)?;
            if !difference.is_zero() {
                return Ok(false);
            }
        }
        previous = t;
    }
    Ok(true)
}
