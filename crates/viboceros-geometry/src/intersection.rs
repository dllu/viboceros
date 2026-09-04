use nalgebra::{Matrix3, Vector3 as NalgebraVector3};

use crate::{
    BoundingBox3, Brep, BrepFace, GeometryError, NurbsCurve, NurbsSurface, Plane, Point3, Real,
    Tolerance, UnitVector3, intersect_three_planes,
};

const MAX_CURVE_SURFACE_NODE_PAIRS: usize = 1_000_000;
const MAX_CURVE_SURFACE_DEPTH: u8 = 56;
const MAX_CURVE_PLANE_ROOT_DEPTH: usize = 64;

/// A point where a finite NURBS curve meets a finite NURBS surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSurfaceIntersection {
    pub(crate) curve_parameter: Real,
    pub(crate) u: Real,
    pub(crate) v: Real,
    pub(crate) point: Point3,
    distance: Real,
}

impl CurveSurfaceIntersection {
    /// Parameter on the intersected curve.
    #[inline]
    pub const fn curve_parameter(self) -> Real {
        self.curve_parameter
    }

    /// Parameter in the surface's first direction.
    #[inline]
    pub const fn u(self) -> Real {
        self.u
    }

    /// Parameter in the surface's second direction.
    #[inline]
    pub const fn v(self) -> Real {
        self.v
    }

    /// Midpoint of the refined curve and surface evaluations.
    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }
}

/// A finite curve interval that lies on a NURBS surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSurfaceOverlap {
    start: CurveSurfaceIntersection,
    end: CurveSurfaceIntersection,
}

impl CurveSurfaceOverlap {
    /// Boundary at the lower curve parameter.
    #[inline]
    pub const fn start(self) -> CurveSurfaceIntersection {
        self.start
    }

    /// Boundary at the higher curve parameter.
    #[inline]
    pub const fn end(self) -> CurveSurfaceIntersection {
        self.end
    }

    /// Increasing source-curve parameter interval occupied by the overlap.
    #[inline]
    pub fn curve_interval(self) -> std::ops::RangeInclusive<Real> {
        self.start.curve_parameter..=self.end.curve_parameter
    }
}

/// A point contact or finite curve interval shared with a NURBS surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveSurfaceIntersectionEvent {
    /// An isolated curve/surface contact.
    Point(CurveSurfaceIntersection),
    /// A finite interval of the source curve that lies on the surface.
    Overlap(CurveSurfaceOverlap),
}

/// A point where a finite NURBS curve meets a trimmed B-rep boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBrepIntersection {
    curve_parameter: Real,
    point: Point3,
}

impl CurveBrepIntersection {
    /// Parameter on the intersected curve.
    #[inline]
    pub const fn curve_parameter(self) -> Real {
        self.curve_parameter
    }

    /// Model-space contact point.
    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }
}

/// A finite source-curve interval shared with a trimmed B-rep boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBrepOverlap {
    start: CurveBrepIntersection,
    end: CurveBrepIntersection,
}

impl CurveBrepOverlap {
    /// Boundary at the lower curve parameter.
    #[inline]
    pub const fn start(self) -> CurveBrepIntersection {
        self.start
    }

    /// Boundary at the higher curve parameter.
    #[inline]
    pub const fn end(self) -> CurveBrepIntersection {
        self.end
    }

    /// Increasing source-curve parameter interval occupied by the overlap.
    #[inline]
    pub fn curve_interval(self) -> std::ops::RangeInclusive<Real> {
        self.start.curve_parameter..=self.end.curve_parameter
    }
}

/// A point contact or finite source-curve interval shared with a trimmed B-rep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveBrepIntersectionEvent {
    /// An isolated curve/B-rep contact.
    Point(CurveBrepIntersection),
    /// A finite interval of the source curve on the B-rep boundary.
    Overlap(CurveBrepOverlap),
}

/// A point or curve shared by two finite NURBS surfaces.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceSurfaceIntersectionEvent {
    /// An isolated contact between the finite surface regions.
    Point(Point3),
    /// A finite intersection-curve component.
    Curve(NurbsCurve),
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

/// Finds curve/surface contacts, representing shared curve intervals by their
/// two boundary points.
pub fn curve_surface_intersections(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersection>, GeometryError> {
    let mut intersections = Vec::new();
    for event in curve_surface_intersection_events(curve, surface, tolerance)? {
        match event {
            CurveSurfaceIntersectionEvent::Point(intersection) => {
                push_unique_curve_surface_intersection(&mut intersections, intersection, tolerance);
            }
            CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                for intersection in [overlap.start, overlap.end] {
                    push_unique_curve_surface_intersection(
                        &mut intersections,
                        intersection,
                        tolerance,
                    );
                }
            }
        }
    }
    intersections.sort_by(compare_curve_surface_intersections);
    Ok(intersections)
}

