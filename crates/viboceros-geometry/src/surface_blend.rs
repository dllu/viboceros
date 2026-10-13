//! Exact rational Hermite interpolation of complete isoparametric boundaries.
//! Boundary jets are matched as spline functions, not fitted at sample points.
use crate::nurbs::stable_divided_difference;
use crate::{
    GeometryError, NurbsSurface, Point3, Real, SurfaceExtensionEdge, Tolerance, WeightedPoint3,
};

#[cfg(test)]
mod tests;

const MAX_EDGE_CONTROLS: usize = 4096;
const MAX_INPUT_CONTROLS: usize = 1_000_000;
const MAX_WORK: usize = 64_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceBlendContinuity {
    Position,
    Tangency,
    Curvature,
}
impl SurfaceBlendContinuity {
    pub const fn order(self) -> usize {
        match self {
            Self::Position => 0,
            Self::Tangency => 1,
            Self::Curvature => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceBlendOptions {
    pub continuity: [SurfaceBlendContinuity; 2],
    /// Positive multipliers of half the corresponding parent's normalized
    /// cross-boundary parameter extent. Each end is independently adjustable.
    pub bulges: [Real; 2],
    /// None aligns sampled edge correspondence by the smaller distance sum.
    pub reverse_second: Option<bool>,
}
impl Default for SurfaceBlendOptions {
    fn default() -> Self {
        Self {
            continuity: [SurfaceBlendContinuity::Tangency; 2],
            bulges: [1.; 2],
            reverse_second: None,
        }
    }
}

/// Numerical checks of the exact construction at span endpoints and interiors.
/// These checks do not certify global fairness or absence of self intersections.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceBlendDiagnostics {
    pub samples: usize,
    pub maximum_position_error: Real,
    pub maximum_tangent_plane_angle: Real,
    pub maximum_first_derivative_relative_error: Real,
    pub maximum_second_derivative_relative_error: Real,
    pub singular_boundary_samples: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceBlendResult {
    pub surface: NurbsSurface,
    pub reversed_second: bool,
    pub diagnostics: SurfaceBlendDiagnostics,
}

fn error(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceBlend { context }
}

/// Blends two complete natural boundaries, preserving rational boundary
/// positions and requested differential jets over their entire common basis.
/// Different degrees, knots, domain lengths, weights and parent sizes are
/// supported. The result has normalized U along the edges and normalized V
/// across the blend. G1/G2 use positive constant cross-parameter scales, a
/// sufficient construction for geometric continuity at regular boundary points.
/// No source is changed. Mixed-sign weights, singular requested tangent planes,
/// excessive work and nonpositive blend weights are rejected explicitly.
pub fn try_blend_nurbs_surfaces(
    first: &NurbsSurface,
    first_edge: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_edge: SurfaceExtensionEdge,
    options: SurfaceBlendOptions,
    tolerance: Tolerance,
) -> Result<SurfaceBlendResult, GeometryError> {
    if options.bulges.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(error("bulges must be positive finite numbers"));
    }
    if [first, second].iter().any(|s| {
        s.control_points().len() > MAX_INPUT_CONTROLS || s.degree_u() > 64 || s.degree_v() > 64
    }) {
        return Err(error("parent surface exceeds the control budget"));
    }
    let mut parents = [
        orient(first, first_edge, true)?,
        orient(second, second_edge, false)?,
    ];
    let reverse = match options.reverse_second {
        Some(reverse) => reverse,
        None => {
            let mut scores = [0.; 2];
            for i in 0..=4 {
                let t = i as Real / 4.;
                let a = parents[0].evaluate(t, 1.)?;
                scores[0] += a.distance_to(parents[1].evaluate(t, 0.)?)?;
                scores[1] += a.distance_to(parents[1].evaluate(1. - t, 0.)?)?;
            }
            scores[1] < scores[0]
        }
    };
    if reverse {
        parents[1] = parents[1]
            .try_reversed_u()?
            .try_reparameterized(0.0..=1., 0.0..=1.)?;
    }
    compatible(&mut parents)?;
    let degree = options.continuity[0].order() + options.continuity[1].order() + 1;
    let count_u = parents[0].control_point_count_u();
    let origin = parents[0]
        .control_point(0, parents[0].control_point_count_v() - 1)
        .unwrap()
        .point();
    let jets = [
        boundary_jets(&parents[0], true, origin)?,
        boundary_jets(&parents[1], false, origin)?,
    ];
    let scales = options.bulges.map(|b| b * 0.5);
    let mut net = vec![[0.; 4]; count_u * (degree + 1)];
    for end in 0..2 {
        for (u, jet) in jets[end].iter().enumerate() {
            let scale = scales[end] * if end == 0 { 1. } else { -1. };
            for distance in 0..=options.continuity[end].order() {
                let row = if end == 0 {
                    distance
                } else {
                    degree - distance
                };
                for k in 0..4 {
                    let h = jet[0][k];
                    let d = jet[1][k] * scale;
                    let dd = jet[2][k] * scale * scale;
                    net[row * count_u + u][k] = match distance {
                        0 => h,
                        1 => h + d / degree as Real,
                        2 => h + 2. * d / degree as Real + dd / (degree * (degree - 1)) as Real,
                        _ => unreachable!(),
                    };
                }
            }
        }
    }
    let controls = net
        .into_iter()
        .map(|h| {
            if !h[3].is_finite() || h[3] <= 0. {
                return Err(error(
                    "blend has nonpositive weights; reduce Bulge1 or Bulge2",
                ));
            }
            WeightedPoint3::try_new(
                Point3::try_new(
                    origin.x() + h[0] / h[3],
                    origin.y() + h[1] / h[3],
                    origin.z() + h[2] / h[3],
                )?,
                h[3],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let knots_v = std::iter::repeat_n(0., degree + 1)
        .chain(std::iter::repeat_n(1., degree + 1))
        .collect();
    let surface = NurbsSurface::try_new_rational(
        parents[0].degree_u(),
        degree,
        count_u,
        degree + 1,
        controls,
        parents[0].knots_u().to_vec(),
        knots_v,
    )?;
    let diagnostics = check(&surface, &parents, scales, options.continuity, tolerance)?;
    Ok(SurfaceBlendResult {
        surface,
        reversed_second: reverse,
        diagnostics,
    })
}

fn orient(
    surface: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    first: bool,
) -> Result<NurbsSurface, GeometryError> {
    let mut s = surface.try_clamped_to_active_domain()?;
    if matches!(
        edge,
        SurfaceExtensionEdge::West | SurfaceExtensionEdge::East
    ) {
        s = s.try_swapped_uv()?;
    }
    let lower = matches!(
        edge,
        SurfaceExtensionEdge::South | SurfaceExtensionEdge::West
    );
    if lower == first {
        s = s.try_reversed_v()?;
    }
    s.try_reparameterized(0.0..=1., 0.0..=1.)
}

fn groups(s: &NurbsSurface) -> Vec<(Real, usize)> {
    let mut groups: Vec<(Real, usize)> = Vec::new();
    for &k in s.knots_u() {
        if k <= 0. || k >= 1. {
            continue;
        }
        if let Some(last) = groups.last_mut()
            && last.0 == k
        {
            last.1 += 1;
        } else {
            groups.push((k, 1));
        }
    }
    groups
}
fn compatible(parents: &mut [NurbsSurface; 2]) -> Result<(), GeometryError> {
    let degree = parents[0].degree_u().max(parents[1].degree_u());
    let mut work = 0usize;
    for s in parents.iter_mut() {
        let count = degree
            + 1
            + groups(s)
                .iter()
                .map(|(_, m)| m + degree - s.degree_u())
                .sum::<usize>();
        if count > MAX_EDGE_CONTROLS
            || count.saturating_mul(s.control_point_count_v()) > MAX_INPUT_CONTROLS
        {
            return Err(error("compatible parent basis exceeds the control budget"));
        }
        if degree != s.degree_u() {
            work = work.saturating_add(
                count
                    .saturating_mul(s.control_point_count_v())
                    .saturating_mul(degree)
                    .saturating_mul(degree),
            );
            if work > MAX_WORK {
                return Err(error("compatible parent basis exceeds the work budget"));
            }
        }
        *s = s.try_change_degree(degree, s.degree_v(), false)?;
    }
    let mut union = groups(&parents[0]);
    for (k, m) in groups(&parents[1]) {
        if let Some(entry) = union.iter_mut().find(|x| x.0 == k) {
            entry.1 = entry.1.max(m);
        } else {
            union.push((k, m));
        }
    }
    union.sort_by(|a, b| a.0.total_cmp(&b.0));
    let count = degree + 1 + union.iter().map(|x| x.1).sum::<usize>();
    if count > MAX_EDGE_CONTROLS
        || parents
            .iter()
            .any(|s| count.saturating_mul(s.control_point_count_v()) > MAX_INPUT_CONTROLS)
    {
        return Err(error("compatible parent basis exceeds the control budget"));
    }
    for s in parents.iter_mut() {
        let existing = groups(s);
        let insertions = union
            .iter()
            .filter(|&&(k, m)| {
                existing
                    .iter()
                    .find(|&&(t, _)| t == k)
                    .is_none_or(|&(_, n)| n < m)
            })
            .count();
        work = work.saturating_add(
            count
                .saturating_mul(s.control_point_count_v())
                .saturating_mul(degree)
                .saturating_mul(insertions),
        );
        if work > MAX_WORK {
            return Err(error("compatible parent basis exceeds the work budget"));
        }
        for &(k, m) in &union {
            if existing
                .iter()
                .find(|&&(t, _)| t == k)
                .is_none_or(|&(_, n)| n < m)
            {
                *s = s.try_insert_knot_u(k, m)?;
            }
        }
    }
    if parents[0].knots_u() != parents[1].knots_u() {
        return Err(error("boundary bases could not be aligned"));
    }
    Ok(())
}

type Jet = [[Real; 4]; 3];
fn boundary_jets(s: &NurbsSurface, end: bool, origin: Point3) -> Result<Vec<Jet>, GeometryError> {
    let p = s.degree_v();
    let n = s.control_point_count_v();
    let knots = s.knots_v();
    let weight_scale = s
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0., Real::max);
    let sign = s.control_points()[0].weight().signum();
    if s.control_points()
        .iter()
        .any(|c| c.weight().signum() != sign)
    {
        return Err(error("mixed-sign parent weights are unsupported"));
    }
    let h = |u, v| -> Result<[Real; 4], GeometryError> {
        let c = s.control_point(u, v).unwrap();
        let w = c.weight() / (weight_scale * sign);
        let xyz = origin.vector_to(c.point())?.to_array();
        let out = [xyz[0] * w, xyz[1] * w, xyz[2] * w, w];
        crate::require_finite(out, "surface blend homogeneous control")?;
        Ok(out)
    };
    (0..s.control_point_count_u())
        .map(|u| {
            let index = if end { n - 1 } else { 0 };
            let mut jet = [h(u, index)?, [0.; 4], [0.; 4]];
            let derivative = |i: usize| -> Result<[Real; 4], GeometryError> {
                let a = h(u, i)?;
                let b = h(u, i + 1)?;
                let mut d = [0.; 4];
                for k in 0..4 {
                    d[k] =
                        stable_divided_difference(b[k], a[k], p, knots[i + 1], knots[i + p + 1])?;
                }
                Ok(d)
            };
            let i = if end { n - 2 } else { 0 };
            jet[1] = derivative(i)?;
            if p > 1 {
                let j = if end { n - 3 } else { 1 };
                let other = derivative(j)?;
                let a = if end { other } else { jet[1] };
                let b = if end { jet[1] } else { other };
                let i = i.min(j);
                for k in 0..4 {
                    jet[2][k] = stable_divided_difference(
                        b[k],
                        a[k],
                        p - 1,
                        knots[i + 2],
                        knots[i + p + 1],
                    )?;
                }
            }
            Ok(jet)
        })
        .collect()
}

fn relative(a: crate::Vector3, b: crate::Vector3) -> Result<Real, GeometryError> {
    let difference = crate::Vector3::try_new(a.x() - b.x(), a.y() - b.y(), a.z() - b.z())?;
    Ok(difference.length()? / a.length()?.max(b.length()?).max(1.))
}
fn check(
    s: &NurbsSurface,
    parents: &[NurbsSurface; 2],
    scales: [Real; 2],
    continuity: [SurfaceBlendContinuity; 2],
    tolerance: Tolerance,
) -> Result<SurfaceBlendDiagnostics, GeometryError> {
    let mut report = SurfaceBlendDiagnostics::default();
    for (a, b) in s.spans_u() {
        for t in [
            a,
            a + (b - a) * 0.25,
            a + (b - a) * 0.5,
            a + (b - a) * 0.75,
            b,
        ] {
            // A knot has two jets. Check the interior side of each incident span.
            let side = if t == b {
                crate::ParameterSide::Left
            } else {
                crate::ParameterSide::Right
            };
            for end in 0..2 {
                let p = parents[end].evaluate_with_second_derivatives_on_sides(
                    t,
                    if end == 0 { 1. } else { 0. },
                    side,
                    crate::ParameterSide::Right,
                )?;
                let q = s.evaluate_with_second_derivatives_on_sides(
                    t,
                    end as Real,
                    side,
                    crate::ParameterSide::Right,
                )?;
                report.samples += 1;
                let gap = p.point.distance_to(q.point)?;
                report.maximum_position_error = report.maximum_position_error.max(gap);
                let model_scale = p
                    .point
                    .to_array()
                    .into_iter()
                    .map(Real::abs)
                    .fold(1., Real::max);
                if gap > tolerance.absolute().max(tolerance.relative() * model_scale) {
                    return Err(error(
                        "boundary position verification exceeded model tolerance",
                    ));
                }
                let a = p.derivative_u.cross(p.derivative_v)?;
                let b = q.derivative_u.cross(q.derivative_v)?;
                if a.length()? == 0. || b.length()? == 0. {
                    report.singular_boundary_samples += 1;
                    if continuity[end].order() == 0 {
                        continue;
                    }
                    return Err(error(
                        "requested continuity meets a singular boundary tangent plane",
                    ));
                }
                if continuity[end].order() == 0 {
                    continue;
                }
                let a = a.scaled(1. / a.length()?)?;
                let b = b.scaled(1. / b.length()?)?;
                let angle = a.cross(b)?.length()?.atan2(a.dot(b)?.abs());
                report.maximum_tangent_plane_angle = report.maximum_tangent_plane_angle.max(angle);
                let d = relative(q.derivative_v, p.derivative_v.scaled(scales[end])?)?
                    .max(relative(q.derivative_u, p.derivative_u)?);
                report.maximum_first_derivative_relative_error =
                    report.maximum_first_derivative_relative_error.max(d);
                if d > 1e-7 {
                    return Err(error("boundary first derivative verification failed"));
                }
                if continuity[end].order() == 2 {
                    let dd = relative(
                        q.derivative_vv,
                        p.derivative_vv.scaled(scales[end] * scales[end])?,
                    )?
                    .max(relative(
                        q.derivative_uv,
                        p.derivative_uv.scaled(scales[end])?,
                    )?)
                    .max(relative(q.derivative_uu, p.derivative_uu)?);
                    report.maximum_second_derivative_relative_error =
                        report.maximum_second_derivative_relative_error.max(dd);
                    if dd > 1e-7 {
                        return Err(error("boundary second derivative verification failed"));
                    }
                }
            }
        }
    }
    Ok(report)
}
