use nalgebra::{Matrix3, Vector3 as NalgebraVector3};

use crate::{BoundingBox3, GeometryError, NurbsCurve, NurbsSurface, Point3, Real, Tolerance};

const MAX_CURVE_SURFACE_NODE_PAIRS: usize = 1_000_000;
const MAX_CURVE_SURFACE_DEPTH: u8 = 56;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CurveSurfaceIntersection {
    pub(crate) curve_parameter: Real,
    pub(crate) u: Real,
    pub(crate) v: Real,
    pub(crate) point: Point3,
    distance: Real,
}

#[derive(Clone, Debug)]
struct CurveNode {
    curve: NurbsCurve,
    bounds: BoundingBox3,
    convex_hull_bounds: bool,
    depth: u8,
}

impl CurveNode {
    fn new(curve: NurbsCurve, depth: u8) -> Self {
        let convex_hull_bounds = weights_have_common_sign(
            curve
                .control_points()
                .iter()
                .map(|control| control.weight()),
        );
        let bounds = curve.control_point_bounds();
        Self {
            curve,
            bounds,
            convex_hull_bounds,
            depth,
        }
    }

    fn split(self) -> Result<[Self; 2], GeometryError> {
        let domain = self.curve.domain();
        let middle = finite_midpoint(*domain.start(), *domain.end());
        let (left, right) = self.curve.try_split(middle)?;
        Ok([
            Self::new(left, self.depth + 1),
            Self::new(right, self.depth + 1),
        ])
    }

    fn spatial_size(&self) -> Result<Real, GeometryError> {
        self.bounds.min().distance_to(self.bounds.max())
    }
}

#[derive(Clone, Debug)]
struct SurfaceNode {
    surface: NurbsSurface,
    bounds: BoundingBox3,
    convex_hull_bounds: bool,
    depth_u: u8,
    depth_v: u8,
}

#[derive(Debug)]
struct InitialCurveSurfaceNodes {
    intersections: Vec<CurveSurfaceIntersection>,
    stack: Vec<(CurveNode, SurfaceNode)>,
}

#[derive(Clone, Copy, Debug)]
struct SurfaceClosestRegion {
    seed: [Real; 2],
    u_domain: [Real; 2],
    v_domain: [Real; 2],
}

impl SurfaceNode {
    fn new(surface: NurbsSurface, depth_u: u8, depth_v: u8) -> Self {
        let convex_hull_bounds = weights_have_common_sign(
            surface
                .control_points()
                .iter()
                .map(|control| control.weight()),
        );
        let bounds = surface.control_point_bounds();
        Self {
            surface,
            bounds,
            convex_hull_bounds,
            depth_u,
            depth_v,
        }
    }

    fn split(self) -> Result<[Self; 2], GeometryError> {
        let [size_u, size_v] = self.directional_sizes()?;
        let split_u = if self.depth_u >= MAX_CURVE_SURFACE_DEPTH {
            false
        } else if self.depth_v >= MAX_CURVE_SURFACE_DEPTH {
            true
        } else {
            size_u >= size_v
        };
        if split_u {
            let domain = self.surface.domain_u();
            let middle = finite_midpoint(*domain.start(), *domain.end());
            let (low, high) = self.surface.try_split_u(middle)?;
            Ok([
                Self::new(low, self.depth_u + 1, self.depth_v),
                Self::new(high, self.depth_u + 1, self.depth_v),
            ])
        } else {
            let domain = self.surface.domain_v();
            let middle = finite_midpoint(*domain.start(), *domain.end());
            let (low, high) = self.surface.try_split_v(middle)?;
            Ok([
                Self::new(low, self.depth_u, self.depth_v + 1),
                Self::new(high, self.depth_u, self.depth_v + 1),
            ])
        }
    }

    fn spatial_size(&self) -> Result<Real, GeometryError> {
        self.bounds.min().distance_to(self.bounds.max())
    }

    fn directional_sizes(&self) -> Result<[Real; 2], GeometryError> {
        let mut size_u = 0.0_f64;
        let mut size_v = 0.0_f64;
        for v in 0..self.surface.control_point_count_v() {
            for u in 1..self.surface.control_point_count_u() {
                let previous = self
                    .surface
                    .control_point(u - 1, v)
                    .expect("surface control indices are in range")
                    .point();
                let current = self
                    .surface
                    .control_point(u, v)
                    .expect("surface control indices are in range")
                    .point();
                size_u = size_u.max(previous.distance_to(current)?);
            }
        }
        for u in 0..self.surface.control_point_count_u() {
            for v in 1..self.surface.control_point_count_v() {
                let previous = self
                    .surface
                    .control_point(u, v - 1)
                    .expect("surface control indices are in range")
                    .point();
                let current = self
                    .surface
                    .control_point(u, v)
                    .expect("surface control indices are in range")
                    .point();
                size_v = size_v.max(previous.distance_to(current)?);
            }
        }
        Ok([size_u, size_v])
    }
}

