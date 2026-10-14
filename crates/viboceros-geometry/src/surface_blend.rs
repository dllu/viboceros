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

/// Delegate exact parent preparation and endpoint interpolation while retaining
/// boundary jet policy. Refinement acts on the oriented U basis.
type ElevateSurface<'a, E> = dyn FnMut(&NurbsSurface, [usize; 2]) -> Result<NurbsSurface, E> + 'a;
type InterpolateBlend<'a, E> =
    dyn FnMut(&SurfaceBlendInterpolation<'_>) -> Result<Vec<[Real; 4]>, E> + 'a;

/// Exact homogeneous endpoint constraints for all cross-boundary Bezier rows.
/// Each row contains `edge_control_count` four-channel targets. At the first
/// endpoint, derivative orders increase from zero; at the second, they decrease
/// to zero. Derivatives refer to the normalized cross parameter and already
/// include each boundary's bulge. Coordinates use a common local origin.
pub struct SurfaceBlendInterpolation<'a> {
    pub continuity: [SurfaceBlendContinuity; 2],
    pub edge_control_count: usize,
    pub targets: &'a [[Real; 4]],
}
impl SurfaceBlendInterpolation<'_> {
    pub fn degree(&self) -> usize {
        self.continuity[0].order() + self.continuity[1].order() + 1
    }
    pub fn validate(&self) -> Result<(), GeometryError> {
        if self.edge_control_count == 0
            || self.edge_control_count > MAX_EDGE_CONTROLS
            || self.targets.len() != self.edge_control_count * (self.degree() + 1)
        {
            return Err(error("invalid boundary interpolation dimensions"));
        }
        crate::require_finite(
            self.targets.iter().flat_map(|h| *h),
            "surface blend boundary targets",
        )
    }
}

pub struct SurfaceBlendOperations<'a, E> {
    pub clamp: &'a mut dyn FnMut(&NurbsSurface) -> Result<NurbsSurface, E>,
    pub elevate: &'a mut ElevateSurface<'a, E>,
    pub refine: &'a mut dyn FnMut(&NurbsSurface, Real, usize) -> Result<NurbsSurface, E>,
    pub interpolate: &'a mut InterpolateBlend<'a, E>,
}

