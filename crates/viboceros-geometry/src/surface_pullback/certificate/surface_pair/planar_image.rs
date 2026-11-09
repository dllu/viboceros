//! Convex planar image proofs independent of parameter speed and injectivity.
use super::*;
mod half_space;
#[cfg(test)]
mod tests;
type P = [Rational; 3];

impl NurbsSurface {
    /// Bound complete surface images through convex planar quadrilaterals,
    /// independently of UV correspondence. Exact auxiliary controls project
    /// onto a corner plane and its four boundary lines. Common-sign weights
    /// bound displacement by the largest control movement. Exact half-spaces
    /// and subdivision prove each auxiliary image lies inside its quad.
    ///
    /// A continuous boundary traversing the four quad edges has degree one;
    /// together with containment this proves the entire quad is covered. No
    /// injectivity is assumed. Matching cyclic/reversed corner triangulations
    /// bound the quads by their largest corresponding vertex distance. The two
    /// control-movement bounds are added outward to enclose the original images.
    /// Both tensors must be clamped and continuous, with nonzero common-sign weights.
    /// Supports degrees <=16 and <=4096 controls per surface. Work, bit and
    /// subdivision limits apply. `None` is outside or unproved, never proof of
    /// inequality. This makes no claim about UV parameters, trims or topology.
    pub fn planar_surface_image_deviation_bound(
        &self,
        other: &Self,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        match bound(self, other, limit, &mut Budget(MAX_WORK)) {
            Err(GeometryError::SurfaceCurveCertificateWorkLimit) => Ok(None),
            result => result,
        }
    }
}

struct Quad {
    corners: [P; 4],
    axes: [usize; 2],
    normal: P,
    positive: bool,
    radius: Real,
}

fn bound(
    a: &NurbsSurface,
    b: &NurbsSurface,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let Some(a) = quad(a, limit, budget)? else {
        return Ok(None);
    };
    let Some(b) = quad(b, limit, budget)? else {
        return Ok(None);
    };
    let mut best: Option<Real> = None;
    for reverse in [false, true] {
        for offset in 0..4 {
            let mut maximum = 0_f64;
            let mut within = true;
            for i in 0..4 {
                let j = (offset + if reverse { 4 - i } else { i }) % 4;
                let delta: H = std::array::from_fn(|k| {
                    if k == 3 {
                        Rational::one()
                    } else {
                        &a.corners[i][k] - &b.corners[j][k]
                    }
                });
                for value in &delta {
                    budget.check(value)?;
                }
                if let Some(value) = algebra::norm_bound(&delta, limit) {
                    maximum = maximum.max(value);
                } else {
                    within = false;
                    break;
                }
            }
            if within {
                let total = [
                    rational(a.radius) + rational(b.radius) + rational(maximum),
                    Rational::zero(),
                    Rational::zero(),
                    Rational::one(),
                ];
                budget.check(&total[0])?;
                if let Some(upper) = algebra::norm_bound(&total, limit) {
                    best = Some(best.map_or(upper, |old| old.min(upper)));
                }
            }
        }
    }
    Ok(best)
}

