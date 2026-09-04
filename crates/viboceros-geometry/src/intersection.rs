use nalgebra::{Matrix3, Vector3 as NalgebraVector3};

use crate::{
    AffineTransform3, BoundingBox3, Brep, BrepFace, GeometryError, NurbsCurve, NurbsSurface, Plane,
    Point3, Polyline3, Real, Tolerance, UnitVector3, intersect_three_planes, join_polylines,
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

/// A point or curve shared by a finite NURBS surface and a trimmed B-rep.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceBrepIntersectionEvent {
    /// An isolated contact with the trimmed B-rep boundary.
    Point(Point3),
    /// A finite intersection-curve component.
    Curve(NurbsCurve),
}

/// A point or curve shared by two trimmed B-reps.
#[derive(Clone, Debug, PartialEq)]
pub enum BrepBrepIntersectionEvent {
    /// An isolated contact between the trimmed B-rep boundaries.
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
    curve_brep_intersection_events_with_transform(curve, brep, None, tolerance, distance_tolerance)
}

/// Finds curve/B-rep contacts after applying the same affine map to both
/// objects' model-space geometry.
///
/// Face-local trim parameter curves are retained because the transform does
/// not alter surface parameters. This supports view-projected command
/// intersections without constructing an invalid, dimension-collapsed B-rep.
pub fn transformed_curve_brep_intersection_events(
    curve: &NurbsCurve,
    brep: &Brep,
    transform: AffineTransform3,
    tolerance: Tolerance,
) -> Result<Vec<CurveBrepIntersectionEvent>, GeometryError> {
    let curve = curve.transformed(transform)?;
    let distance_tolerance =
        transformed_curve_brep_distance_tolerance(&curve, brep, transform, tolerance)?;
    curve_brep_intersection_events_with_transform(
        &curve,
        brep,
        Some(transform),
        tolerance,
        distance_tolerance,
    )
}