/// Finds isolated contacts and finite source-curve intervals shared with a
/// NURBS surface.
pub fn curve_surface_intersection_events(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersectionEvent>, GeometryError> {
    let distance_tolerance = curve_surface_distance_tolerance(curve, surface, tolerance);
    let overlaps = curve_surface_overlaps(curve, surface, tolerance, distance_tolerance)?;
    let curve_domain = curve.domain();
    let mut intersections = if overlaps.is_empty() {
        curve_surface_point_intersections(curve, surface, tolerance)?
    } else {
        let mut gap_start = *curve_domain.start();
        let mut intersections = Vec::new();
        for overlap in &overlaps {
            if !intersection_parameter_near(gap_start, overlap.start.curve_parameter) {
                let gap = curve.try_trimmed(gap_start..=overlap.start.curve_parameter)?;
                intersections.extend(curve_surface_point_intersections(&gap, surface, tolerance)?);
            }
            gap_start = overlap.end.curve_parameter;
        }
        if !intersection_parameter_near(gap_start, *curve_domain.end()) {
            let gap = curve.try_trimmed(gap_start..=*curve_domain.end())?;
            intersections.extend(curve_surface_point_intersections(&gap, surface, tolerance)?);
        }
        intersections
    };
    intersections.retain(|intersection| {
        !overlaps.iter().any(|overlap| {
            parameter_inside_interval(
                intersection.curve_parameter,
                overlap.start.curve_parameter,
                overlap.end.curve_parameter,
            )
        })
    });
    intersections.sort_by(compare_curve_surface_intersections);

    let mut events = intersections
        .into_iter()
        .map(CurveSurfaceIntersectionEvent::Point)
        .chain(
            overlaps
                .into_iter()
                .map(CurveSurfaceIntersectionEvent::Overlap),
        )
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        curve_surface_event_parameter(*left).total_cmp(&curve_surface_event_parameter(*right))
    });
    Ok(events)
}

/// Finds isolated contacts and finite source-curve intervals shared with the
/// trimmed faces of a B-rep.
pub fn curve_brep_intersection_events(
    curve: &NurbsCurve,
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<CurveBrepIntersectionEvent>, GeometryError> {
    let distance_tolerance = curve_brep_distance_tolerance(curve, brep, tolerance);
    let mut intersections = Vec::new();
    let mut overlaps = Vec::new();
    for face in brep.faces() {
        for event in curve_surface_intersection_events(curve, face.surface(), tolerance)? {
            match event {
                CurveSurfaceIntersectionEvent::Point(intersection) => {
                    if face.contains_parameters(intersection.u, intersection.v, tolerance)? {
                        push_unique_curve_brep_intersection(
                            &mut intersections,
                            CurveBrepIntersection {
                                curve_parameter: intersection.curve_parameter,
                                point: intersection.point,
                            },
                            distance_tolerance,
                        );
                    }
                }
                CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                    overlaps.extend(curve_brep_face_overlaps(
                        curve,
                        brep,
                        face,
                        overlap,
                        &mut intersections,
                        tolerance,
                        distance_tolerance,
                    )?);
                }
            }
        }
    }

    overlaps.sort_by(compare_curve_brep_overlaps);
    let overlaps = merge_curve_brep_overlaps(overlaps);
    intersections.retain(|intersection| {
        !overlaps.iter().any(|overlap| {
            parameter_inside_interval(
                intersection.curve_parameter,
                overlap.start.curve_parameter,
                overlap.end.curve_parameter,
            )
        })
    });
    intersections.sort_by(compare_curve_brep_intersections);

    let mut events = intersections
        .into_iter()
        .map(CurveBrepIntersectionEvent::Point)
        .chain(
            overlaps
                .into_iter()
                .map(CurveBrepIntersectionEvent::Overlap),
        )
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        curve_brep_event_parameter(*left).total_cmp(&curve_brep_event_parameter(*right))
    });
    Ok(events)
}

/// Intersects two finite NURBS surfaces.
///
/// The current exact path handles transverse planar surfaces, including
/// multiple clipped components and isolated boundary contacts. Parallel
/// disjoint planes return no events. Non-planar and coincident planar inputs
/// are reported explicitly until their curve and area-overlap paths are
/// implemented.
pub fn surface_surface_intersection_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_plane =
        first
            .plane(tolerance)?
            .ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            })?;
    let second_plane =
        second
            .plane(tolerance)?
            .ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            })?;
    let distance_tolerance = surface_surface_distance_tolerance(first, second, tolerance);
    let direction_vector = first_plane
        .normal()
        .as_vector()
        .cross(second_plane.normal().as_vector())?;
    if direction_vector.length()? <= tolerance.angular() {
        if first_plane.signed_distance_to(second_plane.origin())?.abs() > distance_tolerance * 2.0 {
            return Ok(Vec::new());
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "coincident planar surfaces",
        });
    }
    if !weights_have_common_sign(first.control_points().iter().map(|point| point.weight()))
        || !weights_have_common_sign(second.control_points().iter().map(|point| point.weight()))
    {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "mixed-sign rational weights",
        });
    }
    if !bounding_boxes_overlap(
        first.control_point_bounds(),
        second.control_point_bounds(),
        distance_tolerance * 2.0,
    ) {
        return Ok(Vec::new());
    }

    let direction = direction_vector.normalized_nonzero()?;
    let origin = intersect_three_planes(
        [
            first_plane,
            second_plane,
            Plane::new(Point3::try_new(0.0, 0.0, 0.0)?, direction),
        ],
        tolerance,
    )?;
    let first_range = surface_projection_range(first, origin, direction)?;
    let second_range = surface_projection_range(second, origin, direction)?;
    if first_range[0] > second_range[1] + distance_tolerance * 2.0
        || second_range[0] > first_range[1] + distance_tolerance * 2.0
    {
        return Ok(Vec::new());
    }
    let line_start = first_range[0].min(second_range[0]);
    let line_end = first_range[1].max(second_range[1]);
    if line_end <= line_start {
        return Ok(Vec::new());
    }
    let line = unit_speed_line(origin, direction, line_start, line_end)?;
    let first_intervals =
        curve_surface_event_intervals(curve_surface_intersection_events(&line, first, tolerance)?);
    let second_intervals =
        curve_surface_event_intervals(curve_surface_intersection_events(&line, second, tolerance)?);
    let parameter_scale = line_start.abs().max(line_end.abs()).max(1.0);
    let parameter_tolerance =
        (distance_tolerance * 2.0).max(Real::EPSILON * parameter_scale * 256.0);
    let intervals =
        intersect_parameter_interval_sets(&first_intervals, &second_intervals, parameter_tolerance);

    intervals
        .into_iter()
        .map(|interval| {
            if interval[1] - interval[0] <= parameter_tolerance {
                let parameter = finite_midpoint(interval[0], interval[1]);
                Ok(SurfaceSurfaceIntersectionEvent::Point(point_on_line(
                    origin, direction, parameter,
                )?))
            } else {
                let start = point_on_line(origin, direction, interval[0])?;
                let end = point_on_line(origin, direction, interval[1])?;
                let length = interval[1] - interval[0];
                Ok(SurfaceSurfaceIntersectionEvent::Curve(NurbsCurve::try_new(
                    1,
                    vec![start, end],
                    vec![0.0, 0.0, length, length],
                )?))
            }
        })
        .collect()
}