pub(crate) fn curve_surface_intersections(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersection>, GeometryError> {
    let coordinate_scale = curve
        .control_points()
        .iter()
        .chain(surface.control_points())
        .flat_map(|control| control.point().to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale);
    let refinement_tolerance =
        (distance_tolerance * 1.0e-6).max(Real::EPSILON * coordinate_scale * 32.0);
    let leaf_size = distance_tolerance * 2.0;
    let tangent_probe_size = (distance_tolerance * coordinate_scale).sqrt() * 2.0;

    if let Some(intersections) =
        curve_surface_overlap_boundaries(curve, surface, tolerance, distance_tolerance)?
    {
        return Ok(intersections);
    }

    let InitialCurveSurfaceNodes {
        mut intersections,
        mut stack,
    } = initial_curve_surface_nodes(
        curve,
        surface,
        tolerance,
        refinement_tolerance,
        distance_tolerance,
        tangent_probe_size,
    )?;

    let mut processed = 0_usize;
    while let Some((curve_node, surface_node)) = stack.pop() {
        processed += 1;
        if processed > MAX_CURVE_SURFACE_NODE_PAIRS {
            return Err(GeometryError::CurveIntersectionDidNotConverge);
        }
        if curve_node.convex_hull_bounds
            && surface_node.convex_hull_bounds
            && (!bounding_boxes_overlap(curve_node.bounds, surface_node.bounds, distance_tolerance)
                || !control_hulls_overlap_on_local_axes(
                    &curve_node.curve,
                    &surface_node.surface,
                    distance_tolerance,
                )?)
        {
            continue;
        }

        let curve_size = curve_node.spatial_size()?;
        let surface_size = surface_node.spatial_size()?;
        let depths_exhausted = curve_node.depth >= MAX_CURVE_SURFACE_DEPTH
            && surface_node.depth_u >= MAX_CURVE_SURFACE_DEPTH
            && surface_node.depth_v >= MAX_CURVE_SURFACE_DEPTH;
        let leaf = (curve_node.convex_hull_bounds
            && surface_node.convex_hull_bounds
            && curve_size <= leaf_size
            && surface_size <= leaf_size)
            || depths_exhausted;
        let tangent_probe = curve_node.convex_hull_bounds
            && surface_node.convex_hull_bounds
            && curve_size <= tangent_probe_size
            && surface_size <= tangent_probe_size;
        if leaf || tangent_probe {
            let curve_domain = curve_node.curve.domain();
            let u_domain = surface_node.surface.domain_u();
            let v_domain = surface_node.surface.domain_v();
            let curve_seed = finite_midpoint(*curve_domain.start(), *curve_domain.end());
            let u_seed = finite_midpoint(*u_domain.start(), *u_domain.end());
            let v_seed = finite_midpoint(*v_domain.start(), *v_domain.end());
            let mut intersection = refine_curve_surface_intersection(
                curve,
                surface,
                curve_seed,
                u_seed,
                v_seed,
                [*curve_domain.start(), *curve_domain.end()],
                [*u_domain.start(), *u_domain.end()],
                [*v_domain.start(), *v_domain.end()],
                refinement_tolerance,
                distance_tolerance,
            )?;
            if intersection.is_none() && tangent_probe {
                intersection = refine_tangent_curve_surface_intersection(
                    curve,
                    &surface_node.surface,
                    surface,
                    [*curve_domain.start(), *curve_domain.end()],
                    refinement_tolerance,
                    tolerance,
                )?;
            }
            if let Some(intersection) = intersection {
                retain_best_intersection(
                    &mut intersections,
                    intersection,
                    distance_tolerance,
                    tangent_probe_size,
                );
                continue;
            }
            if leaf {
                continue;
            }
        }

        let split_curve = !curve_node.convex_hull_bounds
            || (surface_node.convex_hull_bounds
                && curve_node.depth < MAX_CURVE_SURFACE_DEPTH
                && (curve_size >= surface_size
                    || (surface_node.depth_u >= MAX_CURVE_SURFACE_DEPTH
                        && surface_node.depth_v >= MAX_CURVE_SURFACE_DEPTH)));
        if split_curve && curve_node.depth < MAX_CURVE_SURFACE_DEPTH {
            let [low, high] = curve_node.split()?;
            stack.push((high, surface_node.clone()));
            stack.push((low, surface_node));
        } else if surface_node.depth_u < MAX_CURVE_SURFACE_DEPTH
            || surface_node.depth_v < MAX_CURVE_SURFACE_DEPTH
        {
            let [low, high] = surface_node.split()?;
            stack.push((curve_node.clone(), high));
            stack.push((curve_node, low));
        } else {
            return Err(GeometryError::CurveIntersectionDidNotConverge);
        }
    }
    intersections.sort_by(|left, right| {
        left.curve_parameter
            .total_cmp(&right.curve_parameter)
            .then_with(|| left.u.total_cmp(&right.u))
            .then_with(|| left.v.total_cmp(&right.v))
    });
    Ok(intersections)
}

fn initial_curve_surface_nodes(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
    refinement_tolerance: Real,
    distance_tolerance: Real,
    tangent_merge_distance: Real,
) -> Result<InitialCurveSurfaceNodes, GeometryError> {
    let mut intersections = Vec::new();
    let mut stack = Vec::new();
    for curve_span in curve.spans() {
        let curve_piece = curve.try_trimmed(curve_span.0..=curve_span.1)?;
        let curve_node = CurveNode::new(curve_piece, 0);
        for u_span in surface.spans_u() {
            for v_span in surface.spans_v() {
                let surface_piece =
                    surface.try_trimmed(u_span.0..=u_span.1, v_span.0..=v_span.1)?;
                let surface_node = SurfaceNode::new(surface_piece, 0, 0);
                if curve_node.convex_hull_bounds
                    && surface_node.convex_hull_bounds
                    && !bounding_boxes_overlap(
                        curve_node.bounds,
                        surface_node.bounds,
                        distance_tolerance,
                    )
                {
                    continue;
                }
                let intersection = refine_tangent_curve_surface_intersection(
                    curve,
                    &surface_node.surface,
                    surface,
                    [curve_span.0, curve_span.1],
                    refinement_tolerance,
                    tolerance,
                )?;
                if let Some(intersection) = intersection {
                    retain_best_intersection(
                        &mut intersections,
                        intersection,
                        distance_tolerance,
                        tangent_merge_distance,
                    );
                }

                let curve_parameters = partition_span(
                    curve_span.0,
                    curve_span.1,
                    intersection.map(|hit| hit.curve_parameter),
                );
                let u_parameters =
                    partition_span(u_span.0, u_span.1, intersection.map(|hit| hit.u));
                let v_parameters =
                    partition_span(v_span.0, v_span.1, intersection.map(|hit| hit.v));
                for curve_interval in curve_parameters.windows(2) {
                    let curve_piece = CurveNode::new(
                        curve.try_trimmed(curve_interval[0]..=curve_interval[1])?,
                        0,
                    );
                    for u_interval in u_parameters.windows(2) {
                        for v_interval in v_parameters.windows(2) {
                            stack.push((
                                curve_piece.clone(),
                                SurfaceNode::new(
                                    surface.try_trimmed(
                                        u_interval[0]..=u_interval[1],
                                        v_interval[0]..=v_interval[1],
                                    )?,
                                    0,
                                    0,
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(InitialCurveSurfaceNodes {
        intersections,
        stack,
    })
}

fn partition_span(start: Real, end: Real, addition: Option<Real>) -> Vec<Real> {
    let mut parameters = vec![start, end];
    if let Some(addition) = addition {
        parameters.push(addition.clamp(start, end));
    }
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| parameter_near(*left, *right));
    parameters
}

fn curve_surface_overlap_boundaries(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<Vec<CurveSurfaceIntersection>>, GeometryError> {
    let mut boundary_hits = Vec::new();
    for edge in surface.natural_edge_curves()? {
        for hit in curve.curve_intersections(&edge, tolerance)? {
            let curve_point = curve.evaluate(hit.first_parameter)?;
            let (u, v) = surface.closest_parameters(curve_point, tolerance)?;
            let surface_point = surface.evaluate(u, v)?;
            let intersection = CurveSurfaceIntersection {
                curve_parameter: hit.first_parameter,
                u,
                v,
                point: Point3::try_new(
                    finite_midpoint(curve_point.x(), surface_point.x()),
                    finite_midpoint(curve_point.y(), surface_point.y()),
                    finite_midpoint(curve_point.z(), surface_point.z()),
                )?,
                distance: curve_point.distance_to(surface_point)?,
            };
            if !boundary_hits
                .iter()
                .any(|existing: &CurveSurfaceIntersection| {
                    existing
                        .point
                        .distance_to(intersection.point)
                        .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
                        && parameter_near(existing.curve_parameter, intersection.curve_parameter)
                })
            {
                boundary_hits.push(intersection);
            }
        }
    }
    if boundary_hits.is_empty() {
        return Ok(None);
    }
    boundary_hits.sort_by(|left, right| left.curve_parameter.total_cmp(&right.curve_parameter));

    let curve_domain = curve.domain();
    let mut breakpoints = Vec::with_capacity(boundary_hits.len() + 2);
    breakpoints.push((*curve_domain.start(), None));
    breakpoints.extend(
        boundary_hits
            .iter()
            .enumerate()
            .map(|(index, hit)| (hit.curve_parameter, Some(index))),
    );
    breakpoints.push((*curve_domain.end(), None));
    breakpoints.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut overlaps_boundary = vec![false; boundary_hits.len()];
    for interval in breakpoints.windows(2) {
        let parameter_scale = interval[0].0.abs().max(interval[1].0.abs()).max(1.0);
        if interval[1].0 - interval[0].0 <= Real::EPSILON * parameter_scale * 256.0 {
            continue;
        }
        if curve_interval_lies_on_surface(
            curve,
            surface,
            [interval[0].0, interval[1].0],
            tolerance,
            distance_tolerance,
        )? {
            if let Some(index) = interval[0].1 {
                overlaps_boundary[index] = true;
            }
            if let Some(index) = interval[1].1 {
                overlaps_boundary[index] = true;
            }
        }
    }
    if !overlaps_boundary.iter().any(|overlaps| *overlaps) {
        return Ok(None);
    }
    Ok(Some(
        boundary_hits
            .into_iter()
            .zip(overlaps_boundary)
            .filter_map(|(hit, overlaps)| overlaps.then_some(hit))
            .collect(),
    ))
}

fn curve_interval_lies_on_surface(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    domain: [Real; 2],
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    const MAX_OVERLAP_CERTIFICATE_SAMPLES: usize = 4096;
    // A generic tensor-product surface of bidegree (m, n) has implicit
    // degree at most 2mn. More than 2pmn common points with a degree-p
    // rational curve certify a shared component rather than isolated hits.
    let Some(intersection_bound) = curve
        .degree()
        .checked_mul(surface.degree_u())
        .and_then(|degree| degree.checked_mul(surface.degree_v()))
        .and_then(|degree| degree.checked_mul(2))
    else {
        return Ok(false);
    };
    let sample_count = intersection_bound.saturating_add(1).max(4);
    if sample_count > MAX_OVERLAP_CERTIFICATE_SAMPLES {
        return Ok(false);
    }
    for sample in 1..=sample_count {
        let fraction = sample as Real / (sample_count + 1) as Real;
        let parameter = interpolate_parameter(domain[0], domain[1], fraction);
        let curve_point = curve.evaluate(parameter)?;
        let (u, v) = surface.closest_parameters(curve_point, tolerance)?;
        if curve_point.distance_to(surface.evaluate(u, v)?)? > distance_tolerance {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn refine_curve_surface_intersection(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    mut curve_parameter: Real,
    mut u: Real,
    mut v: Real,
    curve_domain: [Real; 2],
    u_domain: [Real; 2],
    v_domain: [Real; 2],
    refinement_tolerance: Real,
    acceptance_tolerance: Real,
) -> Result<Option<CurveSurfaceIntersection>, GeometryError> {
    let mut distance = curve
        .evaluate(curve_parameter)?
        .distance_to(surface.evaluate(u, v)?)?;
    let mut used_regular_step = false;
    for _ in 0..80 {
        let (curve_point, curve_derivative) = curve.evaluate_with_derivative(curve_parameter)?;
        let (surface_point, derivative_u, derivative_v) =
            surface.evaluate_with_derivatives(u, v)?;
        let residual = surface_point.vector_to(curve_point)?;
        if distance <= refinement_tolerance {
            break;
        }
        let curve_speed = curve_derivative.length()?;
        let u_speed = derivative_u.length()?;
        let v_speed = derivative_v.length()?;
        if curve_speed == 0.0 || u_speed == 0.0 || v_speed == 0.0 {
            break;
        }
        let curve_values = curve_derivative.to_array();
        let u_values = derivative_u.to_array();
        let v_values = derivative_v.to_array();
        let matrix = Matrix3::from_columns(&[
            NalgebraVector3::from_row_slice(&curve_values) / curve_speed,
            -NalgebraVector3::from_row_slice(&u_values) / u_speed,
            -NalgebraVector3::from_row_slice(&v_values) / v_speed,
        ]);
        let residual_values = residual.to_array();
        let Some(step) = matrix
            .lu()
            .solve(&-NalgebraVector3::from_row_slice(&residual_values))
        else {
            break;
        };
        used_regular_step = true;
        let deltas = [step[0] / curve_speed, step[1] / u_speed, step[2] / v_speed];
        if deltas.iter().any(|value| !value.is_finite()) {
            break;
        }

        let mut factor: Real = 1.0;
        let mut accepted = None;
        for _ in 0..28 {
            let next_curve = factor
                .mul_add(deltas[0], curve_parameter)
                .clamp(curve_domain[0], curve_domain[1]);
            let next_u = factor.mul_add(deltas[1], u).clamp(u_domain[0], u_domain[1]);
            let next_v = factor.mul_add(deltas[2], v).clamp(v_domain[0], v_domain[1]);
            if next_curve == curve_parameter && next_u == u && next_v == v {
                break;
            }
            let next_distance = curve
                .evaluate(next_curve)?
                .distance_to(surface.evaluate(next_u, next_v)?)?;
            if next_distance <= distance {
                accepted = Some((next_curve, next_u, next_v, next_distance));
                break;
            }
            factor *= 0.5;
        }
        let Some((next_curve, next_u, next_v, next_distance)) = accepted else {
            break;
        };
        curve_parameter = next_curve;
        u = next_u;
        v = next_v;
        distance = next_distance;
    }

    let curve_point = curve.evaluate(curve_parameter)?;
    let surface_point = surface.evaluate(u, v)?;
    let distance = curve_point.distance_to(surface_point)?;
    let allowed_distance = if used_regular_step {
        refinement_tolerance
    } else {
        acceptance_tolerance
    };
    if distance > allowed_distance {
        return Ok(None);
    }
    let point = Point3::try_new(
        finite_midpoint(curve_point.x(), surface_point.x()),
        finite_midpoint(curve_point.y(), surface_point.y()),
        finite_midpoint(curve_point.z(), surface_point.z()),
    )?;
    Ok(Some(CurveSurfaceIntersection {
        curve_parameter,
        u,
        v,
        point,
        distance,
    }))
}

fn refine_tangent_curve_surface_intersection(
    curve: &NurbsCurve,
    search_surface: &NurbsSurface,
    refinement_surface: &NurbsSurface,
    domain: [Real; 2],
    refinement_tolerance: Real,
    tolerance: Tolerance,
) -> Result<Option<CurveSurfaceIntersection>, GeometryError> {
    const GOLDEN_FRACTION: Real = 0.618_033_988_749_894_9;
    let closest_tolerance = Tolerance::try_new(
        refinement_tolerance,
        Real::EPSILON * 16.0,
        tolerance.angular(),
    )?;
    let mut left = domain[0];
    let mut right = domain[1];
    let u_domain = search_surface.domain_u();
    let v_domain = search_surface.domain_v();
    let u_domain = [*u_domain.start(), *u_domain.end()];
    let v_domain = [*v_domain.start(), *v_domain.end()];
    let u_seed = finite_midpoint(u_domain[0], u_domain[1]);
    let v_seed = finite_midpoint(v_domain[0], v_domain[1]);
    let search_region = SurfaceClosestRegion {
        seed: [u_seed, v_seed],
        u_domain,
        v_domain,
    };
    let mut inner_left = right - GOLDEN_FRACTION * (right - left);
    let mut inner_right = left + GOLDEN_FRACTION * (right - left);
    let closest_at = |curve_parameter| {
        curve_surface_closest_at_parameter(
            curve,
            search_surface,
            curve_parameter,
            search_region,
            closest_tolerance,
        )
    };
    let mut left_hit = closest_at(inner_left)?;
    let mut right_hit = closest_at(inner_right)?;
    let mut best = closest_at(left)?;
    for candidate in [closest_at(right)?, left_hit, right_hit] {
        if candidate.distance < best.distance {
            best = candidate;
        }
    }

    for _ in 0..80 {
        let parameter_scale = left.abs().max(right.abs()).max(1.0);
        if right - left <= Real::EPSILON * parameter_scale * 64.0 {
            break;
        }
        if left_hit.distance <= right_hit.distance {
            right = inner_right;
            inner_right = inner_left;
            right_hit = left_hit;
            inner_left = right - GOLDEN_FRACTION * (right - left);
            left_hit = closest_at(inner_left)?;
            if left_hit.distance < best.distance {
                best = left_hit;
            }
        } else {
            left = inner_left;
            inner_left = inner_right;
            left_hit = right_hit;
            inner_right = left + GOLDEN_FRACTION * (right - left);
            right_hit = closest_at(inner_right)?;
            if right_hit.distance < best.distance {
                best = right_hit;
            }
        }
    }
    let acceptance = refinement_tolerance * 4.0;
    if best.distance > acceptance {
        return Ok(None);
    }
    let Ok((best_at_full_surface, best_tangency)) = curve_surface_tangency_sample(
        curve,
        refinement_surface,
        best.curve_parameter,
        best.u,
        best.v,
        closest_tolerance,
    ) else {
        return Ok(Some(best));
    };
    if let Ok(Some((refined, refined_tangency))) = refine_curve_surface_tangency(
        curve,
        refinement_surface,
        best_at_full_surface,
        best_tangency,
        domain,
        closest_tolerance,
    ) && refined.distance <= acceptance
        && refined_tangency.abs() < best_tangency.abs()
    {
        best = refined;
    }
    Ok(Some(best))
}

fn refine_curve_surface_tangency(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    mut current: CurveSurfaceIntersection,
    mut current_value: Real,
    domain: [Real; 2],
    tolerance: Tolerance,
) -> Result<Option<(CurveSurfaceIntersection, Real)>, GeometryError> {
    for _ in 0..16 {
        if current_value.abs() <= Real::EPSILON * 512.0 {
            break;
        }
        let parameter_scale = current
            .curve_parameter
            .abs()
            .max((domain[1] - domain[0]).abs())
            .max(1.0);
        let difference_step = Real::EPSILON.sqrt() * parameter_scale * 8.0;
        let lower = (current.curve_parameter - difference_step).max(domain[0]);
        let upper = (current.curve_parameter + difference_step).min(domain[1]);
        if lower == upper {
            break;
        }
        let (_, lower_value) =
            curve_surface_tangency_sample(curve, surface, lower, current.u, current.v, tolerance)?;
        let (_, upper_value) =
            curve_surface_tangency_sample(curve, surface, upper, current.u, current.v, tolerance)?;
        let derivative = (upper_value - lower_value) / (upper - lower);
        if !derivative.is_finite() || derivative == 0.0 {
            break;
        }
        let delta = (-current_value / derivative).clamp(
            -(domain[1] - domain[0]) * 0.25,
            (domain[1] - domain[0]) * 0.25,
        );
        if !delta.is_finite() || delta == 0.0 {
            break;
        }

        let mut factor: Real = 1.0;
        let mut accepted = None;
        for _ in 0..20 {
            let parameter = factor
                .mul_add(delta, current.curve_parameter)
                .clamp(domain[0], domain[1]);
            if parameter == current.curve_parameter {
                break;
            }
            let (candidate, candidate_value) = curve_surface_tangency_sample(
                curve, surface, parameter, current.u, current.v, tolerance,
            )?;
            if candidate_value.abs() < current_value.abs() {
                accepted = Some((candidate, candidate_value));
                break;
            }
            factor *= 0.5;
        }
        let Some((next, next_value)) = accepted else {
            break;
        };
        current = next;
        current_value = next_value;
    }
    Ok(Some((current, current_value)))
}

fn curve_surface_tangency_sample(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    curve_parameter: Real,
    u_seed: Real,
    v_seed: Real,
    tolerance: Tolerance,
) -> Result<(CurveSurfaceIntersection, Real), GeometryError> {
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    let region = SurfaceClosestRegion {
        seed: [u_seed, v_seed],
        u_domain: [*u_domain.start(), *u_domain.end()],
        v_domain: [*v_domain.start(), *v_domain.end()],
    };
    let intersection =
        curve_surface_closest_at_parameter(curve, surface, curve_parameter, region, tolerance)?;
    let (_, curve_derivative) = curve.evaluate_with_derivative(curve_parameter)?;
    let (_, derivative_u, derivative_v) =
        surface.evaluate_with_derivatives(intersection.u, intersection.v)?;
    let curve_tangent = curve_derivative.normalized_nonzero()?;
    let surface_normal = derivative_u.cross(derivative_v)?.normalized_nonzero()?;
    let tangency = curve_tangent.as_vector().dot(surface_normal.as_vector())?;
    Ok((intersection, tangency))
}

fn curve_surface_closest_at_parameter(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    curve_parameter: Real,
    region: SurfaceClosestRegion,
    tolerance: Tolerance,
) -> Result<CurveSurfaceIntersection, GeometryError> {
    let curve_point = curve.evaluate(curve_parameter)?;
    let (u, v) = closest_surface_parameters_local(
        surface,
        curve_point,
        region.seed[0],
        region.seed[1],
        region.u_domain,
        region.v_domain,
        tolerance,
    )?;
    let surface_point = surface.evaluate(u, v)?;
    let distance = curve_point.distance_to(surface_point)?;
    let point = Point3::try_new(
        finite_midpoint(curve_point.x(), surface_point.x()),
        finite_midpoint(curve_point.y(), surface_point.y()),
        finite_midpoint(curve_point.z(), surface_point.z()),
    )?;
    Ok(CurveSurfaceIntersection {
        curve_parameter,
        u,
        v,
        point,
        distance,
    })
}

#[allow(clippy::too_many_arguments)]
fn closest_surface_parameters_local(
    surface: &NurbsSurface,
    target: Point3,
    mut u: Real,
    mut v: Real,
    u_domain: [Real; 2],
    v_domain: [Real; 2],
    tolerance: Tolerance,
) -> Result<(Real, Real), GeometryError> {
    u = u.clamp(u_domain[0], u_domain[1]);
    v = v.clamp(v_domain[0], v_domain[1]);
    let mut distance = surface.evaluate(u, v)?.distance_to(target)?;
    for _ in 0..32 {
        let (point, derivative_u, derivative_v) = surface.evaluate_with_derivatives(u, v)?;
        let residual = point.vector_to(target)?;
        let Ok(x_axis) = derivative_u.normalized_nonzero() else {
            break;
        };
        let u_speed = derivative_u.length()?;
        let v_along_x = derivative_v.dot(x_axis.as_vector())?;
        let derivative_v_values = derivative_v.to_array();
        let x_values = x_axis.as_vector().to_array();
        let v_perpendicular = crate::Vector3::try_new(
            (-v_along_x).mul_add(x_values[0], derivative_v_values[0]),
            (-v_along_x).mul_add(x_values[1], derivative_v_values[1]),
            (-v_along_x).mul_add(x_values[2], derivative_v_values[2]),
        )?;
        let Ok(y_axis) = v_perpendicular.normalized_nonzero() else {
            break;
        };
        let v_speed = v_perpendicular.length()?;
        let tangent_x = residual.dot(x_axis.as_vector())?;
        let tangent_y = residual.dot(y_axis.as_vector())?;
        if tangent_x.hypot(tangent_y) <= tolerance.absolute() {
            break;
        }
        let delta_v = tangent_y / v_speed;
        let delta_u = tangent_x / u_speed - v_along_x * delta_v / u_speed;
        if !delta_u.is_finite() || !delta_v.is_finite() {
            break;
        }

        let mut factor: Real = 1.0;
        let mut accepted = None;
        for _ in 0..20 {
            let next_u = factor.mul_add(delta_u, u).clamp(u_domain[0], u_domain[1]);
            let next_v = factor.mul_add(delta_v, v).clamp(v_domain[0], v_domain[1]);
            if next_u == u && next_v == v {
                break;
            }
            let next_distance = surface.evaluate(next_u, next_v)?.distance_to(target)?;
            if next_distance <= distance {
                accepted = Some((next_u, next_v, next_distance));
                break;
            }
            factor *= 0.5;
        }
        let Some((next_u, next_v, next_distance)) = accepted else {
            break;
        };
        u = next_u;
        v = next_v;
        distance = next_distance;
    }
    Ok((u, v))
}

fn retain_best_intersection(
    intersections: &mut Vec<CurveSurfaceIntersection>,
    intersection: CurveSurfaceIntersection,
    distance_tolerance: Real,
    tangent_merge_distance: Real,
) {
    let substantially_better =
        |left: Real, right: Real| (left == 0.0 && right > 0.0) || left * 4.0 < right;
    if intersections.iter().any(|existing| {
        existing
            .point
            .distance_to(intersection.point)
            .is_ok_and(|distance| distance <= tangent_merge_distance)
            && substantially_better(existing.distance, intersection.distance)
    }) {
        return;
    }
    intersections.retain(|existing| {
        !existing
            .point
            .distance_to(intersection.point)
            .is_ok_and(|distance| distance <= tangent_merge_distance)
            || !substantially_better(intersection.distance, existing.distance)
    });
    let duplicate = intersections.iter().position(|existing| {
        existing
            .point
            .distance_to(intersection.point)
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    });
    if let Some(index) = duplicate {
        if intersection.distance < intersections[index].distance {
            intersections[index] = intersection;
        }
    } else {
        intersections.push(intersection);
    }
}

fn weights_have_common_sign(weights: impl Iterator<Item = Real>) -> bool {
    let mut weights = weights;
    let Some(first) = weights.next() else {
        return false;
    };
    weights.all(|weight| weight.is_sign_positive() == first.is_sign_positive())
}

fn bounding_boxes_overlap(first: BoundingBox3, second: BoundingBox3, padding: Real) -> bool {
    let first_min = first.min().to_array();
    let first_max = first.max().to_array();
    let second_min = second.min().to_array();
    let second_max = second.max().to_array();
    (0..3).all(|axis| {
        first_min[axis] <= second_max[axis] + padding
            && second_min[axis] <= first_max[axis] + padding
    })
}

fn control_hulls_overlap_on_local_axes(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    padding: Real,
) -> Result<bool, GeometryError> {
    let curve_domain = curve.domain();
    let curve_parameter = finite_midpoint(*curve_domain.start(), *curve_domain.end());
    let mut axes = vec![curve.derivative_at(curve_parameter)?];
    let u_domain = surface.domain_u();
    let v_domain = surface.domain_v();
    let u_start = *u_domain.start();
    let u_end = *u_domain.end();
    let v_start = *v_domain.start();
    let v_end = *v_domain.end();
    for (u, v) in [
        (u_start, v_start),
        (u_end, v_start),
        (u_end, v_end),
        (u_start, v_end),
        (
            finite_midpoint(u_start, u_end),
            finite_midpoint(v_start, v_end),
        ),
    ] {
        let (_, derivative_u, derivative_v) = surface.evaluate_with_derivatives(u, v)?;
        axes.push(derivative_u);
        axes.push(derivative_v);
        axes.push(derivative_u.cross(derivative_v)?);
    }

    let origin = surface.control_points()[0].point();
    for axis in axes {
        let Ok(axis) = axis.normalized_nonzero() else {
            continue;
        };
        let curve_projection = control_projection_bounds(
            origin,
            curve.control_points().iter().map(|control| control.point()),
            axis.as_vector(),
        )?;
        let surface_projection = control_projection_bounds(
            origin,
            surface
                .control_points()
                .iter()
                .map(|control| control.point()),
            axis.as_vector(),
        )?;
        if curve_projection[0] > surface_projection[1] + padding
            || surface_projection[0] > curve_projection[1] + padding
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn control_projection_bounds(
    origin: Point3,
    points: impl Iterator<Item = Point3>,
    axis: crate::Vector3,
) -> Result<[Real; 2], GeometryError> {
    let mut minimum = Real::INFINITY;
    let mut maximum = Real::NEG_INFINITY;
    for point in points {
        let projection = origin.vector_to(point)?.dot(axis)?;
        minimum = minimum.min(projection);
        maximum = maximum.max(projection);
    }
    Ok([minimum, maximum])
}

fn finite_midpoint(left: Real, right: Real) -> Real {
    if left.is_sign_negative() == right.is_sign_negative() {
        left + (right - left) * 0.5
    } else {
        left * 0.5 + right * 0.5
    }
}

fn interpolate_parameter(start: Real, end: Real, fraction: Real) -> Real {
    if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    }
}

fn parameter_near(left: Real, right: Real) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= Real::EPSILON * scale * 256.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn intersects_a_line_with_a_bilinear_patch() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(20.0, 0.0, 0.0)],
            vec![0.0, 0.0, 20.0, 20.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(10.0, -5.0, -5.0),
            point(10.0, 5.0, -5.0),
            point(10.0, 5.0, 5.0),
            point(10.0, -5.0, 5.0),
        ])
        .unwrap();
        let intersections =
            curve_surface_intersections(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(intersections.len(), 1, "{intersections:#?}");
        assert!((intersections[0].curve_parameter - 10.0).abs() < 1.0e-10);
        assert!(
            intersections[0]
                .point
                .is_near(point(10.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn returns_the_entry_and_exit_of_a_coplanar_surface_overlap() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(20.0, 0.0, 0.0)],
            vec![0.0, 0.0, 20.0, 20.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(10.0, -5.0, 0.0),
            point(15.0, -5.0, 0.0),
            point(15.0, 5.0, 0.0),
            point(10.0, 5.0, 0.0),
        ])
        .unwrap();
        let intersections =
            curve_surface_intersections(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(intersections.len(), 2, "{intersections:#?}");
        assert!((intersections[0].curve_parameter - 10.0).abs() < 1.0e-10);
        assert!((intersections[1].curve_parameter - 15.0).abs() < 1.0e-10);
    }

    #[test]
    fn detects_a_tangent_hit_on_a_rational_cylinder() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 2.0, -2.0, 2.0).unwrap();
        let curve = NurbsCurve::try_new(
            1,
            vec![
                point(-3.0, 2.0, 0.0),
                point(10.416_407_864_998_739, 2.0, 0.0),
            ],
            vec![0.0, 0.0, 13.416_407_864_998_739, 13.416_407_864_998_739],
        )
        .unwrap();
        let intersections =
            curve_surface_intersections(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(intersections.len(), 1, "{intersections:#?}");
        assert!(
            intersections[0].point.x().abs() < 1.0e-12,
            "{intersections:#?}"
        );
        assert!(
            intersections[0]
                .point
                .is_near(point(0.0, 2.0, 0.0), Tolerance::DEFAULT),
            "{intersections:#?}"
        );
    }

    #[test]
    fn detects_a_tangent_inside_a_rational_surface_span() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 2.0, -2.0, 2.0).unwrap();
        let coordinate = 2.0_f64.sqrt();
        let direction = 0.5_f64.sqrt();
        let curve = NurbsCurve::try_new(
            1,
            vec![
                point(
                    coordinate + 3.0 * direction,
                    coordinate - 3.0 * direction,
                    0.0,
                ),
                point(
                    coordinate - 7.0 * direction,
                    coordinate + 7.0 * direction,
                    0.0,
                ),
            ],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        let intersections =
            curve_surface_intersections(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(intersections.len(), 1, "{intersections:#?}");
        assert!(
            intersections[0]
                .point
                .is_near(point(coordinate, coordinate, 0.0), Tolerance::DEFAULT),
            "{intersections:#?}"
        );
    }

    #[test]
    fn intersects_singular_rational_sphere_poles() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, -5.0), point(0.0, 0.0, 5.0)],
            vec![-5.0, -5.0, 5.0, 5.0],
        )
        .unwrap();
        let intersections =
            curve_surface_intersections(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(intersections.len(), 2, "{intersections:#?}");
        assert!(
            intersections[0]
                .point
                .is_near(point(0.0, 0.0, -2.0), Tolerance::DEFAULT),
            "{intersections:#?}"
        );
        assert!(
            intersections[1]
                .point
                .is_near(point(0.0, 0.0, 2.0), Tolerance::DEFAULT),
            "{intersections:#?}"
        );
    }
}
