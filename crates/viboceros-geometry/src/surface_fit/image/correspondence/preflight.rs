//! A floating grid can reject a map proposal, never certify a surface image.
use super::*;
use crate::ParameterSide;

pub(super) fn within(a: &NurbsSurface, b: &NurbsSurface, maps: &Maps, limit: Real) -> bool {
    let basic = maps.each_ref().map(|pairs| midpoints(pairs));
    let enriched: [Vec<[Real; 2]>; 2] = std::array::from_fn(|axis| {
        let mut pairs = maps[axis].clone();
        for (which, surface) in [a, b].into_iter().enumerate() {
            let knots = if axis == 0 {
                surface.knots_u()
            } else {
                surface.knots_v()
            };
            let domain = if axis == 0 {
                surface.domain_u()
            } else {
                surface.domain_v()
            };
            for knot in knots.chunk_by(|x, y| x == y) {
                let t = knot[0];
                if t <= *domain.start() || t >= *domain.end() {
                    continue;
                }
                let parameters = if axis == 0 {
                    surface.normalized_parameters(t, *surface.domain_v().start())
                } else {
                    surface.normalized_parameters(*surface.domain_u().start(), t)
                };
                let Ok(parameters) = parameters else { continue };
                let value = parameters[axis];
                let index = maps[axis].partition_point(|p| p[which] < value);
                if index == 0 || index == maps[axis].len() {
                    continue;
                }
                let p = maps[axis][index];
                if value == p[which] {
                    pairs.push(p);
                    continue;
                }
                let q = maps[axis][index - 1];
                let fraction = (value - q[which]) / (p[which] - q[which]);
                let mut pair = [0.; 2];
                pair[which] = value;
                pair[1 - which] = q[1 - which] + fraction * (p[1 - which] - q[1 - which]);
                if pair.iter().all(|t| t.is_finite() && (0. ..=1.).contains(t)) {
                    pairs.push(pair);
                }
            }
        }
        pairs.sort_by(|a, b| a[0].total_cmp(&b[0]));
        pairs.dedup_by(|a, b| a[0] == b[0]);
        midpoints(&pairs)
    });
    // Knot enrichment is a preflight optimization, with a separate fixed grid
    // budget. Large source knot grids keep the original bounded uniform grid.
    let stations = if enriched[0].len().saturating_mul(enriched[1].len()) <= 16_384 {
        enriched
    } else {
        basic
    };
    within_stations(a, b, &stations, limit)
}

pub(super) fn midpoints(pairs: &[[Real; 2]]) -> Vec<[Real; 2]> {
    let mut values = vec![pairs[0]];
    for pair in pairs.windows(2) {
        values.push(std::array::from_fn(|i| pair[0][i].midpoint(pair[1][i])));
        values.push(pair[1]);
    }
    values
}

pub(super) fn within_stations(
    a: &NurbsSurface,
    b: &NurbsSurface,
    stations: &[Vec<[Real; 2]>; 2],
    limit: Real,
) -> bool {
    let parameters = |s: &NurbsSurface, which: usize| -> Option<[Vec<(Real, ParameterSide)>; 2]> {
        Some([
            stations[0]
                .iter()
                .map(|p| Some((s.parameter_at_u(p[which]).ok()?, ParameterSide::Right)))
                .collect::<Option<_>>()?,
            stations[1]
                .iter()
                .map(|p| Some((s.parameter_at_v(p[which]).ok()?, ParameterSide::Right)))
                .collect::<Option<_>>()?,
        ])
    };
    let (Some(u), Some(v)) = (parameters(a, 0), parameters(b, 1)) else {
        return false;
    };
    let nu = stations[0].len();
    let mut points = vec![None; nu * stations[1].len()];
    if a.try_for_each_grid_point_on_sides(&u[0], &u[1], |i, j, p| {
        points[j * nu + i] = Some(p.map_err(|_| ())?);
        Ok::<_, ()>(())
    })
    .is_err()
    {
        return false;
    }
    b.try_for_each_grid_point_on_sides(&v[0], &v[1], |i, j, p| {
        let distance = points[j * nu + i]
            .ok_or(())?
            .distance_to(p.map_err(|_| ())?)
            .map_err(|_| ())?;
        if distance > limit * 1.001 {
            return Err(());
        }
        Ok(())
    })
    .is_ok()
}