fn surface_projection_range(
    surface: &NurbsSurface,
    origin: Point3,
    direction: UnitVector3,
) -> Result<[Real; 2], GeometryError> {
    let mut minimum = Real::INFINITY;
    let mut maximum = Real::NEG_INFINITY;
    for control in surface.control_points() {
        let parameter = origin
            .vector_to(control.point())?
            .dot(direction.as_vector())?;
        minimum = minimum.min(parameter);
        maximum = maximum.max(parameter);
    }
    crate::require_finite([minimum, maximum], "planar surface intersection projection")?;
    Ok([minimum, maximum])
}

fn unit_speed_line(
    origin: Point3,
    direction: UnitVector3,
    start: Real,
    end: Real,
) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new(
        1,
        vec![
            point_on_line(origin, direction, start)?,
            point_on_line(origin, direction, end)?,
        ],
        vec![start, start, end, end],
    )
}

fn point_on_line(
    origin: Point3,
    direction: UnitVector3,
    parameter: Real,
) -> Result<Point3, GeometryError> {
    origin.translated(direction.as_vector().scaled(parameter)?)
}

fn curve_surface_event_intervals(events: Vec<CurveSurfaceIntersectionEvent>) -> Vec<[Real; 2]> {
    events
        .into_iter()
        .map(|event| match event {
            CurveSurfaceIntersectionEvent::Point(intersection) => {
                [intersection.curve_parameter, intersection.curve_parameter]
            }
            CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                [overlap.start.curve_parameter, overlap.end.curve_parameter]
            }
        })
        .collect()
}

fn intersect_parameter_interval_sets(
    first: &[[Real; 2]],
    second: &[[Real; 2]],
    tolerance: Real,
) -> Vec<[Real; 2]> {
    let mut result: Vec<[Real; 2]> = Vec::new();
    let mut first_index = 0;
    let mut second_index = 0;
    while first_index < first.len() && second_index < second.len() {
        let first_interval = first[first_index];
        let second_interval = second[second_index];
        let mut start = first_interval[0].max(second_interval[0]);
        let mut end = first_interval[1].min(second_interval[1]);
        if start <= end + tolerance {
            if end < start {
                let contact = finite_midpoint(start, end);
                start = contact;
                end = contact;
            }
            if let Some(previous) = result.last_mut()
                && start <= previous[1] + tolerance
            {
                previous[1] = previous[1].max(end);
            } else {
                result.push([start, end]);
            }
        }

        if first_interval[1] < second_interval[1] - tolerance {
            first_index += 1;
        } else if second_interval[1] < first_interval[1] - tolerance {
            second_index += 1;
        } else {
            first_index += 1;
            second_index += 1;
        }
    }
    result
}

fn surface_surface_distance_tolerance(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
) -> Real {
    let coordinate_scale = first
        .control_points()
        .iter()
        .chain(second.control_points())
        .flat_map(|control| control.point().to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale)
}