fn quad(
    source: &NurbsSurface,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Quad>, GeometryError> {
    if source.degree_u() > MAX_DEGREE
        || source.degree_v() > MAX_DEGREE
        || source.control_points().len() > MAX_CONTROLS
    {
        return Ok(None);
    }
    for (p, k, n) in [
        (
            source.degree_u(),
            source.knots_u(),
            source.control_point_count_u(),
        ),
        (
            source.degree_v(),
            source.knots_v(),
            source.control_point_count_v(),
        ),
    ] {
        let start = k[p];
        let end = k[n];
        if k[..=p].iter().any(|t| *t != start)
            || k[n..].iter().any(|t| *t != end)
            || k.chunk_by(|a, b| a == b)
                .any(|g| g[0] > start && g[0] < end && g.len() > p)
        {
            return Ok(None);
        }
    }
    let sign = source.control_points()[0].weight().is_sign_negative();
    if source
        .control_points()
        .iter()
        .any(|p| p.weight() == 0. || p.weight().is_sign_negative() != sign)
    {
        return Ok(None);
    }
    budget.charge(source.control_points().len() * 16)?;
    let points = source
        .control_points()
        .iter()
        .map(|p| p.point().to_array().map(rational))
        .collect::<Vec<P>>();
    let nu = source.control_point_count_u();
    let nv = source.control_point_count_v();
    let mut corners = [
        points[0].clone(),
        points[nu - 1].clone(),
        points[nu * nv - 1].clone(),
        points[(nv - 1) * nu].clone(),
    ];
    let normal = cross(
        &difference(&corners[1], &corners[0]),
        &difference(&corners[3], &corners[0]),
    );
    for value in &normal {
        budget.check(value)?;
    }
    let Some(drop) = normal.iter().position(|x| !x.is_zero()) else {
        return Ok(None);
    };
    let normal_square = normal.iter().map(|n| n * n).sum::<Rational>();
    budget.check(&normal_square)?;
    corners[2] = project_plane(&corners[2], &corners[0], &normal, &normal_square);
    for coordinate in &corners[2] {
        budget.check(coordinate)?;
    }
    let axes = (0..3).filter(|i| *i != drop).collect::<Vec<_>>();
    let axes = [axes[0], axes[1]];
    let turn = orient(&corners[0], &corners[1], &corners[2], axes);
    budget.check(&turn)?;
    if turn.is_zero() {
        return Ok(None);
    }
    let mut quad = Quad {
        corners,
        axes,
        normal,
        positive: turn > Rational::zero(),
        radius: 0.,
    };
    for i in 0..4 {
        let turn = orient(
            &quad.corners[i],
            &quad.corners[(i + 1) % 4],
            &quad.corners[(i + 2) % 4],
            axes,
        );
        budget.check(&turn)?;
        if turn.is_zero() || (turn > Rational::zero()) != quad.positive {
            return Ok(None);
        }
    }
    let mut needs_patch = [false; 4];
    let mut auxiliary = Vec::with_capacity(points.len());
    for (index, original) in points.iter().enumerate() {
        let corner = if index == 0 {
            Some(0)
        } else if index == nu - 1 {
            Some(1)
        } else if index == nu * nv - 1 {
            Some(2)
        } else if index == (nv - 1) * nu {
            Some(3)
        } else {
            None
        };
        let boundary = if index < nu {
            Some(0)
        } else if index % nu + 1 == nu {
            Some(1)
        } else if index / nu + 1 == nv {
            Some(2)
        } else if index % nu == 0 {
            Some(3)
        } else {
            None
        };
        let mut point = project_plane(original, &quad.corners[0], &quad.normal, &normal_square);
        if let Some(corner) = corner {
            point = quad.corners[corner].clone();
        } else if let Some(edge) = boundary {
            point = project_line(&point, &quad.corners[edge], &quad.corners[(edge + 1) % 4]);
        }
        let displacement: H = std::array::from_fn(|k| {
            if k == 3 {
                Rational::one()
            } else {
                &point[k] - &original[k]
            }
        });
        for coordinate in point.iter().chain(displacement.iter()) {
            budget.check(coordinate)?;
        }
        let Some(radius) = algebra::norm_bound(&displacement, limit) else {
            return Ok(None);
        };
        quad.radius = quad.radius.max(radius);
        for edge in 0..4 {
            let value = quad.side(edge, &point);
            budget.check(&value)?;
            if value < Rational::zero() {
                needs_patch[edge] = true;
            }
            let boundary = match edge {
                0 => index < nu,
                1 => index % nu + 1 == nu,
                2 => index / nu + 1 == nv,
                _ => index % nu == 0,
            };
            if boundary && !value.is_zero() {
                return Ok(None);
            }
        }
        auxiliary.push(point);
    }
    if needs_patch.iter().any(|v| *v) {
        let Some(mut surface) =
            surface::Surface::with_cartesian_controls(source, &auxiliary, budget)?
        else {
            return Ok(None);
        };
        let cuts = [surface.normalized_cuts(0), surface.normalized_cuts(1)];
        for u in cuts[0].windows(2) {
            for v in cuts[1].windows(2) {
                let net = surface.normalized_patch(
                    &[[u[0].clone(), u[1].clone()], [v[0].clone(), v[1].clone()]],
                    budget,
                )?;
                for (edge, needed) in needs_patch.iter().enumerate() {
                    if !needed {
                        continue;
                    }
                    let values = net
                        .iter()
                        .map(|h| quad.homogeneous_side(edge, h))
                        .collect::<Vec<_>>();
                    for value in &values {
                        budget.check(value)?;
                    }
                    if !half_space::nonnegative(
                        values,
                        [source.degree_u(), source.degree_v()],
                        budget,
                    )? {
                        return Ok(None);
                    }
                }
            }
        }
    }
    Ok(Some(quad))
}

impl Quad {
    fn side(&self, edge: usize, p: &P) -> Rational {
        let value = orient(
            &self.corners[edge],
            &self.corners[(edge + 1) % 4],
            p,
            self.axes,
        );
        if self.positive { value } else { -value }
    }
    fn homogeneous_side(&self, edge: usize, h: &H) -> Rational {
        let a = &self.corners[edge];
        let b = &self.corners[(edge + 1) % 4];
        let [x, y] = self.axes;
        let value =
            (&b[x] - &a[x]) * (&h[y] - &a[y] * &h[3]) - (&b[y] - &a[y]) * (&h[x] - &a[x] * &h[3]);
        if self.positive { value } else { -value }
    }
}
fn difference(a: &P, b: &P) -> P {
    std::array::from_fn(|i| &a[i] - &b[i])
}
fn cross(a: &P, b: &P) -> P {
    std::array::from_fn(|i| &a[(i + 1) % 3] * &b[(i + 2) % 3] - &a[(i + 2) % 3] * &b[(i + 1) % 3])
}
fn orient(a: &P, b: &P, c: &P, [x, y]: [usize; 2]) -> Rational {
    (&b[x] - &a[x]) * (&c[y] - &a[y]) - (&b[y] - &a[y]) * (&c[x] - &a[x])
}
fn project_plane(p: &P, origin: &P, normal: &P, square: &Rational) -> P {
    let delta = difference(p, origin);
    let height = (0..3).map(|k| &normal[k] * &delta[k]).sum::<Rational>() / square;
    std::array::from_fn(|k| &p[k] - &normal[k] * &height)
}
fn project_line(p: &P, a: &P, b: &P) -> P {
    let direction = difference(b, a);
    let delta = difference(p, a);
    let t = (0..3).map(|k| &direction[k] * &delta[k]).sum::<Rational>()
        / direction.iter().map(|x| x * x).sum::<Rational>();
    std::array::from_fn(|k| &a[k] + &direction[k] * &t)
}