/// Portable exact endpoint interpolation. Native builds can replace this
/// operation with their kernel's derivative basis and homogeneous solver.
pub fn try_interpolate_surface_blend(
    input: &SurfaceBlendInterpolation<'_>,
) -> Result<Vec<[Real; 4]>, GeometryError> {
    input.validate()?;
    let degree = input.degree();
    let count = input.edge_control_count;
    let mut net = vec![[0.; 4]; input.targets.len()];
    for end in 0..2 {
        let row = if end == 0 { 0 } else { degree };
        let direction = if end == 0 { 1. } else { -1. };
        for u in 0..count {
            let h = input.targets[row * count + u];
            for order in 0..=input.continuity[end].order() {
                let row = if end == 0 { order } else { degree - order };
                for k in 0..4 {
                    let d = if order > 0 {
                        input.targets[if end == 0 {
                            count + u
                        } else {
                            (degree - 1) * count + u
                        }][k]
                            * direction
                    } else {
                        0.
                    };
                    net[row * count + u][k] = match order {
                        0 => h[k],
                        1 => h[k] + d / degree as Real,
                        2 => {
                            let dd = input.targets[row * count + u][k];
                            h[k] + 2. * d / degree as Real + dd / (degree * (degree - 1)) as Real
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
    }
    Ok(net)
}

fn portable_clamp(source: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
    source.try_clamped_to_active_domain()
}
fn portable_elevate(
    source: &NurbsSurface,
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    source.try_change_degree(degree[0], degree[1], false)
}
fn portable_refine(
    source: &NurbsSurface,
    parameter: Real,
    target: usize,
) -> Result<NurbsSurface, GeometryError> {
    source.try_insert_knot_u(parameter, target)
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
    try_blend_nurbs_surfaces_with_operations(
        first,
        first_edge,
        second,
        second_edge,
        options,
        tolerance,
        &mut SurfaceBlendOperations {
            clamp: &mut portable_clamp,
            elevate: &mut portable_elevate,
            refine: &mut portable_refine,
            interpolate: &mut try_interpolate_surface_blend,
        },
    )
}

/// Preserve blend policy with delegated parent edits and endpoint interpolation.
pub fn try_blend_nurbs_surfaces_with_operations<E: From<GeometryError>>(
    first: &NurbsSurface,
    first_edge: SurfaceExtensionEdge,
    second: &NurbsSurface,
    second_edge: SurfaceExtensionEdge,
    options: SurfaceBlendOptions,
    tolerance: Tolerance,
    operations: &mut SurfaceBlendOperations<'_, E>,
) -> Result<SurfaceBlendResult, E> {
    if options.bulges.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(error("bulges must be positive finite numbers").into());
    }
    if [first, second].iter().any(|s| {
        s.control_points().len() > MAX_INPUT_CONTROLS || s.degree_u() > 64 || s.degree_v() > 64
    }) {
        return Err(error("parent surface exceeds the control budget").into());
    }
    let mut parents = [
        orient_with_clamp(first, first_edge, true, operations.clamp)?,
        orient_with_clamp(second, second_edge, false, operations.clamp)?,
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
    compatible(&mut parents, operations)?;
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
    let mut targets = vec![[0.; 4]; count_u * (degree + 1)];
    for end in 0..2 {
        for (u, jet) in jets[end].iter().enumerate() {
            let scale = scales[end];
            for distance in 0..=options.continuity[end].order() {
                let row = if end == 0 {
                    distance
                } else {
                    degree - distance
                };
                for k in 0..4 {
                    targets[row * count_u + u][k] = match distance {
                        0 => jet[0][k],
                        1 => jet[1][k] * scale,
                        2 => jet[2][k] * scale * scale,
                        _ => unreachable!(),
                    };
                }
            }
        }
    }
    let input = SurfaceBlendInterpolation {
        continuity: options.continuity,
        edge_control_count: count_u,
        targets: &targets,
    };
    input.validate()?;
    let net = (operations.interpolate)(&input)?;
    if net.len() != targets.len() {
        return Err(error("boundary interpolation returned an invalid control count").into());
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

#[cfg(test)]
fn orient(
    surface: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    first: bool,
) -> Result<NurbsSurface, GeometryError> {
    orient_with_clamp(surface, edge, first, &mut portable_clamp)
}

fn orient_with_clamp<E: From<GeometryError>>(
    surface: &NurbsSurface,
    edge: SurfaceExtensionEdge,
    first: bool,
    clamp: &mut dyn FnMut(&NurbsSurface) -> Result<NurbsSurface, E>,
) -> Result<NurbsSurface, E> {
    let mut s = clamp(surface)?;
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
    Ok(s.try_reparameterized(0.0..=1., 0.0..=1.)?)
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
fn compatible<E: From<GeometryError>>(
    parents: &mut [NurbsSurface; 2],
    operations: &mut SurfaceBlendOperations<'_, E>,
) -> Result<(), E> {
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
            return Err(error("compatible parent basis exceeds the control budget").into());
        }
        if degree != s.degree_u() {
            work = work.saturating_add(
                count
                    .saturating_mul(s.control_point_count_v())
                    .saturating_mul(degree)
                    .saturating_mul(degree),
            );
            if work > MAX_WORK {
                return Err(error("compatible parent basis exceeds the work budget").into());
            }
        }
        *s = (operations.elevate)(s, [degree, s.degree_v()])?;
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
        return Err(error("compatible parent basis exceeds the control budget").into());
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
            return Err(error("compatible parent basis exceeds the work budget").into());
        }
        for &(k, m) in &union {
            if existing
                .iter()
                .find(|&&(t, _)| t == k)
                .is_none_or(|&(_, n)| n < m)
            {
                *s = (operations.refine)(s, k, m)?;
            }
        }
    }
    if parents[0].knots_u() != parents[1].knots_u() {
        return Err(error("boundary bases could not be aligned").into());
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