fn curve_surface_point_intersections(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersection>, GeometryError> {
    if let Some(plane) = surface.plane(tolerance)? {
        return curve_planar_surface_point_intersections(curve, surface, plane, tolerance);
    }
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

fn curve_planar_surface_point_intersections(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersection>, GeometryError> {
    let distance_tolerance = curve_surface_distance_tolerance(curve, surface, tolerance);
    let (mut parameters, _) = curve_plane_root_parameters(curve, plane)?;
    parameters.extend(curve_surface_boundary_parameters(
        curve,
        surface,
        tolerance,
        distance_tolerance,
    )?);
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| intersection_parameter_near(*left, *right));

    let mut intersections = Vec::new();
    for parameter in parameters {
        let intersection =
            curve_surface_intersection_at_parameter(curve, surface, parameter, tolerance)?;
        if intersection.distance <= distance_tolerance * 2.0 {
            push_unique_curve_surface_intersection_with_distance(
                &mut intersections,
                intersection,
                distance_tolerance,
            );
        }
    }
    intersections.sort_by(compare_curve_surface_intersections);
    Ok(intersections)
}

fn curve_plane_root_parameters(
    curve: &NurbsCurve,
    plane: Plane,
) -> Result<(Vec<Real>, bool), GeometryError> {
    let mut parameters = Vec::new();
    let mut processed = 0_usize;
    let mut has_coplanar_span = false;
    for span in curve.spans() {
        let piece = curve.try_trimmed(span.0..=span.1)?;
        let coefficients = piece
            .control_points()
            .iter()
            .map(|control| {
                let coefficient = plane.signed_distance_to(control.point())? * control.weight();
                crate::require_finite([coefficient], "curve/plane intersection polynomial")?;
                Ok(coefficient)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if coefficients.iter().all(|coefficient| *coefficient == 0.0) {
            has_coplanar_span = true;
        } else {
            collect_curve_plane_roots(
                &coefficients,
                [span.0, span.1],
                0,
                true,
                true,
                &mut parameters,
                &mut processed,
            )?;
        }
    }
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| intersection_parameter_near(*left, *right));
    Ok((parameters, has_coplanar_span))
}

fn collect_curve_plane_roots(
    coefficients: &[Real],
    parameter: [Real; 2],
    depth: usize,
    include_start: bool,
    include_end: bool,
    roots: &mut Vec<Real>,
    processed: &mut usize,
) -> Result<(), GeometryError> {
    *processed = processed.saturating_add(1);
    if *processed > MAX_CURVE_SURFACE_NODE_PAIRS {
        return Err(GeometryError::CurveIntersectionDidNotConverge);
    }
    if include_start && coefficients[0] == 0.0 {
        roots.push(parameter[0]);
    }
    if include_end && coefficients[coefficients.len() - 1] == 0.0 {
        roots.push(parameter[1]);
    }
    if curve_plane_bernstein_sign_changes(coefficients) == 0 {
        return Ok(());
    }
    let middle = finite_midpoint(parameter[0], parameter[1]);
    if depth >= MAX_CURVE_PLANE_ROOT_DEPTH || middle <= parameter[0] || middle >= parameter[1] {
        roots.push(middle);
        return Ok(());
    }
    let (left, right) = subdivide_curve_plane_bernstein_half(coefficients);
    collect_curve_plane_roots(
        &left,
        [parameter[0], middle],
        depth + 1,
        include_start,
        true,
        roots,
        processed,
    )?;
    collect_curve_plane_roots(
        &right,
        [middle, parameter[1]],
        depth + 1,
        false,
        include_end,
        roots,
        processed,
    )?;
    Ok(())
}

fn subdivide_curve_plane_bernstein_half(coefficients: &[Real]) -> (Vec<Real>, Vec<Real>) {
    let degree = coefficients.len() - 1;
    let mut work = coefficients.to_vec();
    let mut left = Vec::with_capacity(coefficients.len());
    let mut right = Vec::with_capacity(coefficients.len());
    left.push(work[0]);
    right.push(work[degree]);
    for level in 1..=degree {
        for index in 0..=degree - level {
            work[index] = finite_midpoint(work[index], work[index + 1]);
        }
        left.push(work[0]);
        right.push(work[degree - level]);
    }
    right.reverse();
    (left, right)
}

fn curve_plane_bernstein_sign_changes(coefficients: &[Real]) -> usize {
    let mut previous = 0_i8;
    let mut changes = 0;
    for coefficient in coefficients {
        let sign = if *coefficient < 0.0 {
            -1
        } else if *coefficient > 0.0 {
            1
        } else {
            continue;
        };
        if previous != 0 && sign != previous {
            changes += 1;
        }
        previous = sign;
    }
    changes
}

fn curve_surface_boundary_parameters(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<Real>, GeometryError> {
    let plane = surface.plane(tolerance)?;
    let mut parameters = Vec::new();
    for edge in surface.natural_edge_curves()? {
        let linear_parameters = if let Some(plane) = plane
            && edge.is_linear_at_zero_tolerance()?
        {
            curve_linear_edge_contact_parameters(
                curve,
                &edge,
                plane,
                tolerance,
                distance_tolerance,
            )?
        } else {
            None
        };
        if let Some(linear_parameters) = linear_parameters {
            parameters.extend(linear_parameters);
        } else {
            parameters.extend(
                curve
                    .intersections_with_curve(&edge, tolerance)?
                    .into_iter()
                    .map(|intersection| intersection.first_parameter()),
            );
        }
    }
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| intersection_parameter_near(*left, *right));
    Ok(parameters)
}

fn curve_linear_edge_contact_parameters(
    curve: &NurbsCurve,
    edge: &NurbsCurve,
    surface_plane: Plane,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<Vec<Real>>, GeometryError> {
    let edge_domain = edge.domain();
    let start = edge.evaluate(*edge_domain.start())?;
    let end = edge.evaluate(*edge_domain.end())?;
    let direction = start.vector_to(end)?;
    let Ok(direction) = direction.normalized_nonzero() else {
        let parameter = curve.closest_parameter(start, tolerance)?;
        let is_contact = curve.evaluate(parameter)?.distance_to(start)? <= distance_tolerance * 2.0;
        return Ok(Some(is_contact.then_some(parameter).into_iter().collect()));
    };
    let perpendicular = direction
        .as_vector()
        .cross(surface_plane.normal().as_vector())?;
    let Ok(normal) = perpendicular.normalized_nonzero() else {
        return Ok(None);
    };
    let edge_plane = Plane::new(start, normal);
    let (roots, has_collinear_span) = curve_plane_root_parameters(curve, edge_plane)?;
    if has_collinear_span {
        return Ok(None);
    }

    let mut parameters = Vec::new();
    for parameter in roots {
        let point = curve.evaluate(parameter)?;
        let edge_parameter = edge.closest_parameter(point, tolerance)?;
        if point.distance_to(edge.evaluate(edge_parameter)?)? <= distance_tolerance * 2.0 {
            parameters.push(parameter);
        }
    }
    Ok(Some(parameters))
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

fn curve_surface_overlaps(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<CurveSurfaceOverlap>, GeometryError> {
    let mut boundary_hits = Vec::new();
    for parameter in
        curve_surface_boundary_parameters(curve, surface, tolerance, distance_tolerance)?
    {
        let intersection =
            curve_surface_intersection_at_parameter(curve, surface, parameter, tolerance)?;
        push_unique_curve_surface_intersection_with_distance(
            &mut boundary_hits,
            intersection,
            distance_tolerance,
        );
    }
    boundary_hits.sort_by(compare_curve_surface_intersections);

    let curve_domain = curve.domain();
    let mut breakpoints = Vec::with_capacity(boundary_hits.len() + 2);
    breakpoints.push(*curve_domain.start());
    breakpoints.extend(boundary_hits.iter().map(|hit| hit.curve_parameter));
    breakpoints.push(*curve_domain.end());
    breakpoints.sort_by(Real::total_cmp);
    breakpoints.dedup_by(|left, right| intersection_parameter_near(*left, *right));

    let mut overlaps: Vec<CurveSurfaceOverlap> = Vec::new();
    for interval in breakpoints.windows(2) {
        let parameter_scale = interval[0].abs().max(interval[1].abs()).max(1.0);
        if interval[1] - interval[0] <= Real::EPSILON * parameter_scale * 256.0 {
            continue;
        }
        if curve_interval_lies_on_surface(
            curve,
            surface,
            [interval[0], interval[1]],
            tolerance,
            distance_tolerance,
        )? {
            let overlap = CurveSurfaceOverlap {
                start: curve_surface_intersection_at_parameter(
                    curve,
                    surface,
                    interval[0],
                    tolerance,
                )?,
                end: curve_surface_intersection_at_parameter(
                    curve,
                    surface,
                    interval[1],
                    tolerance,
                )?,
            };
            if let Some(previous) = overlaps.last_mut()
                && curve_surface_intersections_match(
                    previous.end,
                    overlap.start,
                    distance_tolerance,
                )
            {
                previous.end = overlap.end;
            } else {
                overlaps.push(overlap);
            }
        }
    }
    Ok(overlaps)
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

fn curve_brep_face_overlaps(
    curve: &NurbsCurve,
    brep: &Brep,
    face: &BrepFace,
    overlap: CurveSurfaceOverlap,
    intersections: &mut Vec<CurveBrepIntersection>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<CurveBrepOverlap>, GeometryError> {
    let start = overlap.start.curve_parameter;
    let end = overlap.end.curve_parameter;
    let mut breakpoints = vec![start, end];
    for trim in face.loops().iter().flat_map(|face_loop| face_loop.trims()) {
        if let Some(edge_index) = trim.edge() {
            let edge = &brep.edges()[edge_index];
            for intersection in curve.intersections_with_curve(edge.curve(), tolerance)? {
                let parameter =
                    snap_parameter_to_interval(intersection.first_parameter(), start, end);
                if parameter_inside_interval(parameter, start, end) {
                    breakpoints.push(parameter);
                }
            }
        } else {
            let vertex = brep.vertices()[trim.vertices()[0]].point();
            let parameter = curve.closest_parameter(vertex, tolerance)?;
            if parameter_inside_interval(parameter, start, end)
                && curve.evaluate(parameter)?.distance_to(vertex)? <= distance_tolerance * 2.0
            {
                breakpoints.push(snap_parameter_to_interval(parameter, start, end));
            }
        }
    }
    breakpoints.sort_by(Real::total_cmp);
    breakpoints.dedup_by(|left, right| intersection_parameter_near(*left, *right));

    // An overlap with the untrimmed underlying surface can meet the face's
    // trim region at a single point (for example, a coplanar line tangent to
    // an outer-loop corner). Such a contact has no inside midpoint interval,
    // so preserve every contained breakpoint as a point candidate. Endpoints
    // belonging to actual overlap intervals are removed after global merging.
    for &curve_parameter in &breakpoints {
        let intersection = curve_brep_intersection_at_parameter(curve, curve_parameter)?;
        let (u, v) = face
            .surface()
            .closest_parameters(intersection.point, tolerance)?;
        if intersection
            .point
            .distance_to(face.surface().evaluate(u, v)?)?
            <= distance_tolerance * 2.0
            && face.contains_parameters(u, v, tolerance)?
        {
            push_unique_curve_brep_intersection(intersections, intersection, distance_tolerance);
        }
    }

    let mut result = Vec::new();
    for interval in breakpoints.windows(2) {
        let parameter_scale = interval[0].abs().max(interval[1].abs()).max(1.0);
        if interval[1] - interval[0] <= Real::EPSILON * parameter_scale * 256.0 {
            continue;
        }
        let middle = finite_midpoint(interval[0], interval[1]);
        let point = curve.evaluate(middle)?;
        let (u, v) = face.surface().closest_parameters(point, tolerance)?;
        if point.distance_to(face.surface().evaluate(u, v)?)? <= distance_tolerance * 2.0
            && face.contains_parameters(u, v, tolerance)?
        {
            result.push(CurveBrepOverlap {
                start: curve_brep_intersection_at_parameter(curve, interval[0])?,
                end: curve_brep_intersection_at_parameter(curve, interval[1])?,
            });
        }
    }
    Ok(result)
}

fn snap_parameter_to_interval(parameter: Real, start: Real, end: Real) -> Real {
    if parameter < start && intersection_parameter_near(parameter, start) {
        start
    } else if parameter > end && intersection_parameter_near(parameter, end) {
        end
    } else {
        parameter
    }
}

fn curve_brep_distance_tolerance(curve: &NurbsCurve, brep: &Brep, tolerance: Tolerance) -> Real {
    let coordinate_scale = curve
        .control_points()
        .iter()
        .map(|control| control.point())
        .chain(brep.vertices().iter().map(|vertex| vertex.point()))
        .chain(
            brep.faces()
                .iter()
                .flat_map(|face| face.surface().control_points())
                .map(|control| control.point()),
        )
        .flat_map(Point3::to_array)
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale)
}

fn curve_brep_intersection_at_parameter(
    curve: &NurbsCurve,
    curve_parameter: Real,
) -> Result<CurveBrepIntersection, GeometryError> {
    Ok(CurveBrepIntersection {
        curve_parameter,
        point: curve.evaluate(curve_parameter)?,
    })
}

fn compare_curve_brep_intersections(
    left: &CurveBrepIntersection,
    right: &CurveBrepIntersection,
) -> std::cmp::Ordering {
    left.curve_parameter
        .total_cmp(&right.curve_parameter)
        .then_with(|| compare_points(left.point, right.point))
}

fn compare_curve_brep_overlaps(
    left: &CurveBrepOverlap,
    right: &CurveBrepOverlap,
) -> std::cmp::Ordering {
    compare_curve_brep_intersections(&left.start, &right.start)
        .then_with(|| compare_curve_brep_intersections(&left.end, &right.end))
}

fn compare_points(left: Point3, right: Point3) -> std::cmp::Ordering {
    left.x()
        .total_cmp(&right.x())
        .then_with(|| left.y().total_cmp(&right.y()))
        .then_with(|| left.z().total_cmp(&right.z()))
}

fn merge_curve_brep_overlaps(overlaps: Vec<CurveBrepOverlap>) -> Vec<CurveBrepOverlap> {
    let mut merged: Vec<CurveBrepOverlap> = Vec::with_capacity(overlaps.len());
    for overlap in overlaps {
        if let Some(previous) = merged.last_mut() {
            let intervals_overlap = overlap.start.curve_parameter < previous.end.curve_parameter;
            let intervals_touch = intersection_parameter_near(
                overlap.start.curve_parameter,
                previous.end.curve_parameter,
            );
            if intervals_overlap || intervals_touch {
                if overlap.end.curve_parameter > previous.end.curve_parameter {
                    previous.end = overlap.end;
                }
                continue;
            }
        }
        merged.push(overlap);
    }
    merged
}

fn push_unique_curve_brep_intersection(
    intersections: &mut Vec<CurveBrepIntersection>,
    intersection: CurveBrepIntersection,
    distance_tolerance: Real,
) {
    if !intersections.iter().any(|existing| {
        intersection_parameter_near(existing.curve_parameter, intersection.curve_parameter)
            && existing
                .point
                .distance_to(intersection.point)
                .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    }) {
        intersections.push(intersection);
    }
}

fn curve_brep_event_parameter(event: CurveBrepIntersectionEvent) -> Real {
    match event {
        CurveBrepIntersectionEvent::Point(intersection) => intersection.curve_parameter,
        CurveBrepIntersectionEvent::Overlap(overlap) => overlap.start.curve_parameter,
    }
}

fn curve_surface_distance_tolerance(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Real {
    let coordinate_scale = curve
        .control_points()
        .iter()
        .chain(surface.control_points())
        .flat_map(|control| control.point().to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale)
}

fn curve_surface_intersection_at_parameter(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    curve_parameter: Real,
    tolerance: Tolerance,
) -> Result<CurveSurfaceIntersection, GeometryError> {
    let curve_point = curve.evaluate(curve_parameter)?;
    let (u, v) = surface.closest_parameters(curve_point, tolerance)?;
    let surface_point = surface.evaluate(u, v)?;
    Ok(CurveSurfaceIntersection {
        curve_parameter,
        u,
        v,
        point: Point3::try_new(
            finite_midpoint(curve_point.x(), surface_point.x()),
            finite_midpoint(curve_point.y(), surface_point.y()),
            finite_midpoint(curve_point.z(), surface_point.z()),
        )?,
        distance: curve_point.distance_to(surface_point)?,
    })
}

fn compare_curve_surface_intersections(
    left: &CurveSurfaceIntersection,
    right: &CurveSurfaceIntersection,
) -> std::cmp::Ordering {
    left.curve_parameter
        .total_cmp(&right.curve_parameter)
        .then_with(|| left.u.total_cmp(&right.u))
        .then_with(|| left.v.total_cmp(&right.v))
}

fn curve_surface_event_parameter(event: CurveSurfaceIntersectionEvent) -> Real {
    match event {
        CurveSurfaceIntersectionEvent::Point(intersection) => intersection.curve_parameter,
        CurveSurfaceIntersectionEvent::Overlap(overlap) => overlap.start.curve_parameter,
    }
}

fn push_unique_curve_surface_intersection(
    intersections: &mut Vec<CurveSurfaceIntersection>,
    intersection: CurveSurfaceIntersection,
    tolerance: Tolerance,
) {
    let coordinate_scale = intersection
        .point
        .to_array()
        .into_iter()
        .chain(
            intersections
                .iter()
                .flat_map(|existing| existing.point.to_array()),
        )
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale);
    push_unique_curve_surface_intersection_with_distance(
        intersections,
        intersection,
        distance_tolerance,
    );
}

fn push_unique_curve_surface_intersection_with_distance(
    intersections: &mut Vec<CurveSurfaceIntersection>,
    intersection: CurveSurfaceIntersection,
    distance_tolerance: Real,
) {
    if !intersections.iter().any(|existing| {
        curve_surface_intersections_match(*existing, intersection, distance_tolerance)
    }) {
        intersections.push(intersection);
    }
}

fn curve_surface_intersections_match(
    left: CurveSurfaceIntersection,
    right: CurveSurfaceIntersection,
    distance_tolerance: Real,
) -> bool {
    intersection_parameter_near(left.curve_parameter, right.curve_parameter)
        && left
            .point
            .distance_to(right.point)
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
}

fn parameter_inside_interval(value: Real, start: Real, end: Real) -> bool {
    (value >= start || intersection_parameter_near(value, start))
        && (value <= end || intersection_parameter_near(value, end))
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

fn intersection_parameter_near(left: Real, right: Real) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= Real::EPSILON.sqrt() * scale * 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn horizontal_surface(z: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(0.0, 0.0, z),
            point(10.0, 0.0, z),
            point(10.0, 10.0, z),
            point(0.0, 10.0, z),
        ])
        .unwrap()
    }

    fn vertical_surface(x_start: Real, x_end: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(x_start, 5.0, -5.0),
            point(x_end, 5.0, -5.0),
            point(x_end, 5.0, 5.0),
            point(x_start, 5.0, 5.0),
        ])
        .unwrap()
    }

    #[test]
    fn intersects_transverse_planar_surfaces_with_oriented_exact_lines() {
        let horizontal = horizontal_surface(0.0);
        let vertical = vertical_surface(-5.0, 15.0);
        let events =
            surface_surface_intersection_events(&horizontal, &vertical, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("expected one surface/surface intersection line, got {events:#?}")
        };
        assert_eq!(curve.degree(), 1);
        assert_eq!(curve.domain(), 0.0..=10.0);
        assert!(
            curve
                .evaluate(0.0)
                .unwrap()
                .is_near(point(0.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            curve
                .evaluate(10.0)
                .unwrap()
                .is_near(point(10.0, 5.0, 0.0), Tolerance::DEFAULT)
        );

        let reversed =
            surface_surface_intersection_events(&vertical, &horizontal, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = reversed.as_slice() else {
            panic!("expected one reversed surface/surface line, got {reversed:#?}")
        };
        assert!(
            curve
                .evaluate(0.0)
                .unwrap()
                .is_near(point(10.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            curve
                .evaluate(10.0)
                .unwrap()
                .is_near(point(0.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn clips_planar_surface_intersections_and_retains_endpoint_contacts() {
        let horizontal = horizontal_surface(0.0);
        let partial = surface_surface_intersection_events(
            &horizontal,
            &vertical_surface(2.0, 8.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = partial.as_slice() else {
            panic!("expected one clipped surface/surface line, got {partial:#?}")
        };
        assert_eq!(curve.domain(), 0.0..=6.0);
        assert!(
            curve
                .evaluate(0.0)
                .unwrap()
                .is_near(point(2.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            curve
                .evaluate(6.0)
                .unwrap()
                .is_near(point(8.0, 5.0, 0.0), Tolerance::DEFAULT)
        );

        let endpoint = surface_surface_intersection_events(
            &horizontal,
            &vertical_surface(10.0, 20.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = endpoint.as_slice() else {
            panic!("expected one endpoint surface contact, got {endpoint:#?}")
        };
        assert!(contact.is_near(point(10.0, 5.0, 0.0), Tolerance::DEFAULT));

        assert!(
            surface_surface_intersection_events(
                &horizontal,
                &vertical_surface(10.0 + 1.0e-8, 20.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty(),
            "a gap wider than model tolerance must remain a near miss"
        );
    }

    #[test]
    fn distinguishes_disjoint_and_coincident_planar_surfaces() {
        let horizontal = horizontal_surface(0.0);
        assert!(
            surface_surface_intersection_events(
                &horizontal,
                &horizontal_surface(1.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            surface_surface_intersection_events(&horizontal, &horizontal, Tolerance::DEFAULT,),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident planar surfaces",
            })
        );
    }

    fn box_brep() -> Brep {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        Brep::try_box(
            frame,
            [[0.0, 10.0], [0.0, 10.0], [0.0, 10.0]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn intersects_a_curve_with_trimmed_brep_faces_and_deduplicates_vertices() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(-5.0, -5.0, -5.0), point(15.0, 15.0, 15.0)],
            vec![0.0, 0.0, 20.0, 20.0],
        )
        .unwrap();
        let events =
            curve_brep_intersection_events(&curve, &box_brep(), Tolerance::DEFAULT).unwrap();
        let [
            CurveBrepIntersectionEvent::Point(first),
            CurveBrepIntersectionEvent::Point(second),
        ] = events.as_slice()
        else {
            panic!("expected two deduplicated box vertex hits, got {events:#?}")
        };
        assert!(
            first
                .point()
                .is_near(point(0.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            second
                .point()
                .is_near(point(10.0, 10.0, 10.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn clips_curve_brep_overlaps_to_face_and_shared_edge_boundaries() {
        for (y, z) in [(5.0, 10.0), (0.0, 0.0)] {
            let curve = NurbsCurve::try_new(
                1,
                vec![point(-5.0, y, z), point(15.0, y, z)],
                vec![0.0, 0.0, 20.0, 20.0],
            )
            .unwrap();
            let events =
                curve_brep_intersection_events(&curve, &box_brep(), Tolerance::DEFAULT).unwrap();
            let [CurveBrepIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
                panic!("expected one clipped B-rep overlap, got {events:#?}")
            };
            assert!((overlap.start().curve_parameter() - 5.0).abs() < 1.0e-10);
            assert!((overlap.end().curve_parameter() - 15.0).abs() < 1.0e-10);
            assert!(
                overlap
                    .start()
                    .point()
                    .is_near(point(0.0, y, z), Tolerance::DEFAULT)
            );
            assert!(
                overlap
                    .end()
                    .point()
                    .is_near(point(10.0, y, z), Tolerance::DEFAULT)
            );
        }
    }

    #[test]
    fn curve_inside_a_brep_has_no_boundary_intersection() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(2.0, 5.0, 5.0), point(8.0, 5.0, 5.0)],
            vec![-2.0, -2.0, 4.0, 4.0],
        )
        .unwrap();
        assert!(
            curve_brep_intersection_events(&curve, &box_brep(), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn curve_brep_intersection_respects_planar_face_holes() {
        let outer = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 10.0, 20.0, 30.0, 40.0, 40.0],
        )
        .unwrap();
        let hole = NurbsCurve::try_new(
            1,
            vec![
                point(4.0, 4.0, 0.0),
                point(6.0, 4.0, 0.0),
                point(6.0, 6.0, 0.0),
                point(4.0, 6.0, 0.0),
                point(4.0, 4.0, 0.0),
            ],
            vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0, 8.0],
        )
        .unwrap();
        let brep = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
        let coplanar = NurbsCurve::try_new(
            1,
            vec![point(-1.0, 5.0, 0.0), point(11.0, 5.0, 0.0)],
            vec![0.0, 0.0, 12.0, 12.0],
        )
        .unwrap();

        let events = curve_brep_intersection_events(&coplanar, &brep, Tolerance::DEFAULT).unwrap();
        let [
            CurveBrepIntersectionEvent::Overlap(before_hole),
            CurveBrepIntersectionEvent::Overlap(after_hole),
        ] = events.as_slice()
        else {
            panic!("expected the face hole to split the overlap, got {events:#?}")
        };
        for (actual, expected) in [
            (before_hole.curve_interval(), 1.0..=5.0),
            (after_hole.curve_interval(), 7.0..=11.0),
        ] {
            assert!((*actual.start() - *expected.start()).abs() < 1.0e-10);
            assert!((*actual.end() - *expected.end()).abs() < 1.0e-10);
        }

        let through_hole = NurbsCurve::try_new(
            1,
            vec![point(5.0, 5.0, -1.0), point(5.0, 5.0, 1.0)],
            vec![0.0, 0.0, 2.0, 2.0],
        )
        .unwrap();
        assert!(
            curve_brep_intersection_events(&through_hole, &brep, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reports_an_isolated_coplanar_trim_contact() {
        let outer = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            vec![0.0, 0.0, 10.0, 20.0, 30.0, 40.0, 40.0],
        )
        .unwrap();
        let brep = Brep::try_planar_face_with_holes(&outer, &[], Tolerance::DEFAULT).unwrap();
        let tangent = NurbsCurve::try_new(
            1,
            vec![point(-1.0, 1.0, 0.0), point(1.0, -1.0, 0.0)],
            vec![0.0, 0.0, 2.0, 2.0],
        )
        .unwrap();

        let events = curve_brep_intersection_events(&tangent, &brep, Tolerance::DEFAULT).unwrap();
        let [CurveBrepIntersectionEvent::Point(intersection)] = events.as_slice() else {
            panic!("expected one coplanar trim contact, got {events:#?}")
        };
        assert!((intersection.curve_parameter() - 1.0).abs() < 1.0e-10);
        assert!(
            intersection
                .point()
                .is_near(point(0.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
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

        let events =
            curve_surface_intersection_events(&curve, &surface, Tolerance::DEFAULT).unwrap();
        let [CurveSurfaceIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
            panic!("expected one curve/surface overlap, got {events:#?}")
        };
        assert_eq!(overlap.curve_interval(), 10.0..=15.0);
    }

    #[test]
    fn returns_a_full_overlap_for_a_curve_inside_a_surface() {
        let curve = NurbsCurve::try_new(
            1,
            vec![point(2.0, 5.0, 0.0), point(8.0, 5.0, 0.0)],
            vec![-2.0, -2.0, 4.0, 4.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap();

        let events =
            curve_surface_intersection_events(&curve, &surface, Tolerance::DEFAULT).unwrap();
        let [CurveSurfaceIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
            panic!("expected one full curve/surface overlap, got {events:#?}")
        };
        assert_eq!(overlap.curve_interval(), -2.0..=4.0);
        assert_eq!(overlap.start().point(), point(2.0, 5.0, 0.0));
        assert_eq!(overlap.end().point(), point(8.0, 5.0, 0.0));
    }

    #[test]
    fn combines_a_coplanar_overlap_with_a_later_transverse_hit() {
        let curve = NurbsCurve::try_new(
            1,
            vec![
                point(-5.0, 5.0, 0.0),
                point(15.0, 5.0, 0.0),
                point(5.0, 5.0, 5.0),
                point(5.0, 5.0, -5.0),
            ],
            vec![0.0, 0.0, 20.0, 30.0, 40.0, 40.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap();

        let events =
            curve_surface_intersection_events(&curve, &surface, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2, "{events:#?}");
        let CurveSurfaceIntersectionEvent::Overlap(overlap) = events[0] else {
            panic!("the first event must be the coplanar overlap")
        };
        assert!((overlap.start().curve_parameter() - 5.0).abs() < 1.0e-10);
        assert!((overlap.end().curve_parameter() - 15.0).abs() < 1.0e-10);
        let CurveSurfaceIntersectionEvent::Point(intersection) = events[1] else {
            panic!("the second event must be the transverse contact")
        };
        assert!((intersection.curve_parameter() - 35.0).abs() < 1.0e-10);
        assert!(
            intersection
                .point()
                .is_near(point(5.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn detects_a_quadratic_tangent_to_a_planar_surface() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 5.0, 1.0),
                point(5.0, 5.0, -1.0),
                point(10.0, 5.0, 1.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap();

        let events =
            curve_surface_intersection_events(&curve, &surface, Tolerance::DEFAULT).unwrap();
        let [CurveSurfaceIntersectionEvent::Point(intersection)] = events.as_slice() else {
            panic!("expected one tangent curve/surface point, got {events:#?}")
        };
        assert!((intersection.curve_parameter() - 0.5).abs() < 1.0e-10);
        assert!(
            intersection
                .point()
                .is_near(point(5.0, 5.0, 0.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn detects_an_isolated_coplanar_tangent_to_a_surface_edge() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, -1.0, 0.0),
                point(5.0, 1.0, 0.0),
                point(10.0, -1.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap();

        let events =
            curve_surface_intersection_events(&curve, &surface, Tolerance::DEFAULT).unwrap();
        let [CurveSurfaceIntersectionEvent::Point(intersection)] = events.as_slice() else {
            panic!("expected one coplanar edge tangent, got {events:#?}")
        };
        assert!((intersection.curve_parameter() - 0.5).abs() < 1.0e-10);
        assert!(
            intersection
                .point()
                .is_near(point(5.0, 0.0, 0.0), Tolerance::DEFAULT)
        );
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