fn curve_brep_intersection_events_with_transform(
    curve: &NurbsCurve,
    brep: &Brep,
    transform: Option<AffineTransform3>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<CurveBrepIntersectionEvent>, GeometryError> {
    let mut intersections = Vec::new();
    let mut overlaps = Vec::new();
    for face in brep.faces() {
        let transformed_surface;
        let surface = if let Some(transform) = transform {
            transformed_surface = face.surface().transformed(transform)?;
            &transformed_surface
        } else {
            face.surface()
        };
        for event in curve_surface_intersection_events(curve, surface, tolerance)? {
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
                        CurveBrepFaceGeometry {
                            brep,
                            face,
                            surface,
                            transform,
                        },
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
/// multiple clipped components and isolated boundary contacts, plus
/// coincident nonsingular convex non-rational four-sided bilinear patches. Coincident
/// patches return their area-overlap perimeter or shared edge; a lone shared
/// corner produces no event, matching Rhino. Parallel disjoint planes return
/// no events. Non-planar and more general coincident inputs are reported
/// explicitly until their intersection-curve paths are implemented.
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
        return coincident_planar_surface_intersection_events(
            first,
            second,
            first_plane,
            tolerance,
            distance_tolerance,
        );
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

/// Intersects a finite NURBS surface with the trimmed faces of a B-rep.
///
/// The current exact path handles planar surfaces against B-reps whose
/// underlying face surfaces are planar. Face-level curves are clipped against
/// exact trim regions, deduplicated across shared topology, and joined into
/// maximal linear components. A coincident face must cover its underlying
/// surface's complete natural domain; more general coincident trim regions are
/// rejected explicitly until planar region Boolean intersection is available.
pub fn surface_brep_intersection_events(
    surface: &NurbsSurface,
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceBrepIntersectionEvent>, GeometryError> {
    let surface_plane =
        surface
            .plane(tolerance)?
            .ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            })?;
    let distance_tolerance = surface_brep_distance_tolerance(surface, brep, tolerance);
    let mut points = Vec::new();
    let mut curves = Vec::new();

    for face in brep.faces() {
        let face_plane = face.surface().plane(tolerance)?.ok_or(
            GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            },
        )?;
        let coincident =
            planes_are_coincident(surface_plane, face_plane, tolerance, distance_tolerance)?;
        let face_events = surface_surface_intersection_events(surface, face.surface(), tolerance)?;
        if coincident
            && !face_events.is_empty()
            && !crate::brep::face_covers_full_surface_domain(face, tolerance)?
        {
            return Err(GeometryError::UnsupportedSurfaceBrepIntersection {
                context: "coincident trimmed face regions",
            });
        }

        for event in face_events {
            match event {
                SurfaceSurfaceIntersectionEvent::Point(point) => {
                    if point_on_brep_face(point, face, tolerance, distance_tolerance)? {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                }
                SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                    let (face_points, face_curves) =
                        clip_curve_to_brep_face(&curve, brep, face, tolerance, distance_tolerance)?;
                    for point in face_points {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                    curves.extend(face_curves);
                }
            }
        }
    }

    let (curves, points) = finalize_linear_brep_intersection_geometry(
        points,
        curves,
        tolerance,
        distance_tolerance,
        GeometryError::UnsupportedSurfaceBrepIntersection {
            context: "joining non-linear face intersection curves",
        },
    )?;

    Ok(curves
        .into_iter()
        .map(SurfaceBrepIntersectionEvent::Curve)
        .chain(points.into_iter().map(SurfaceBrepIntersectionEvent::Point))
        .collect())
}

/// Intersects the trimmed faces of two B-reps.
///
/// The current exact path handles B-reps whose underlying face surfaces are
/// planar. Every face-pair result is clipped against both exact trim regions,
/// then shared-topology duplicates are removed and linear pieces are joined
/// into maximal components. Coincident pairs currently require both faces to
/// cover their complete natural surface domains, with at most one coincident
/// area-overlap pair per B-rep pair.
pub fn brep_brep_intersection_events(
    first: &Brep,
    second: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<BrepBrepIntersectionEvent>, GeometryError> {
    let distance_tolerance = brep_brep_distance_tolerance(first, second, tolerance);
    let mut points = Vec::new();
    let mut curves = Vec::new();
    let mut coincident_area_pairs = 0_usize;

    for first_face in first.faces() {
        let first_plane = first_face.surface().plane(tolerance)?.ok_or(
            GeometryError::UnsupportedBrepBrepIntersection {
                context: "non-planar face surfaces",
            },
        )?;
        for second_face in second.faces() {
            let second_plane = second_face.surface().plane(tolerance)?.ok_or(
                GeometryError::UnsupportedBrepBrepIntersection {
                    context: "non-planar face surfaces",
                },
            )?;
            let face_events = surface_surface_intersection_events(
                first_face.surface(),
                second_face.surface(),
                tolerance,
            )?;
            let coincident = !face_events.is_empty()
                && planes_are_coincident(first_plane, second_plane, tolerance, distance_tolerance)?;
            if coincident {
                if !crate::brep::face_covers_full_surface_domain(first_face, tolerance)?
                    || !crate::brep::face_covers_full_surface_domain(second_face, tolerance)?
                {
                    return Err(GeometryError::UnsupportedBrepBrepIntersection {
                        context: "coincident trimmed face regions",
                    });
                }
                let mut has_area_overlap = false;
                for event in &face_events {
                    if let SurfaceSurfaceIntersectionEvent::Curve(curve) = event
                        && curve.is_closed()?
                    {
                        has_area_overlap = true;
                        break;
                    }
                }
                if has_area_overlap {
                    coincident_area_pairs += 1;
                    if coincident_area_pairs > 1 {
                        return Err(GeometryError::UnsupportedBrepBrepIntersection {
                            context: "multiple coincident face regions",
                        });
                    }
                }
            }

            for event in face_events {
                match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => {
                        if point_on_brep_face(point, first_face, tolerance, distance_tolerance)?
                            && point_on_brep_face(
                                point,
                                second_face,
                                tolerance,
                                distance_tolerance,
                            )?
                        {
                            push_unique_brep_point(&mut points, point, distance_tolerance);
                        }
                    }
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                        let (second_points, second_curves) = clip_curve_to_brep_face(
                            &curve,
                            second,
                            second_face,
                            tolerance,
                            distance_tolerance,
                        )?;
                        for point in second_points {
                            if point_on_brep_face(point, first_face, tolerance, distance_tolerance)?
                            {
                                push_unique_brep_point(&mut points, point, distance_tolerance);
                            }
                        }
                        for second_curve in second_curves {
                            let (first_points, first_curves) = clip_curve_to_brep_face(
                                &second_curve,
                                first,
                                first_face,
                                tolerance,
                                distance_tolerance,
                            )?;
                            for point in first_points {
                                push_unique_brep_point(&mut points, point, distance_tolerance);
                            }
                            curves.extend(first_curves);
                        }
                    }
                }
            }
        }
    }

    let (curves, points) = finalize_linear_brep_intersection_geometry(
        points,
        curves,
        tolerance,
        distance_tolerance,
        GeometryError::UnsupportedBrepBrepIntersection {
            context: "joining non-linear face intersection curves",
        },
    )?;
    Ok(curves
        .into_iter()
        .map(BrepBrepIntersectionEvent::Curve)
        .chain(points.into_iter().map(BrepBrepIntersectionEvent::Point))
        .collect())
}

fn planes_are_coincident(
    first: Plane,
    second: Plane,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    let normals_cross = first
        .normal()
        .as_vector()
        .cross(second.normal().as_vector())?;
    Ok(normals_cross.length()? <= tolerance.angular()
        && first.signed_distance_to(second.origin())?.abs() <= distance_tolerance * 2.0)
}

fn finalize_linear_brep_intersection_geometry(
    points: Vec<Point3>,
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    distance_tolerance: Real,
    non_linear_error: GeometryError,
) -> Result<(Vec<NurbsCurve>, Vec<Point3>), GeometryError> {
    let mut curves =
        join_brep_linear_curves(curves, tolerance, distance_tolerance, non_linear_error)?;
    let mut isolated_points = Vec::with_capacity(points.len());
    for point in points {
        let mut lies_on_curve = false;
        for curve in &curves {
            let parameter = curve.closest_parameter(point, tolerance)?;
            if curve.evaluate(parameter)?.distance_to(point)? <= distance_tolerance * 2.0 {
                lies_on_curve = true;
                break;
            }
        }
        if !lies_on_curve {
            isolated_points.push(point);
        }
    }
    let mut points = isolated_points;
    points.sort_by(|left, right| compare_points(*left, *right));
    curves.sort_by(|left, right| {
        compare_points(
            left.control_points()[0].point(),
            right.control_points()[0].point(),
        )
    });

    Ok((curves, points))
}

fn point_on_brep_face(
    point: Point3,
    face: &BrepFace,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    let (u, v) = face.surface().closest_parameters(point, tolerance)?;
    Ok(
        point.distance_to(face.surface().evaluate(u, v)?)? <= distance_tolerance * 2.0
            && face.contains_parameters(u, v, tolerance)?,
    )
}

fn clip_curve_to_brep_face(
    curve: &NurbsCurve,
    brep: &Brep,
    face: &BrepFace,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<(Vec<Point3>, Vec<NurbsCurve>), GeometryError> {
    let mut intersections = Vec::new();
    let mut overlaps = Vec::new();
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
                    CurveBrepFaceGeometry {
                        brep,
                        face,
                        surface: face.surface(),
                        transform: None,
                    },
                    overlap,
                    &mut intersections,
                    tolerance,
                    distance_tolerance,
                )?);
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
    let points = intersections
        .into_iter()
        .map(|intersection| intersection.point)
        .collect();
    let curves = overlaps
        .into_iter()
        .map(|overlap| curve.try_trimmed(overlap.curve_interval()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((points, curves))
}

fn join_brep_linear_curves(
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    distance_tolerance: Real,
    non_linear_error: GeometryError,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if curves
        .iter()
        .any(|curve| curve.degree() != 1 || curve.is_rational())
    {
        return Err(non_linear_error);
    }

    let mut closed = Vec::new();
    let mut closed_segments = Vec::new();
    for curve in &curves {
        if !curve.is_closed()? {
            continue;
        }
        let segments = linear_curve_segments(curve, tolerance, distance_tolerance)?;
        if closed.iter().any(|existing: &NurbsCurve| {
            linear_closed_curves_match(existing, curve, distance_tolerance)
        }) {
            continue;
        }
        closed_segments.extend(segments);
        closed.push(curve.clone());
    }

    let mut segments = Vec::new();
    for curve in curves {
        if curve.is_closed()? {
            continue;
        }
        for segment in linear_curve_segments(&curve, tolerance, distance_tolerance)? {
            if closed_segments
                .iter()
                .chain(segments.iter())
                .any(|existing| linear_segments_match(existing, &segment, distance_tolerance))
            {
                continue;
            }
            segments.push(segment);
        }
    }
    let mut joined = join_polylines(&segments, tolerance)?
        .into_iter()
        .map(|component| component.polyline().to_nurbs())
        .collect::<Result<Vec<_>, _>>()?;
    closed.append(&mut joined);
    Ok(closed)
}

fn linear_curve_segments(
    curve: &NurbsCurve,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<Polyline3>, GeometryError> {
    let mut segments = Vec::new();
    for controls in curve.control_points().windows(2) {
        let start = controls[0].point();
        let end = controls[1].point();
        if start.distance_to(end)? <= distance_tolerance * 2.0 {
            continue;
        }
        segments.push(Polyline3::try_new(vec![start, end], tolerance)?);
    }
    Ok(segments)
}

fn linear_closed_curves_match(
    first: &NurbsCurve,
    second: &NurbsCurve,
    distance_tolerance: Real,
) -> bool {
    let first = first
        .control_points()
        .windows(2)
        .map(|controls| [controls[0].point(), controls[1].point()])
        .collect::<Vec<_>>();
    let second = second
        .control_points()
        .windows(2)
        .map(|controls| [controls[0].point(), controls[1].point()])
        .collect::<Vec<_>>();
    first.len() == second.len()
        && first.iter().all(|segment| {
            second
                .iter()
                .any(|candidate| point_pairs_match(*segment, *candidate, distance_tolerance))
        })
}

fn linear_segments_match(first: &Polyline3, second: &Polyline3, distance_tolerance: Real) -> bool {
    point_pairs_match(
        [first.vertices()[0], first.vertices()[1]],
        [second.vertices()[0], second.vertices()[1]],
        distance_tolerance,
    )
}

fn point_pairs_match(first: [Point3; 2], second: [Point3; 2], distance_tolerance: Real) -> bool {
    let near = |left: Point3, right: Point3| {
        left.distance_to(right)
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    };
    (near(first[0], second[0]) && near(first[1], second[1]))
        || (near(first[0], second[1]) && near(first[1], second[0]))
}

fn push_unique_brep_point(points: &mut Vec<Point3>, point: Point3, distance_tolerance: Real) {
    if !points.iter().any(|existing| {
        existing
            .distance_to(point)
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    }) {
        points.push(point);
    }
}

#[derive(Clone, Copy, Debug)]
struct ProjectedIntersectionPoint {
    point: Point3,
    x: Real,
    y: Real,
}

fn coincident_planar_surface_intersection_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let unsupported = || GeometryError::UnsupportedSurfaceSurfaceIntersection {
        context: "coincident planar surfaces other than nonsingular convex non-rational four-sided bilinear patches",
    };
    if !is_four_sided_bilinear_patch(first)
        || !is_four_sided_bilinear_patch(second)
        || first.is_rational()
        || second.is_rational()
    {
        return Err(unsupported());
    }

    let mut first_polygon = bilinear_patch_polygon(first);
    let mut second_polygon = bilinear_patch_polygon(second);
    if !orient_and_validate_convex_polygon(&mut first_polygon, plane.normal(), distance_tolerance)?
        || !orient_and_validate_convex_polygon(
            &mut second_polygon,
            plane.normal(),
            distance_tolerance,
        )?
    {
        return Err(unsupported());
    }

    let mut candidates = Vec::new();
    for point in &first_polygon {
        if point_inside_convex_polygon(*point, &second_polygon, plane.normal(), distance_tolerance)?
        {
            push_unique_planar_point(&mut candidates, *point, distance_tolerance);
        }
    }
    for point in &second_polygon {
        if point_inside_convex_polygon(*point, &first_polygon, plane.normal(), distance_tolerance)?
        {
            push_unique_planar_point(&mut candidates, *point, distance_tolerance);
        }
    }
    for first_index in 0..first_polygon.len() {
        let first_edge = [
            first_polygon[first_index],
            first_polygon[(first_index + 1) % first_polygon.len()],
        ];
        for second_index in 0..second_polygon.len() {
            let second_edge = [
                second_polygon[second_index],
                second_polygon[(second_index + 1) % second_polygon.len()],
            ];
            if let Some(point) = planar_segment_intersection(
                first_edge,
                second_edge,
                plane.normal(),
                tolerance,
                distance_tolerance,
            )? {
                push_unique_planar_point(&mut candidates, point, distance_tolerance);
            }
        }
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut hull = planar_convex_hull(candidates, plane.normal(), distance_tolerance)?;
    if hull.len() == 1 {
        // Rhino does not create a point for a zero-area contact between
        // coincident surface regions, unlike a transverse endpoint contact.
        return Ok(Vec::new());
    }
    let domain_start = if hull.len() == 2 {
        if let Some(oriented) =
            coincident_line_orientation(second, [hull[0], hull[1]], tolerance, distance_tolerance)?
                .or(coincident_line_orientation(
                    first,
                    [hull[0], hull[1]],
                    tolerance,
                    distance_tolerance,
                )?)
        {
            hull = oriented.into();
        }
        0.0
    } else {
        let full_boundary = matching_polygon_start(&hull, &first_polygon, distance_tolerance)
            .map(|point_index| (point_index, -*first.domain_v().start()))
            .or_else(|| {
                matching_polygon_start(&hull, &second_polygon, distance_tolerance)
                    .map(|point_index| (point_index, -*second.domain_v().start()))
            });
        let partial_boundary = if full_boundary.is_none() {
            coincident_boundary_start(first, &hull, tolerance, distance_tolerance)?.or(
                coincident_boundary_start(second, &hull, tolerance, distance_tolerance)?,
            )
        } else {
            None
        };
        if let Some((point_index, parameter)) = full_boundary.or(partial_boundary) {
            hull.rotate_left(point_index);
            parameter
        } else {
            0.0
        }
    };
    if hull.len() > 2 {
        hull.push(hull[0]);
    }
    let polyline = Polyline3::try_new(hull, tolerance)?;
    let length = polyline.length()?;
    let domain_end = domain_start + length;
    crate::require_finite(
        [domain_start, domain_end],
        "coincident surface intersection curve domain",
    )?;
    let curve = polyline
        .to_nurbs()?
        .try_reparameterized(domain_start..=domain_end)?;
    Ok(vec![SurfaceSurfaceIntersectionEvent::Curve(curve)])
}

fn is_four_sided_bilinear_patch(surface: &NurbsSurface) -> bool {
    surface.degree_u() == 1
        && surface.degree_v() == 1
        && surface.control_point_count_u() == 2
        && surface.control_point_count_v() == 2
}

fn bilinear_patch_polygon(surface: &NurbsSurface) -> Vec<Point3> {
    [(0, 0), (1, 0), (1, 1), (0, 1)]
        .into_iter()
        .map(|(u, v)| {
            surface
                .control_point(u, v)
                .expect("a bilinear patch has a two-by-two control net")
                .point()
        })
        .collect()
}

fn orient_and_validate_convex_polygon(
    polygon: &mut [Point3],
    normal: UnitVector3,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    let mut signed_twice_area = 0.0;
    let mut perimeter = 0.0;
    let origin = polygon[0];
    for index in 0..polygon.len() {
        let point = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        let edge_length = point.distance_to(next)?;
        if edge_length <= distance_tolerance * 2.0 {
            return Ok(false);
        }
        perimeter += edge_length;
        signed_twice_area += origin
            .vector_to(point)?
            .cross(origin.vector_to(next)?)?
            .dot(normal.as_vector())?;
    }
    crate::require_finite(
        [signed_twice_area, perimeter],
        "coincident surface intersection polygon",
    )?;
    if signed_twice_area.abs() <= distance_tolerance * perimeter * 2.0 {
        return Ok(false);
    }
    if signed_twice_area < 0.0 {
        polygon[1..].reverse();
    }

    for index in 0..polygon.len() {
        let previous = polygon[index];
        let corner = polygon[(index + 1) % polygon.len()];
        let next = polygon[(index + 2) % polygon.len()];
        let edge = previous.vector_to(corner)?;
        let edge_length = edge.length()?;
        let turn_distance = edge
            .cross(corner.vector_to(next)?)?
            .dot(normal.as_vector())?
            / edge_length;
        // A flat or reflex corner makes the bilinear parameterization singular
        // or folded at/near the boundary, so it is not the convex four-sided
        // patch handled by this exact path.
        if turn_distance <= distance_tolerance * 2.0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn point_inside_convex_polygon(
    point: Point3,
    polygon: &[Point3],
    normal: UnitVector3,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let edge = start.vector_to(end)?;
        let signed_distance = edge
            .cross(start.vector_to(point)?)?
            .dot(normal.as_vector())?
            / edge.length()?;
        if signed_distance < -distance_tolerance * 2.0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn planar_segment_intersection(
    first: [Point3; 2],
    second: [Point3; 2],
    normal: UnitVector3,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<Point3>, GeometryError> {
    let first_vector = first[0].vector_to(first[1])?;
    let second_vector = second[0].vector_to(second[1])?;
    let first_length = first_vector.length()?;
    let second_length = second_vector.length()?;
    let first_direction = first_vector.normalized_nonzero()?;
    let second_direction = second_vector.normalized_nonzero()?;
    let denominator = first_direction
        .as_vector()
        .cross(second_direction.as_vector())?
        .dot(normal.as_vector())?;
    if denominator.abs() <= tolerance.angular() {
        return Ok(None);
    }

    let delta = first[0].vector_to(second[0])?;
    let first_distance = delta
        .cross(second_direction.as_vector())?
        .dot(normal.as_vector())?
        / denominator;
    let second_distance = delta
        .cross(first_direction.as_vector())?
        .dot(normal.as_vector())?
        / denominator;
    if first_distance < -distance_tolerance * 2.0
        || first_distance > first_length + distance_tolerance * 2.0
        || second_distance < -distance_tolerance * 2.0
        || second_distance > second_length + distance_tolerance * 2.0
    {
        return Ok(None);
    }

    let first_point = first[0].translated(
        first_direction
            .as_vector()
            .scaled(first_distance.clamp(0.0, first_length))?,
    )?;
    let second_point = second[0].translated(
        second_direction
            .as_vector()
            .scaled(second_distance.clamp(0.0, second_length))?,
    )?;
    Ok(Some(midpoint(first_point, second_point)?))
}

fn push_unique_planar_point(points: &mut Vec<Point3>, point: Point3, distance_tolerance: Real) {
    if !points.iter().any(|existing| {
        existing
            .distance_to(point)
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    }) {
        points.push(point);
    }
}

fn planar_convex_hull(
    points: Vec<Point3>,
    normal: UnitVector3,
    distance_tolerance: Real,
) -> Result<Vec<Point3>, GeometryError> {
    if points.len() <= 1 {
        return Ok(points);
    }
    let origin = points[0];
    let x_axis = origin.vector_to(points[1])?.normalized_nonzero()?;
    let y_axis = normal
        .as_vector()
        .cross(x_axis.as_vector())?
        .normalized_nonzero()?;
    let mut projected = points
        .into_iter()
        .map(|point| {
            let offset = origin.vector_to(point)?;
            Ok(ProjectedIntersectionPoint {
                point,
                x: offset.dot(x_axis.as_vector())?,
                y: offset.dot(y_axis.as_vector())?,
            })
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    projected.sort_by(|left, right| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.y.total_cmp(&right.y))
    });

    let mut lower = Vec::new();
    for point in &projected {
        while projected_hull_turn_is_flat_or_clockwise(&lower, *point, distance_tolerance)? {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper = Vec::new();
    for point in projected.iter().rev() {
        while projected_hull_turn_is_flat_or_clockwise(&upper, *point, distance_tolerance)? {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    Ok(lower.into_iter().map(|point| point.point).collect())
}

fn projected_hull_turn_is_flat_or_clockwise(
    hull: &[ProjectedIntersectionPoint],
    candidate: ProjectedIntersectionPoint,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    let [.., first, second] = hull else {
        return Ok(false);
    };
    let first_edge = first.point.vector_to(second.point)?;
    let second_edge = second.point.vector_to(candidate.point)?;
    let scale = first_edge.length()?.max(second_edge.length()?);
    let turn = (second.x - first.x).mul_add(
        candidate.y - second.y,
        -(second.y - first.y) * (candidate.x - second.x),
    );
    crate::require_finite([turn], "coincident surface intersection hull")?;
    Ok(turn <= distance_tolerance * scale * 2.0)
}

fn coincident_boundary_start(
    surface: &NurbsSurface,
    hull: &[Point3],
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<(usize, Real)>, GeometryError> {
    let mut best: Option<(Real, usize, Real)> = None;
    for (edge_index, edge) in surface.natural_edge_curves()?.into_iter().enumerate() {
        let domain = edge.domain();
        let start = *domain.start();
        let end = *domain.end();
        for (point_index, point) in hull.iter().enumerate() {
            let parameter = edge.closest_parameter(*point, tolerance)?;
            if edge.evaluate(parameter)?.distance_to(*point)? > distance_tolerance * 2.0 {
                continue;
            }
            let fraction = ((parameter - start) / (end - start)).clamp(0.0, 1.0);
            let order = edge_index as Real + fraction;
            if best.is_none_or(|current| order < current.0) {
                best = Some((order, point_index, parameter));
            }
        }
    }
    Ok(best.map(|(_, point_index, parameter)| (point_index, parameter)))
}

fn coincident_line_orientation(
    surface: &NurbsSurface,
    points: [Point3; 2],
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<[Point3; 2]>, GeometryError> {
    for edge in surface.natural_edge_curves()? {
        let first_parameter = edge.closest_parameter(points[0], tolerance)?;
        let second_parameter = edge.closest_parameter(points[1], tolerance)?;
        if edge.evaluate(first_parameter)?.distance_to(points[0])? <= distance_tolerance * 2.0
            && edge.evaluate(second_parameter)?.distance_to(points[1])? <= distance_tolerance * 2.0
        {
            return Ok(Some(if first_parameter <= second_parameter {
                points
            } else {
                [points[1], points[0]]
            }));
        }
    }
    Ok(None)
}

fn matching_polygon_start(
    hull: &[Point3],
    polygon: &[Point3],
    distance_tolerance: Real,
) -> Option<usize> {
    if hull.len() != polygon.len()
        || polygon.iter().any(|point| {
            !hull.iter().any(|candidate| {
                candidate
                    .distance_to(*point)
                    .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
            })
        })
    {
        return None;
    }
    hull.iter().position(|point| {
        point
            .distance_to(polygon[0])
            .is_ok_and(|distance| distance <= distance_tolerance * 2.0)
    })
}

fn midpoint(first: Point3, second: Point3) -> Result<Point3, GeometryError> {
    Point3::try_new(
        finite_midpoint(first.x(), second.x()),
        finite_midpoint(first.y(), second.y()),
        finite_midpoint(first.z(), second.z()),
    )
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

#[derive(Clone, Copy)]
struct CurveBrepFaceGeometry<'a> {
    brep: &'a Brep,
    face: &'a BrepFace,
    surface: &'a NurbsSurface,
    transform: Option<AffineTransform3>,
}

fn curve_brep_face_overlaps(
    curve: &NurbsCurve,
    geometry: CurveBrepFaceGeometry<'_>,
    overlap: CurveSurfaceOverlap,
    intersections: &mut Vec<CurveBrepIntersection>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<CurveBrepOverlap>, GeometryError> {
    let CurveBrepFaceGeometry {
        brep,
        face,
        surface: face_surface,
        transform,
    } = geometry;
    let start = overlap.start.curve_parameter;
    let end = overlap.end.curve_parameter;
    let mut breakpoints = vec![start, end];
    for trim in face.loops().iter().flat_map(|face_loop| face_loop.trims()) {
        if let Some(edge_index) = trim.edge() {
            let edge = &brep.edges()[edge_index];
            let transformed_edge;
            let edge_curve = if let Some(transform) = transform {
                transformed_edge = edge.curve().transformed(transform)?;
                &transformed_edge
            } else {
                edge.curve()
            };
            for intersection in curve.intersections_with_curve(edge_curve, tolerance)? {
                let parameter =
                    snap_parameter_to_interval(intersection.first_parameter(), start, end);
                if parameter_inside_interval(parameter, start, end) {
                    breakpoints.push(parameter);
                }
            }
        } else {
            let mut vertex = brep.vertices()[trim.vertices()[0]].point();
            if let Some(transform) = transform {
                vertex = transform.transform_point(vertex)?;
            }
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
        let (u, v) = face_surface.closest_parameters(intersection.point, tolerance)?;
        if intersection
            .point
            .distance_to(face_surface.evaluate(u, v)?)?
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
        let (u, v) = face_surface.closest_parameters(point, tolerance)?;
        if point.distance_to(face_surface.evaluate(u, v)?)? <= distance_tolerance * 2.0
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

fn transformed_curve_brep_distance_tolerance(
    transformed_curve: &NurbsCurve,
    brep: &Brep,
    transform: AffineTransform3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let mut coordinate_scale = transformed_curve
        .control_points()
        .iter()
        .flat_map(|control| control.point().to_array())
        .fold(1.0_f64, |scale, coordinate| scale.max(coordinate.abs()));
    for point in brep.vertices().iter().map(|vertex| vertex.point()).chain(
        brep.faces()
            .iter()
            .flat_map(|face| face.surface().control_points())
            .map(|control| control.point()),
    ) {
        coordinate_scale = transform
            .transform_point(point)?
            .to_array()
            .into_iter()
            .fold(coordinate_scale, |scale, coordinate| {
                scale.max(coordinate.abs())
            });
    }
    Ok(tolerance
        .absolute()
        .max(tolerance.relative() * coordinate_scale))
}

fn surface_brep_distance_tolerance(
    surface: &NurbsSurface,
    brep: &Brep,
    tolerance: Tolerance,
) -> Real {
    let coordinate_scale = surface
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

fn brep_brep_distance_tolerance(first: &Brep, second: &Brep, tolerance: Tolerance) -> Real {
    let coordinate_scale = first
        .vertices()
        .iter()
        .map(|vertex| vertex.point())
        .chain(second.vertices().iter().map(|vertex| vertex.point()))
        .chain(
            first
                .faces()
                .iter()
                .chain(second.faces())
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
        horizontal_rectangle(0.0, 10.0, 0.0, 10.0, z)
    }

    fn horizontal_rectangle(
        x_start: Real,
        x_end: Real,
        y_start: Real,
        y_end: Real,
        z: Real,
    ) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(x_start, y_start, z),
            point(x_end, y_start, z),
            point(x_end, y_end, z),
            point(x_start, y_end, z),
        ])
        .and_then(|surface| surface.try_reparameterized(x_start..=x_end, y_start..=y_end))
        .unwrap()
    }

    fn vertical_surface(x_start: Real, x_end: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(x_start, 5.0, -5.0),
            point(x_end, 5.0, -5.0),
            point(x_end, 5.0, 5.0),
            point(x_start, 5.0, 5.0),
        ])
        .and_then(|surface| surface.try_reparameterized(x_start..=x_end, -5.0..=5.0))
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
    fn intersects_coincident_nonsingular_convex_bilinear_patches_and_distinguishes_parallel_planes()
    {
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
        let identical =
            surface_surface_intersection_events(&horizontal, &horizontal, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(boundary)] = identical.as_slice() else {
            panic!("identical bilinear patches must return one boundary, got {identical:#?}")
        };
        assert_eq!(boundary.domain(), 0.0..=40.0);
        assert_eq!(
            boundary
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );

        let shifted = horizontal_rectangle(5.0, 15.0, 0.0, 10.0, 0.0);
        let partial =
            surface_surface_intersection_events(&horizontal, &shifted, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(boundary)] = partial.as_slice() else {
            panic!("overlapping bilinear patches must return one boundary, got {partial:#?}")
        };
        assert_eq!(boundary.domain(), 5.0..=35.0);
        assert_eq!(
            boundary
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                point(5.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(5.0, 10.0, 0.0),
                point(5.0, 0.0, 0.0),
            ]
        );

        let contained = surface_surface_intersection_events(
            &horizontal,
            &horizontal_rectangle(2.0, 8.0, 2.0, 8.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(boundary)] = contained.as_slice() else {
            panic!("a contained coincident patch must return its boundary, got {contained:#?}")
        };
        assert_eq!(boundary.domain(), -2.0..=22.0);
        assert_eq!(
            boundary
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                point(2.0, 2.0, 0.0),
                point(8.0, 2.0, 0.0),
                point(8.0, 8.0, 0.0),
                point(2.0, 8.0, 0.0),
                point(2.0, 2.0, 0.0),
            ]
        );

        let rotated = NurbsSurface::try_bilinear([
            point(5.0, -2.0, 0.0),
            point(12.0, 5.0, 0.0),
            point(5.0, 12.0, 0.0),
            point(-2.0, 5.0, 0.0),
        ])
        .unwrap();
        let rotated_overlap =
            surface_surface_intersection_events(&horizontal, &rotated, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(boundary)] = rotated_overlap.as_slice() else {
            panic!("rotated coincident patches must return one boundary, got {rotated_overlap:#?}")
        };
        let expected = [
            point(3.0, 0.0, 0.0),
            point(7.0, 0.0, 0.0),
            point(10.0, 3.0, 0.0),
            point(10.0, 7.0, 0.0),
            point(7.0, 10.0, 0.0),
            point(3.0, 10.0, 0.0),
            point(0.0, 7.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(3.0, 0.0, 0.0),
        ];
        assert_eq!(boundary.control_points().len(), expected.len());
        for (actual, expected) in boundary.control_points().iter().zip(expected) {
            assert!(actual.point().is_near(expected, Tolerance::DEFAULT));
        }
        assert!((*boundary.domain().start() - 3.0).abs() < 1.0e-10);

        let edge_contact = surface_surface_intersection_events(
            &horizontal,
            &horizontal_rectangle(10.0, 20.0, 0.0, 10.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(edge)] = edge_contact.as_slice() else {
            panic!("a shared coincident edge must return one line, got {edge_contact:#?}")
        };
        assert_eq!(edge.domain(), 0.0..=10.0);
        assert_eq!(edge.evaluate(0.0).unwrap(), point(10.0, 10.0, 0.0));
        assert_eq!(edge.evaluate(10.0).unwrap(), point(10.0, 0.0, 0.0));

        let corner_contact = surface_surface_intersection_events(
            &horizontal,
            &horizontal_rectangle(10.0, 20.0, 10.0, 20.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(corner_contact.is_empty());

        assert!(
            surface_surface_intersection_events(
                &horizontal,
                &horizontal_rectangle(11.0, 20.0, 0.0, 10.0, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );

        let singular_boundary = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(5.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap();
        assert_eq!(
            surface_surface_intersection_events(
                &horizontal,
                &singular_boundary,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident planar surfaces other than nonsingular convex non-rational four-sided bilinear patches",
            })
        );

        let quadratic = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(5.0, 10.0, 0.0),
                point(10.0, 10.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        assert_eq!(
            surface_surface_intersection_events(&horizontal, &quadratic, Tolerance::DEFAULT,),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident planar surfaces other than nonsingular convex non-rational four-sided bilinear patches",
            })
        );
    }

    fn box_brep() -> Brep {
        box_brep_with_intervals([[0.0, 10.0], [0.0, 10.0], [0.0, 10.0]])
    }

    fn box_brep_with_intervals(intervals: [[Real; 2]; 3]) -> Brep {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        Brep::try_box(frame, intervals, Tolerance::DEFAULT).unwrap()
    }

    #[test]
    fn intersects_overlapping_planar_breps_as_one_joined_loop() {
        let first = box_brep();
        let second = box_brep_with_intervals([[5.0, 15.0], [5.0, 15.0], [5.0, 15.0]]);
        let events = brep_brep_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        let [BrepBrepIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("overlapping boxes must produce one joined loop, got {events:#?}")
        };
        assert!(curve.is_closed().unwrap());
        assert_eq!(curve.control_points().len(), 7);
        assert_eq!(curve.domain(), 0.0..=30.0);
        let actual = curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>();
        for expected in [
            point(10.0, 5.0, 5.0),
            point(10.0, 10.0, 5.0),
            point(5.0, 10.0, 5.0),
            point(5.0, 10.0, 10.0),
            point(5.0, 5.0, 10.0),
            point(10.0, 5.0, 10.0),
        ] {
            assert!(
                actual
                    .iter()
                    .any(|point| point.is_near(expected, Tolerance::DEFAULT)),
                "missing expected box-intersection vertex {expected:?} from {actual:?}"
            );
        }

        assert!(
            brep_brep_intersection_events(
                &first,
                &box_brep_with_intervals([[2.0, 8.0], [2.0, 8.0], [2.0, 8.0]]),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
        assert!(
            brep_brep_intersection_events(
                &first,
                &box_brep_with_intervals([[20.0, 30.0], [20.0, 30.0], [20.0, 30.0]]),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn brep_brep_intersection_distinguishes_face_edge_and_vertex_contacts() {
        let first = box_brep();
        let face_events = brep_brep_intersection_events(
            &first,
            &box_brep_with_intervals([[10.0, 20.0], [0.0, 10.0], [0.0, 10.0]]),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [BrepBrepIntersectionEvent::Curve(face_boundary)] = face_events.as_slice() else {
            panic!("face-touching boxes must produce one boundary, got {face_events:#?}")
        };
        assert!(face_boundary.is_closed().unwrap());
        assert_eq!(face_boundary.control_points().len(), 5);

        let edge_events = brep_brep_intersection_events(
            &first,
            &box_brep_with_intervals([[10.0, 20.0], [10.0, 20.0], [0.0, 10.0]]),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [BrepBrepIntersectionEvent::Curve(edge)] = edge_events.as_slice() else {
            panic!("edge-touching boxes must produce one line, got {edge_events:#?}")
        };
        assert_eq!(edge.control_points().len(), 2);
        let endpoints = [
            edge.control_points()[0].point(),
            edge.control_points()[1].point(),
        ];
        assert!(endpoints.contains(&point(10.0, 10.0, 0.0)));
        assert!(endpoints.contains(&point(10.0, 10.0, 10.0)));

        let vertex_events = brep_brep_intersection_events(
            &first,
            &box_brep_with_intervals([[10.0, 20.0], [10.0, 20.0], [10.0, 20.0]]),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [BrepBrepIntersectionEvent::Point(contact)] = vertex_events.as_slice() else {
            panic!("vertex-touching boxes must produce one point, got {vertex_events:#?}")
        };
        assert!(contact.is_near(point(10.0, 10.0, 10.0), Tolerance::DEFAULT));

        assert_eq!(
            brep_brep_intersection_events(&first, &first, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedBrepBrepIntersection {
                context: "multiple coincident face regions",
            })
        );
    }

    #[test]
    fn brep_brep_intersection_clips_both_exact_trim_regions() {
        let closed_polyline = |points: Vec<Point3>| {
            Polyline3::try_new(points, Tolerance::DEFAULT)
                .and_then(|polyline| polyline.to_nurbs())
                .unwrap()
        };
        let horizontal_outer = closed_polyline(vec![
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]);
        let horizontal_hole = closed_polyline(vec![
            point(4.0, 4.0, 0.0),
            point(6.0, 4.0, 0.0),
            point(6.0, 6.0, 0.0),
            point(4.0, 6.0, 0.0),
            point(4.0, 4.0, 0.0),
        ]);
        let horizontal = Brep::try_planar_face_with_holes(
            &horizontal_outer,
            &[horizontal_hole],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let vertical_outer = closed_polyline(vec![
            point(0.0, 5.0, -5.0),
            point(10.0, 5.0, -5.0),
            point(10.0, 5.0, 5.0),
            point(0.0, 5.0, 5.0),
            point(0.0, 5.0, -5.0),
        ]);
        let vertical_hole = closed_polyline(vec![
            point(7.0, 5.0, -1.0),
            point(9.0, 5.0, -1.0),
            point(9.0, 5.0, 1.0),
            point(7.0, 5.0, 1.0),
            point(7.0, 5.0, -1.0),
        ]);
        let vertical =
            Brep::try_planar_face_with_holes(&vertical_outer, &[vertical_hole], Tolerance::DEFAULT)
                .unwrap();

        let events =
            brep_brep_intersection_events(&horizontal, &vertical, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 3);
        let mut intervals = events
            .iter()
            .map(|event| {
                let BrepBrepIntersectionEvent::Curve(curve) = event else {
                    panic!("transverse trimmed faces must produce only curve intervals")
                };
                let mut interval = [
                    curve.control_points()[0].point().x(),
                    curve.control_points()[1].point().x(),
                ];
                interval.sort_by(Real::total_cmp);
                interval
            })
            .collect::<Vec<_>>();
        intervals.sort_by(|left, right| left[0].total_cmp(&right[0]));
        for (actual, expected) in intervals.iter().zip([[0.0, 4.0], [6.0, 7.0], [9.0, 10.0]]) {
            assert!((actual[0] - expected[0]).abs() < 1.0e-10);
            assert!((actual[1] - expected[1]).abs() < 1.0e-10);
        }

        assert_eq!(
            brep_brep_intersection_events(&horizontal, &horizontal, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedBrepBrepIntersection {
                context: "coincident trimmed face regions",
            })
        );
    }

    #[test]
    fn intersects_planar_surfaces_with_a_box_and_joins_face_curves() {
        let brep = box_brep();
        let section = horizontal_rectangle(-5.0, 15.0, -5.0, 15.0, 5.0);
        let events = surface_brep_intersection_events(&section, &brep, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("a box section must produce one joined curve, got {events:#?}")
        };
        assert!(curve.is_closed().unwrap());
        assert_eq!(curve.domain(), 0.0..=40.0);
        assert_eq!(
            curve
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 5.0),
                point(10.0, 0.0, 5.0),
                point(10.0, 10.0, 5.0),
                point(0.0, 10.0, 5.0),
                point(0.0, 0.0, 5.0),
            ]
        );

        let partial = surface_brep_intersection_events(
            &horizontal_rectangle(-5.0, 5.0, -5.0, 15.0, 5.0),
            &brep,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(curve)] = partial.as_slice() else {
            panic!("a partial box section must produce one open curve, got {partial:#?}")
        };
        assert!(!curve.is_closed().unwrap());
        assert_eq!(curve.domain(), 0.0..=20.0);
        let endpoints = [
            curve.evaluate(*curve.domain().start()).unwrap(),
            curve.evaluate(*curve.domain().end()).unwrap(),
        ];
        assert!(endpoints.contains(&point(5.0, 0.0, 5.0)));
        assert!(endpoints.contains(&point(5.0, 10.0, 5.0)));

        let coincident = surface_brep_intersection_events(
            &horizontal_rectangle(2.0, 8.0, 2.0, 8.0, 10.0),
            &brep,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(curve)] = coincident.as_slice() else {
            panic!("a contained coincident face must preserve one boundary, got {coincident:#?}")
        };
        assert!(curve.is_closed().unwrap());
        assert_eq!(curve.domain(), -2.0..=22.0);

        assert!(
            surface_brep_intersection_events(
                &horizontal_rectangle(-5.0, 15.0, -5.0, 15.0, 15.0),
                &brep,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn surface_brep_intersection_deduplicates_edge_and_vertex_contacts() {
        let brep = box_brep();
        let edge_surface = NurbsSurface::try_bilinear([
            point(-5.0, -5.0, 5.0),
            point(15.0, -5.0, 5.0),
            point(15.0, 5.0, 15.0),
            point(-5.0, 5.0, 15.0),
        ])
        .unwrap();
        let edge_events =
            surface_brep_intersection_events(&edge_surface, &brep, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(edge)] = edge_events.as_slice() else {
            panic!("a box-edge contact must produce one line, got {edge_events:#?}")
        };
        assert_eq!(edge.control_points().len(), 2);
        let endpoints = [
            edge.control_points()[0].point(),
            edge.control_points()[1].point(),
        ];
        assert!(endpoints.contains(&point(0.0, 0.0, 10.0)));
        assert!(endpoints.contains(&point(10.0, 0.0, 10.0)));

        let vertex_surface = NurbsSurface::try_bilinear([
            point(-5.0, -5.0, 0.0),
            point(15.0, -5.0, 20.0),
            point(15.0, 15.0, 40.0),
            point(-5.0, 15.0, 20.0),
        ])
        .unwrap();
        let vertex_events =
            surface_brep_intersection_events(&vertex_surface, &brep, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Point(contact)] = vertex_events.as_slice() else {
            panic!("a box-vertex contact must produce one point, got {vertex_events:#?}")
        };
        assert!(contact.is_near(point(0.0, 0.0, 10.0), Tolerance::DEFAULT));
    }

    #[test]
    fn surface_brep_intersection_clips_face_holes_and_rejects_coincident_trim_regions() {
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
        let face = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
        let crossing = vertical_surface(-1.0, 11.0);
        let events =
            surface_brep_intersection_events(&crossing, &face, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        let mut intervals = events
            .iter()
            .map(|event| {
                let SurfaceBrepIntersectionEvent::Curve(curve) = event else {
                    panic!("a transverse trimmed-face intersection must contain only curves")
                };
                let mut x = [
                    curve.control_points()[0].point().x(),
                    curve.control_points()[1].point().x(),
                ];
                x.sort_by(Real::total_cmp);
                x
            })
            .collect::<Vec<_>>();
        intervals.sort_by(|left, right| left[0].total_cmp(&right[0]));
        for (actual, expected) in intervals.iter().zip([[0.0, 4.0], [6.0, 10.0]]) {
            assert!((actual[0] - expected[0]).abs() < 1.0e-10);
            assert!((actual[1] - expected[1]).abs() < 1.0e-10);
        }

        assert_eq!(
            surface_brep_intersection_events(&horizontal_surface(0.0), &face, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceBrepIntersection {
                context: "coincident trimmed face regions",
            })
        );
        assert!(
            surface_brep_intersection_events(
                &horizontal_rectangle(20.0, 30.0, 20.0, 30.0, 0.0),
                &face,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
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

        let elevated = NurbsCurve::try_new(
            1,
            vec![point(-1.0, 5.0, 1.0), point(11.0, 5.0, 1.0)],
            vec![0.0, 0.0, 12.0, 12.0],
        )
        .unwrap();
        assert!(
            curve_brep_intersection_events(&elevated, &brep, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        let projection = AffineTransform3::try_planar_projection(Plane::new(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap(),
        ))
        .unwrap();
        let projected = transformed_curve_brep_intersection_events(
            &elevated,
            &brep,
            projection,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [
            CurveBrepIntersectionEvent::Overlap(before_hole),
            CurveBrepIntersectionEvent::Overlap(after_hole),
        ] = projected.as_slice()
        else {
            panic!("projected face hole must split the overlap, got {projected:#?}")
        };
        for (actual, expected) in [
            (before_hole.curve_interval(), 1.0..=5.0),
            (after_hole.curve_interval(), 7.0..=11.0),
        ] {
            assert!((*actual.start() - *expected.start()).abs() < 1.0e-10);
            assert!((*actual.end() - *expected.end()).abs() < 1.0e-10);
        }
        assert_eq!(before_hole.start().point().z(), 0.0);
        assert_eq!(after_hole.end().point().z(), 0.0);

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
