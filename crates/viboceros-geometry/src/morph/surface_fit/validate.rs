use super::super::interpolation::error_fractions;
use super::*;

pub(super) struct Errors {
    pub deviation: Real,
    pub directions: [Vec<Real>; 2],
}

pub(super) fn errors(
    point_at: &mut impl FnMut([Real; 2], [ParameterSide; 2]) -> Result<Point3, GeometryError>,
    fitted: &NurbsSurface,
    breaks: &[Vec<Break>; 2],
    threshold: Real,
) -> Result<Errors, GeometryError> {
    let fractions = error_fractions(8);
    let count = fractions.len();
    let mut result = Errors {
        deviation: 0.0,
        directions: breaks.each_ref().map(|b| vec![0.0; b.len() - 1]),
    };
    let mut residuals = Vec::with_capacity(count * count);
    for (j, span_v) in breaks[1].windows(2).enumerate() {
        for (i, span_u) in breaks[0].windows(2).enumerate() {
            residuals.clear();
            let mut deviation: Real = 0.0;
            for &fv in &fractions {
                let v = stable_lerp(span_v[0].parameter, span_v[1].parameter, fv)?;
                let side_v = if v == span_v[1].parameter {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                for &fu in &fractions {
                    let u = stable_lerp(span_u[0].parameter, span_u[1].parameter, fu)?;
                    let side_u = if u == span_u[1].parameter {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    };
                    let exact = point_at([u, v], [side_u, side_v])?;
                    let actual = fitted.evaluate_on_sides(u, v, side_u, side_v)?;
                    let residual = actual.vector_to(exact)?;
                    deviation = deviation.max(residual.length()?);
                    residuals.push(residual.to_array());
                }
            }
            result.deviation = result.deviation.max(deviation);
            if deviation > threshold {
                let mut variation = [0.0_f64; 2];
                for v in 0..count {
                    for u in 0..count {
                        let r = residuals[v * count + u];
                        for (axis, other) in
                            [residuals[v * count], residuals[u]].into_iter().enumerate()
                        {
                            let difference =
                                Vector3::try_from(std::array::from_fn(|c| r[c] - other[c]))?
                                    .length()?;
                            variation[axis] = variation[axis].max(difference);
                        }
                    }
                }
                // Residual variation is a refinement heuristic, not an error
                // bound. Avoid refining a direction already represented exactly
                // when the other direction dominates the observed error.
                if variation[0] >= variation[1] * 0.25 {
                    result.directions[0][i] = result.directions[0][i].max(deviation);
                }
                if variation[1] >= variation[0] * 0.25 {
                    result.directions[1][j] = result.directions[1][j].max(deviation);
                }
            }
        }
    }
    Ok(result)
}
