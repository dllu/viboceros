//! Bounded proposals from mid-isocurves; only complete tensor proofs admit them.
use super::*;
use crate::{NurbsCurve, PreparedCurveClosest, Tolerance};
mod preflight;
#[cfg(test)]
mod tests;

const MAX_DIVISIONS: usize = 16;
type Maps = [Vec<[Real; 2]>; 2];

pub(super) fn certify(
    source: &NurbsSurface,
    candidate: &NurbsSurface,
    limit: Real,
) -> Result<Option<(Real, Maps)>, GeometryError> {
    // Exact zero still has parameter and planar admission. Numerical closest
    // queries cannot establish an exact nonlinear clock at a finite grid.
    if limit == 0. {
        return Ok(None);
    }
    let Some(a) = sections(source) else {
        return Ok(None);
    };
    let Some(b) = sections(candidate) else {
        return Ok(None);
    };
    let (Ok(u), Ok(v)) = (
        b[0].prepare_closest_parameter_search(),
        b[1].prepare_closest_parameter_search(),
    ) else {
        return Ok(None);
    };
    let searches = [u, v];
    let tolerance = Tolerance::try_new(limit.min(1e-12).max(Real::MIN_POSITIVE), 1e-12, 1e-9)?;
    let mut cache = [[None; MAX_DIVISIONS + 1]; 2];
    for divisions in [4, 8, MAX_DIVISIONS] {
        let Some(u) = axis_map(&a[0], &searches[0], divisions, tolerance, &mut cache[0]) else {
            continue;
        };
        let Some(v) = axis_map(&a[1], &searches[1], divisions, tolerance, &mut cache[1]) else {
            continue;
        };
        let maps = [u, v];
        if maps.iter().all(|m| m.len() == 2) || !preflight::within(source, candidate, &maps, limit)
        {
            continue;
        }
        if let Some(bound) = source.mapped_surface_deviation_bound(
            candidate,
            maps.each_ref().map(Vec::as_slice),
            limit,
        )? {
            return Ok(Some((bound, maps)));
        }
    }
    Ok(None)
}

/// Rounded isocurves and unit-domain copies only propose correspondences.
/// Final admission always refers to the untouched original stored tensors.
fn sections(source: &NurbsSurface) -> Option<[NurbsCurve; 2]> {
    let u = source.domain_u();
    let v = source.domain_v();
    Some([
        source
            .isocurve_u(v.start().midpoint(*v.end()))
            .ok()?
            .try_reparameterized(0. ..=1.)
            .ok()?,
        source
            .isocurve_v(u.start().midpoint(*u.end()))
            .ok()?
            .try_reparameterized(0. ..=1.)
            .ok()?,
    ])
}

fn axis_map(
    source: &NurbsCurve,
    target: &PreparedCurveClosest<'_>,
    divisions: usize,
    tolerance: Tolerance,
    cache: &mut [Option<Real>; MAX_DIVISIONS + 1],
) -> Option<Vec<[Real; 2]>> {
    let mut pairs = vec![[0., 0.]];
    for i in 1..divisions {
        let index = i * (MAX_DIVISIONS / divisions);
        let f = i as Real / divisions as Real;
        let g = match cache[index] {
            Some(value) => value,
            None => {
                let value = target
                    .closest_parameter(source.evaluate(f).ok()?, tolerance)
                    .ok()?;
                cache[index] = Some(value);
                value
            }
        };
        if !g.is_finite() || g <= pairs.last()?[1] || g >= 1. {
            return None;
        }
        pairs.push([f, g]);
    }
    pairs.push([1., 1.]);
    // Snapping is another proposal, never an accuracy decision. It avoids
    // expanding a tensor grid along a numerically unchanged direction.
    if pairs.iter().all(|p| (p[0] - p[1]).abs() < 1e-12) {
        pairs = vec![[0., 0.], [1., 1.]];
    }
    Some(pairs)
}
