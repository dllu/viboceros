//! Residual variation chooses proposals; exact certificates decide acceptance.
use super::*;

/// Skip expensive certification for proposals with an obvious sampled error.
/// A passing preflight still requires the full exact certificate. A failed
/// preflight only requests more fitting; it makes no exact inequality claim.
pub(super) fn sampled_outside(
    candidate: &NurbsSurface,
    cuts: &[Vec<Real>; 2],
    limit: Real,
    sample: &mut impl FnMut(Real, Real) -> Result<Point3, GeometryError>,
) -> Result<bool, GeometryError> {
    for vs in cuts[1].windows(2) {
        for us in cuts[0].windows(2) {
            for v in stations(vs, 3)? {
                for u in stations(us, 3)? {
                    if candidate.evaluate(u, v)?.distance_to(sample(u, v)?)? > limit * 1.001 {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn refine(
    candidate: &NurbsSurface,
    cuts: &mut [Vec<Real>; 2],
    options: SurfaceFitOptions,
    limit: Real,
    sample: &mut impl FnMut(Real, Real) -> Result<Point3, GeometryError>,
) -> Result<bool, GeometryError> {
    let mut scores: [Vec<Real>; 2] = cuts.each_ref().map(|c| vec![0.; c.len() - 1]);
    let count = options.degree.map(|p| p + 2);
    for (j, vs) in cuts[1].windows(2).enumerate() {
        for (i, us) in cuts[0].windows(2).enumerate() {
            let u = stations(us, count[0])?;
            let v = stations(vs, count[1])?;
            let mut residuals = Vec::with_capacity(count[0] * count[1]);
            let mut maximum = 0_f64;
            for &y in &v {
                for &x in &u {
                    let r = candidate
                        .evaluate(x, y)?
                        .vector_to(sample(x, y)?)?
                        .to_array();
                    maximum = maximum.max(norm(r));
                    residuals.push(r);
                }
            }
            if maximum <= limit * 0.5 {
                continue;
            }
            let variation = [
                variation(&residuals, count, 0)?,
                variation(&residuals, count, 1)?,
            ];
            let largest = variation[0].max(variation[1]);
            for (a, index) in [i, j].into_iter().enumerate() {
                // A residual caused by the other axis can vary polynomially
                // here. Degree+1 differences avoid unnecessary tensor growth.
                if largest <= maximum * 1e-8 || variation[a] >= largest * 0.125 {
                    scores[a][index] = scores[a][index].max(maximum);
                }
            }
        }
    }
    // A certificate can detect a feature missed by every sample, or decline
    // after its arithmetic budget. Sampling cannot turn that into acceptance.
    if scores.iter().flatten().all(|s| *s == 0.) {
        for score in scores.iter_mut().flatten() {
            *score = 1.;
        }
    }
    let mut added = false;
    for a in 0..2 {
        let remaining = options.maximum_control_count[a] - (cuts[a].len() + options.degree[a] - 1);
        let mut proposals = scores[a]
            .iter()
            .enumerate()
            .filter_map(|(i, &score)| {
                let t = cuts[a][i].midpoint(cuts[a][i + 1]);
                (score > 0. && t > cuts[a][i] && t < cuts[a][i + 1]).then_some((score, t))
            })
            .collect::<Vec<_>>();
        proposals.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.total_cmp(&b.1)));
        proposals.truncate(remaining);
        added |= !proposals.is_empty();
        cuts[a].extend(proposals.into_iter().map(|(_, t)| t));
        cuts[a].sort_by(Real::total_cmp);
    }
    Ok(added)
}

fn stations(span: &[Real], count: usize) -> Result<Vec<Real>, GeometryError> {
    (0..count)
        .map(|i| {
            let t = i as Real / (count - 1) as Real;
            let x = if i == 0 {
                span[0]
            } else if i + 1 == count {
                span[1]
            } else {
                span[0] * (1. - t) + span[1] * t
            };
            if !x.is_finite() || x < span[0] || x > span[1] {
                Err(GeometryError::SurfaceApproximationLimit)
            } else {
                Ok(x)
            }
        })
        .collect()
}

fn norm(r: [Real; 3]) -> Real {
    r[0].hypot(r[1]).hypot(r[2])
}

fn variation(
    residuals: &[[Real; 3]],
    count: [usize; 2],
    axis: usize,
) -> Result<Real, GeometryError> {
    let mut maximum = 0_f64;
    for fixed in 0..count[1 - axis] {
        let mut line = (0..count[axis])
            .map(|i| {
                residuals[if axis == 0 {
                    fixed * count[0] + i
                } else {
                    i * count[0] + fixed
                }]
            })
            .collect::<Vec<_>>();
        for order in 1..count[axis] {
            for i in 0..count[axis] - order {
                line[i] = std::array::from_fn(|k| line[i + 1][k] - line[i][k]);
                if line[i].iter().any(|x| !x.is_finite()) {
                    return Err(GeometryError::SurfaceApproximationLimit);
                }
            }
        }
        maximum = maximum.max(norm(line[0]));
    }
    Ok(maximum)
}
