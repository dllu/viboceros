//! Fit the transverse handle rows while keeping the rail curves fixed.
use super::{transport, unsupported};
use crate::native::{Error, knot_refinement::solve_uniform_system};
use viboceros_geometry::{
    NurbsCurve, NurbsSurface, Point3, SurfaceBlendContinuity, SurfaceJet2, Tolerance,
    UniformInterpolationSystem, Vector3, WeightedPoint3,
};

#[derive(Clone, Copy, Debug)]
pub struct TwoRailEdgeSupport<'a> {
    pub guide: &'a NurbsCurve,
    pub surface: &'a NurbsSurface,
}

#[derive(Clone, Copy, Debug)]
pub struct TwoRailSweepMatching<'a> {
    pub supports: [&'a [TwoRailEdgeSupport<'a>]; 2],
    pub continuity: [SurfaceBlendContinuity; 2],
    /// Apply the continuity adjustments at the two terminal cross sections.
    pub adjust_ends: [bool; 2],
}

impl TwoRailSweepMatching<'_> {
    pub(super) fn orders(self, degree: usize, width: usize) -> [usize; 2] {
        let orders = std::array::from_fn(|i| {
            if self.supports[i].is_empty() {
                0
            } else {
                self.continuity[i].order()
            }
        });
        if orders.iter().sum::<usize>() + 2 > width || orders.iter().any(|&order| order > degree) {
            [0; 2]
        } else {
            orders
        }
    }

    pub(super) fn adjusts(self, station: f64) -> bool {
        (station != 0. || self.adjust_ends[0]) && (station != 1. || self.adjust_ends[1])
    }
}

pub(super) fn apply(
    surface: NurbsSurface,
    matching: TwoRailSweepMatching<'_>,
    orders: [usize; 2],
    stations: &[f64],
    world_offset: Vector3,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let width = surface.control_point_count_v();
    let count = surface.control_point_count_u();
    let mut controls = surface.control_points().to_vec();
    let degree = surface.degree_u();
    let knots = surface.knots_u();
    let domain = surface.domain_u();
    let (start, end) = (*domain.start(), *domain.end());
    let mut sites = transport::greville(knots, degree)
        .into_iter()
        .map(|u| (u - start) / (end - start))
        .collect::<Vec<_>>();
    sites[0] = 0.;
    *sites.last_mut().unwrap() = 1.;
    transport::pin_sections(&mut sites, stations)?;
    let parameters = sites
        .iter()
        .map(|&site| {
            if site == 1. {
                end
            } else {
                start + site * (end - start)
            }
        })
        .collect::<Vec<_>>();
    // A transported open polynomial section has one shared rail weight at
    // each station across all its handles. Fit Cartesian offsets in that gauge.
    let denominator = NurbsCurve::try_new(
        degree,
        controls[..count]
            .iter()
            .map(|c| Point3::try_new(c.weight(), 0., 0.))
            .collect::<Result<Vec<_>, _>>()?,
        knots.to_vec(),
    )?;

    for (side, order) in orders
        .into_iter()
        .enumerate()
        .filter(|&(_, order)| order > 0)
    {
        let index = |depth: usize| if side == 0 { depth } else { width - 1 - depth };
        let cross = CrossBasis::new(&surface, side);
        let rows = (0..=order)
            .map(|depth| {
                let row = index(depth) * count;
                NurbsCurve::try_new_rational(
                    degree,
                    controls[row..row + count].to_vec(),
                    knots.to_vec(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut targets = Vec::with_capacity(count * 4 * order);
        for (&site, &u) in sites.iter().zip(&parameters) {
            let point = rows[0].evaluate(u)?;
            let handle = rows[1].evaluate(u)?;
            if !matching.adjusts(site) || point.distance_to(handle)? <= tolerance.absolute() {
                targets.extend(std::iter::repeat_n(0., 4 * order));
                continue;
            }
            let (jet, tangent) = support_at(
                matching.supports[side],
                point.translated(world_offset)?,
                tolerance,
            )?;
            let normal = jet
                .derivative_u
                .cross(jet.derivative_v)?
                .normalized_nonzero()?
                .as_vector();
            let old = point.vector_to(handle)?;
            let axial = tangent.scaled(old.dot(tangent)?)?;
            let across = add(old, axial.scaled(-1.)?)?;
            let transverse = normal.cross(tangent)?.normalized_nonzero()?.as_vector();
            let sign = if across.dot(transverse)? < 0. {
                -1.
            } else {
                1.
            };
            let first = add(axial, transverse.scaled(sign * across.length()?)?)?;
            let weight = denominator.evaluate(u)?.x();
            push(&mut targets, add(first, old.scaled(-1.)?)?, weight);
            if order == 2 {
                let second = point.vector_to(rows[2].evaluate(u)?)?;
                let acceleration = normal_acceleration(jet, normal, first.scaled(cross.first)?)?;
                let projection = acceleration / cross.second - second.dot(normal)?;
                push(&mut targets, normal.scaled(projection)?, weight);
            }
        }
        let solved = solve_uniform_system(&UniformInterpolationSystem {
            degree,
            knots,
            parameters: &parameters,
            periodic: false,
            columns: 4 * order,
            targets: &targets,
        })?;
        for u in 0..count {
            for depth in 1..=order {
                let control = &mut controls[index(depth) * count + u];
                let weight = control.weight();
                let base = 4 * (u * order + depth - 1);
                let delta = Vector3::try_new(
                    solved[base] / weight,
                    solved[base + 1] / weight,
                    solved[base + 2] / weight,
                )?;
                *control = WeightedPoint3::try_new(control.point().translated(delta)?, weight)?;
            }
        }
    }
    Ok(NurbsSurface::try_new_rational(
        degree,
        surface.degree_v(),
        count,
        width,
        controls,
        knots.to_vec(),
        surface.knots_v().to_vec(),
    )?)
}

struct CrossBasis {
    first: f64,
    second: f64,
}

impl CrossBasis {
    fn new(surface: &NurbsSurface, side: usize) -> Self {
        let p = surface.degree_v();
        let knots = surface.knots_v();
        let span = |offset| {
            if side == 0 {
                knots[p + offset] - knots[p]
            } else {
                knots[knots.len() - p - 1] - knots[knots.len() - p - 1 - offset]
            }
        };
        Self {
            first: p as f64 / span(1),
            second: (p * (p - 1)) as f64 / (span(1) * span(2)),
        }
    }
}

fn support_at(
    choices: &[TwoRailEdgeSupport<'_>],
    point: Point3,
    tolerance: Tolerance,
) -> Result<(SurfaceJet2, Vector3), Error> {
    let mut nearest = None;
    for support in choices {
        let t = support.guide.closest_parameter(point, tolerance)?;
        let on_edge = support.guide.evaluate(t)?;
        let distance = on_edge.distance_to(point)?;
        if nearest
            .as_ref()
            .is_none_or(|&(_, _, _, best)| distance < best)
        {
            nearest = Some((support, t, on_edge, distance));
        }
    }
    let (support, t, on_edge, distance) =
        nearest.ok_or_else(|| unsupported("surface-edge continuity needs a supporting face"))?;
    if distance > tolerance.absolute() {
        return Err(unsupported(
            "matched sweep boundary departed from its supporting edge",
        ));
    }
    let (u, v) = support.surface.closest_parameters(on_edge, tolerance)?;
    let jet = support.surface.evaluate_with_second_derivatives(u, v)?;
    if jet.point.distance_to(on_edge)? > tolerance.absolute() {
        return Err(unsupported(
            "could not locate the rail on its supporting face",
        ));
    }
    Ok((
        jet,
        support
            .guide
            .derivative_at(t)?
            .normalized_nonzero()?
            .as_vector(),
    ))
}

fn normal_acceleration(jet: SurfaceJet2, normal: Vector3, first: Vector3) -> Result<f64, Error> {
    let u = jet.derivative_u;
    let v = jet.derivative_v;
    let e = u.dot(u)?;
    let f = u.dot(v)?;
    let g = v.dot(v)?;
    let det = e * g - f * f;
    if det <= 0. || !det.is_finite() {
        return Err(unsupported("singular support frame for edge curvature"));
    }
    let a = (g * u.dot(first)? - f * v.dot(first)?) / det;
    let b = (e * v.dot(first)? - f * u.dot(first)?) / det;
    Ok(a * a * normal.dot(jet.derivative_uu)?
        + 2. * a * b * normal.dot(jet.derivative_uv)?
        + b * b * normal.dot(jet.derivative_vv)?)
}

fn push(targets: &mut Vec<f64>, offset: Vector3, weight: f64) {
    targets.extend(offset.to_array().map(|value| value * weight));
    targets.push(0.);
}

fn add(a: Vector3, b: Vector3) -> Result<Vector3, Error> {
    Ok(Vector3::try_new(
        a.x() + b.x(),
        a.y() + b.y(),
        a.z() + b.z(),
    )?)
}
