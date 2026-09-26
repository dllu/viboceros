use nalgebra::{Matrix3, Vector3 as NalgebraVector3};

mod bilinear_plane;
mod coincident_brep_graph;
mod cone_cone;
mod cone_cylinder;
mod cone_plane;
mod cylinder_cylinder;
mod sphere_cone;
mod sphere_cylinder_noncoaxial;
mod sphere_cylinder_singular;
mod sphere_cylinder_turning;
mod torus_cone;
mod torus_cylinder;
mod torus_meridian;
mod torus_plane;
mod torus_sphere;
mod torus_torus;

use crate::{
    AffineTransform3, BoundingBox3, Brep, BrepFace, Circle3, Curve3, CurveJoinOptions,
    CurveJoinStyle, GeometryError, NurbsCurve, NurbsSurface, Plane, Point3, Polyline3, Real,
    Tolerance, UnitVector3, WeightedPoint3, intersect_three_planes, join_curves, join_polylines,
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
/// multiple clipped components and isolated boundary contacts. Nonplanar
/// rational bilinear patches with weights of one sign meet finite planes in exact rational conics,
/// plane-contained rulings, or isolated points. The path also handles
/// coincident nonsingular convex four-sided bilinear patches with weights of
/// one sign, plus certified affine and projective patches of any degree. Coincident
/// patches return their area-overlap perimeter or shared edge; a lone shared
/// corner produces no event, matching Rhino. Canonical spheres intersect
/// each other, coaxial cylinders and cones, and planar finite patches in exact
/// rational circles or tangent points. Smooth noncoaxial sphere/cylinder
/// branches are fitted as cubic curves within the modeling tolerance and
/// clipped to the finite cylinder height. Their singular crossing is an exact
/// rational quartic, also clipped to the finite height.
/// Noncoaxial sphere/cone sections form tolerance-bounded cubic curves when
/// the sphere strictly contains the cone apex, clipped at the cone base.
/// A sphere through the apex has an analytic generator branch, fitted to a
/// cubic loop or finite arcs; an apex-only contact is omitted.
/// Spheres outside the apex can form two separate cubic loops, each clipped
/// to arcs or isolated base-rim contacts.
/// At their internal tangency, one nodal cubic curve traverses both loops.
/// Offset spheres outside the apex with two radial turns produce two fitted
/// open branches, likewise clipped at the base.
/// At their external tangency the two branches reduce to one exact point.
/// Planar sections of canonical cylinders produce exact circles, rational
/// ellipses, or straight generatrices, clipped to finite source regions.
/// Parallel canonical cylinder walls intersect in exact finite generatrices,
/// isolated rim points, or a shared rim circle.
/// Equal-radius cylinders with crossing axes intersect in exact
/// rational ellipses clipped to their finite heights.
/// Unequal-radius cylinders with crossing axes intersect in tolerance-bounded
/// cubic curves, also clipped to their finite heights.
/// Skew axes with one cylinder wall strictly inside the other's radial reach
/// produce two separate tolerance-bounded cubic branches, clipped to both heights.
/// Larger axis separation joins the branches into one closed cubic loop; finite
/// heights clip the loop to arcs or isolated rim contacts.
/// At the internal axis tangency, one nodal cubic curve traverses both lobes.
/// Coaxial cone and cylinder walls meet in an exact circle. Parallel offset
/// axes produce tolerance-bounded cubic curves, clipped to both finite rims.
/// A cylinder wall through the cone apex produces a cubic curve with an exact
/// corner at the apex.
/// A perpendicular cylinder axis through the cone apex produces a smooth cubic
/// loop, clipped to both finite height ranges.
/// Coaxial cone walls meet in an exact rational circle when their finite
/// radius profiles cross; coincident wall regions remain unsupported.
/// Parallel offset cones of equal slope meet in an exact conic plane section,
/// clipped to both finite height ranges.
/// Unequal-slope parallel cones meet in bounded-error cubic branches, clipped
/// to both finite height ranges.
/// Cones with different axis directions and a shared apex meet in exact finite
/// generators where their directional circles cross or touch.
/// Canonical tori meet perpendicular or axis-containing planar patches in
/// exact rational circles, clipped to the finite patch.
/// Axis-parallel offset and oblique patches meet tori in fitted cubic loops or arcs.
/// Coaxial torus and finite cylinder walls meet in exact rational circles.
/// Parallel offset cylinder walls meet tori in fitted cubic loops, arcs, or points.
/// Centered perpendicular cylinder walls meet tori in fitted cubic loops,
/// finite arcs, or isolated contacts across their supported radius ranges.
/// Axis-centered spheres and canonical tori meet in exact rational circles.
/// Coaxial canonical tori meet in exact rational circles.
/// Equal parallel offset tori meet in fitted symmetry-plane and elliptic sections.
/// Unequal-major parallel offset tori with matching tube radii meet in fitted
/// lifted hyperbolic and elliptic sections at the same axial level.
/// Equal centered tori with crossed axes meet in two fitted planar sections.
/// Coaxial canonical tori and finite cone walls meet in exact rational circles.
/// Canonical cones produce exact circular, elliptical, parabolic, and hyperbolic sections,
/// plus generators for planes through the apex. The singular apex alone has no
/// intersection event, following Rhino's surface/surface result.
/// Surfaces with separated same-sign rational control-point hulls return no
/// events. Other intersecting non-planar and more general coincident inputs
/// are reported explicitly until their intersection-curve paths are implemented.
pub fn surface_surface_intersection_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    // Same-sign rational weights keep each surface inside its Euclidean
    // control-point hull. This safely rejects separated pairs before the
    // analytic dispatch, including pairs whose nonplanar solver is unfinished.
    if weights_have_common_sign(first.control_points().iter().map(|point| point.weight()))
        && weights_have_common_sign(second.control_points().iter().map(|point| point.weight()))
        && !bounding_boxes_overlap(
            first.control_point_bounds(),
            second.control_point_bounds(),
            surface_surface_distance_tolerance(first, second, tolerance) * 2.0,
        )
    {
        return Ok(Vec::new());
    }
    let first_cone = first.canonical_cone(tolerance)?;
    let second_cone = second.canonical_cone(tolerance)?;
    if let (Some(first_data), Some(second_data)) = (first_cone, second_cone) {
        return cone_cone::cone_cone_intersection_events(first, first_data, second_data, tolerance);
    }
    if let Some((frame, radius, height)) = first_cone
        && let Some(plane) = second.plane(tolerance)?
    {
        return cone_plane::cone_planar_surface_intersection_events(
            first, frame, radius, height, second, plane, tolerance,
        );
    }
    if let Some((frame, radius, height)) = second_cone
        && let Some(plane) = first.plane(tolerance)?
    {
        return cone_plane::cone_planar_surface_intersection_events(
            second, frame, radius, height, first, plane, tolerance,
        );
    }
    let first_cylinder = first.canonical_cylinder(tolerance)?;
    let second_cylinder = second.canonical_cylinder(tolerance)?;
    if let Some((frame, radius, height)) = first_cylinder
        && let Some(plane) = second.plane(tolerance)?
    {
        return cylinder_planar_surface_intersection_events(
            first, frame, radius, height, second, plane, true, tolerance,
        );
    }
    if let Some((frame, radius, height)) = second_cylinder
        && let Some(plane) = first.plane(tolerance)?
    {
        return cylinder_planar_surface_intersection_events(
            second, frame, radius, height, first, plane, false, tolerance,
        );
    }
    if let (Some(first_data), Some(second_data)) = (first_cylinder, second_cylinder) {
        return cylinder_cylinder::cylinder_cylinder_intersection_events(
            first_data,
            second_data,
            tolerance,
        );
    }
    if let (Some(cone_data), Some(cylinder_data)) = (first_cone, second_cylinder) {
        return cone_cylinder::cone_cylinder_intersection_events(
            cone_data,
            cylinder_data,
            tolerance,
        );
    }
    if let (Some(cone_data), Some(cylinder_data)) = (second_cone, first_cylinder) {
        return cone_cylinder::cone_cylinder_intersection_events(
            cone_data,
            cylinder_data,
            tolerance,
        );
    }
    let first_sphere = first.canonical_sphere(tolerance)?;
    let second_sphere = second.canonical_sphere(tolerance)?;
    if let (Some((first_center, first_radius)), Some((second_center, second_radius))) =
        (first_sphere, second_sphere)
    {
        return sphere_sphere_surface_intersection_events(
            first_center,
            first_radius,
            second_center,
            second_radius,
            tolerance,
        );
    }
    if let (Some((center, sphere_radius)), Some(cone_data)) = (first_sphere, second_cone) {
        return sphere_cone::sphere_cone_intersection_events(
            center,
            sphere_radius,
            cone_data,
            tolerance,
        );
    }
    if let (Some((center, sphere_radius)), Some(cone_data)) = (second_sphere, first_cone) {
        return sphere_cone::sphere_cone_intersection_events(
            center,
            sphere_radius,
            cone_data,
            tolerance,
        );
    }
    if let (Some((center, sphere_radius)), Some((frame, cylinder_radius, height))) =
        (first_sphere, second_cylinder)
    {
        return sphere_cylinder_surface_intersection_events(
            center,
            sphere_radius,
            frame,
            cylinder_radius,
            height,
            tolerance,
        );
    }
    if let (Some((center, sphere_radius)), Some((frame, cylinder_radius, height))) =
        (second_sphere, first_cylinder)
    {
        return sphere_cylinder_surface_intersection_events(
            center,
            sphere_radius,
            frame,
            cylinder_radius,
            height,
            tolerance,
        );
    }
    if let Some((center, radius)) = first_sphere
        && let Some(plane) = second.plane(tolerance)?
    {
        return sphere_planar_surface_intersection_events(center, radius, second, plane, tolerance);
    }
    if let Some((center, radius)) = second_sphere
        && let Some(plane) = first.plane(tolerance)?
    {
        return sphere_planar_surface_intersection_events(center, radius, first, plane, tolerance);
    }
    let first_torus = first.canonical_torus(tolerance)?;
    let second_torus = second.canonical_torus(tolerance)?;
    if let (Some(first_data), Some(second_data)) = (first_torus, second_torus) {
        return torus_torus::intersect(first_data, second_data, tolerance);
    }
    if let (Some(torus), Some(cone)) = (first_torus, second_cone) {
        return torus_cone::intersect(torus, cone, tolerance);
    }
    if let (Some(torus), Some(cone)) = (second_torus, first_cone) {
        return torus_cone::intersect(torus, cone, tolerance);
    }
    if let (Some(torus), Some(cylinder)) = (first_torus, second_cylinder) {
        return torus_cylinder::intersect(torus, cylinder, tolerance);
    }
    if let (Some(torus), Some(cylinder)) = (second_torus, first_cylinder) {
        return torus_cylinder::intersect(torus, cylinder, tolerance);
    }
    if let (Some(torus), Some(sphere)) = (first_torus, second_sphere) {
        return torus_sphere::intersect(torus, sphere, tolerance);
    }
    if let (Some(torus), Some(sphere)) = (second_torus, first_sphere) {
        return torus_sphere::intersect(torus, sphere, tolerance);
    }
    if let Some(torus) = first_torus
        && let Some(plane) = second.plane(tolerance)?
    {
        return torus_plane::intersect(torus, second, plane, tolerance);
    }
    if let Some(torus) = second_torus
        && let Some(plane) = first.plane(tolerance)?
    {
        return torus_plane::intersect(torus, first, plane, tolerance);
    }
    let first_plane = first.plane(tolerance)?;
    let second_plane = second.plane(tolerance)?;
    if let Some(plane) = first_plane
        && second_plane.is_none()
        && is_four_sided_bilinear_patch(second)
    {
        return bilinear_plane::intersect(second, first, plane, tolerance);
    }
    if let Some(plane) = second_plane
        && first_plane.is_none()
        && is_four_sided_bilinear_patch(first)
    {
        return bilinear_plane::intersect(first, second, plane, tolerance);
    }
    let first_plane = first_plane.ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
        context: "non-planar surfaces",
    })?;
    let second_plane =
        second_plane.ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
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

fn sphere_cylinder_surface_intersection_events(
    sphere_center: Point3,
    sphere_radius: Real,
    cylinder_frame: crate::Frame3,
    cylinder_radius: Real,
    height: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let [radial_x, radial_y, axial_center] = cylinder_frame.coordinates_of(sphere_center)?;
    let radial_offset = radial_x.hypot(radial_y);
    let coordinate_scale = cylinder_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(sphere_center.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let radial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * sphere_radius.max(cylinder_radius).max(radial_offset));
    let coaxial_tolerance = radial_tolerance.max(8.0 * Real::EPSILON * coordinate_scale);
    let axial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * sphere_radius.max(height))
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let nearest_axial = axial_center.clamp(0.0, height);
    let axial_offset = (axial_center - nearest_axial).abs();
    let nearest_radial = (radial_offset - cylinder_radius).abs();
    if nearest_radial.hypot(axial_offset) > sphere_radius + coaxial_tolerance.max(axial_tolerance) {
        return Ok(Vec::new());
    }
    if radial_offset > coaxial_tolerance {
        return sphere_cylinder_noncoaxial::intersect(
            sphere_center,
            sphere_radius,
            cylinder_frame,
            cylinder_radius,
            height,
            tolerance,
        );
    }
    if cylinder_radius > sphere_radius {
        return Ok(Vec::new());
    }
    let axial_offset =
        ((sphere_radius - cylinder_radius) * (sphere_radius + cylinder_radius)).sqrt();
    let axial_offsets = if axial_offset <= axial_tolerance {
        vec![0.0]
    } else {
        vec![-axial_offset, axial_offset]
    };
    let mut events = Vec::new();
    for offset in axial_offsets {
        let axial_position = axial_center + offset;
        if axial_position < -axial_tolerance || axial_position > height + axial_tolerance {
            continue;
        }
        let center = cylinder_frame.point_at([0.0, 0.0, axial_position.clamp(0.0, height)])?;
        let circle = Circle3::try_from_frame(
            center,
            cylinder_radius,
            cylinder_frame.x_axis(),
            cylinder_frame.z_axis(),
            tolerance,
        )?
        .to_nurbs()?;
        events.push(SurfaceSurfaceIntersectionEvent::Curve(circle));
    }
    Ok(events)
}

fn sphere_sphere_surface_intersection_events(
    first_center: Point3,
    first_radius: Real,
    second_center: Point3,
    second_radius: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let displacement = first_center.vector_to(second_center)?;
    let separation = displacement.length()?;
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * first_radius.max(second_radius).max(separation));
    if separation == 0.0 {
        if (first_radius - second_radius).abs() <= distance_tolerance {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident spheres have a two-dimensional intersection",
            });
        }
        return Ok(Vec::new());
    }
    let radius_sum = first_radius + second_radius;
    let radius_difference = (first_radius - second_radius).abs();
    if separation > radius_sum + distance_tolerance
        || separation < radius_difference - distance_tolerance
    {
        return Ok(Vec::new());
    }
    let axis = displacement.normalized_nonzero()?;
    let axial_distance =
        0.5 * (separation + (first_radius - second_radius) * radius_sum / separation);
    let circle_center = first_center.translated(axis.as_vector().scaled(axial_distance)?)?;
    let squared_radius = (first_radius - axial_distance) * (first_radius + axial_distance);
    if squared_radius <= distance_tolerance * distance_tolerance {
        return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(circle_center)]);
    }
    let circle_frame = crate::Frame3::try_from_normal(circle_center, axis.as_vector(), tolerance)?;
    let circle = Circle3::try_from_frame(
        circle_center,
        squared_radius.sqrt(),
        circle_frame.y_axis(),
        axis.opposite(),
        tolerance,
    )?
    .to_nurbs()?;
    let domain = circle.domain();
    let midpoint = 0.5 * (*domain.start() + *domain.end());
    Ok(vec![
        SurfaceSurfaceIntersectionEvent::Curve(circle.try_subcurve(*domain.start(), midpoint)?),
        SurfaceSurfaceIntersectionEvent::Curve(circle.try_subcurve(midpoint, *domain.end())?),
    ])
}

fn sphere_planar_surface_intersection_events(
    center: Point3,
    radius: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let signed_distance = plane.signed_distance_to(center)?;
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * radius.max(signed_distance.abs()));
    if signed_distance.abs() > radius + distance_tolerance {
        return Ok(Vec::new());
    }
    let circle_center = center.translated(plane.normal().as_vector().scaled(-signed_distance)?)?;
    let squared_radius = (radius - signed_distance.abs()) * (radius + signed_distance.abs());
    if squared_radius <= distance_tolerance * distance_tolerance {
        let (u, v) = planar_surface.closest_parameters(circle_center, tolerance)?;
        return Ok(
            if planar_surface.evaluate(u, v)?.distance_to(circle_center)? <= distance_tolerance {
                vec![SurfaceSurfaceIntersectionEvent::Point(circle_center)]
            } else {
                Vec::new()
            },
        );
    }
    let circle = Circle3::try_new(
        circle_center,
        squared_radius.sqrt(),
        plane.normal(),
        tolerance,
    )?
    .to_nurbs()?;
    intersect_curve_with_planar_surface(&circle, planar_surface, tolerance)
}

fn cylinder_planar_surface_intersection_events(
    cylinder_surface: &NurbsSurface,
    frame: crate::Frame3,
    radius: Real,
    height: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    cylinder_first: bool,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let axis = frame.z_axis().as_vector();
    let normal = plane.normal().as_vector();
    let parallel = axis.cross(normal)?.length()? <= tolerance.angular();
    let axial_dot = axis.dot(normal)?;
    if parallel {
        let axial_position = -plane.signed_distance_to(frame.origin())? / axial_dot;
        if axial_position < -tolerance.absolute() || axial_position > height + tolerance.absolute()
        {
            return Ok(Vec::new());
        }
        let center = frame.point_at([0.0, 0.0, axial_position.clamp(0.0, height)])?;
        let section_normal = if cylinder_first {
            plane.normal().opposite()
        } else {
            plane.normal()
        };
        let circle = Circle3::try_new(center, radius, section_normal, tolerance)?.to_nurbs()?;
        let mut events = intersect_curve_with_planar_surface(&circle, planar_surface, tolerance)?;
        let positive_domain = section_normal.as_vector().dot(frame.z_axis().as_vector())? > 0.0;
        let domain = if positive_domain {
            0.0..=std::f64::consts::TAU
        } else {
            -std::f64::consts::TAU..=0.0
        };
        for event in &mut events {
            if let SurfaceSurfaceIntersectionEvent::Curve(section) = event
                && section.is_closed()?
            {
                *section = section.try_reparameterized(domain.clone())?;
            }
        }
        return Ok(events);
    }
    if axial_dot.abs() > tolerance.angular() {
        let circle =
            Circle3::try_new(frame.origin(), radius, frame.z_axis(), tolerance)?.to_nurbs()?;
        let controls = circle
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    control.point().translated(
                        axis.scaled(-plane.signed_distance_to(control.point())? / axial_dot)?,
                    )?,
                    control.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let ellipse =
            NurbsCurve::try_new_rational(circle.degree(), controls, circle.knots().to_vec())?;
        let inside_rims = ellipse
            .control_points()
            .iter()
            .try_fold(true, |inside, control| {
                let height_at_control = frame.origin().vector_to(control.point())?.dot(axis)?;
                Ok::<bool, GeometryError>(
                    inside && height_at_control >= 0.0 && height_at_control <= height,
                )
            })?;
        if inside_rims {
            return intersect_curve_with_planar_surface(&ellipse, planar_surface, tolerance);
        }
        let mut result = Vec::new();
        for event in curve_surface_intersection_events(&ellipse, cylinder_surface, tolerance)? {
            if let CurveSurfaceIntersectionEvent::Overlap(overlap) = event {
                result.extend(intersect_curve_with_planar_surface(
                    &ellipse.try_trimmed(overlap.curve_interval())?,
                    planar_surface,
                    tolerance,
                )?);
            }
        }
        return Ok(result);
    }
    let signed_distance = plane.signed_distance_to(frame.origin())?;
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * radius.max(height));
    if signed_distance.abs() > radius + distance_tolerance {
        return Ok(Vec::new());
    }
    let base = frame
        .origin()
        .translated(normal.scaled(-signed_distance)?)?;
    let transverse = axis.cross(normal)?.normalized_nonzero()?.as_vector();
    let squared_offset = (radius - signed_distance.abs()) * (radius + signed_distance.abs());
    let offset = squared_offset.max(0.0).sqrt();
    let signs: &[Real] = if offset <= distance_tolerance {
        // Rhino reports both coincident branches at a tangent cylinder plane.
        &[0.0, 0.0]
    } else {
        &[-1.0, 1.0]
    };
    let mut result = Vec::new();
    for sign in signs {
        let line_origin = base.translated(transverse.scaled(sign * offset)?)?;
        let line = unit_speed_line(line_origin, frame.z_axis(), 0.0, height)?;
        result.extend(intersect_curve_with_planar_surface(
            &line,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(result)
}

fn intersect_curve_with_planar_surface(
    curve: &NurbsCurve,
    planar_surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    curve_surface_intersection_events(curve, planar_surface, tolerance)?
        .into_iter()
        .map(|event| match event {
            CurveSurfaceIntersectionEvent::Point(point) => {
                Ok(SurfaceSurfaceIntersectionEvent::Point(point.point()))
            }
            CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                Ok(SurfaceSurfaceIntersectionEvent::Curve(
                    curve.try_trimmed(overlap.curve_interval())?,
                ))
            }
        })
        .collect()
}

/// Intersects a finite NURBS surface with the trimmed faces of a B-rep.
///
/// Single full-domain curved faces use the exact surface/surface path. The
/// multi-face path handles planar surfaces against planar and curved B-rep
/// faces. Face-level curves are clipped against exact trim regions when needed,
/// deduplicated across shared topology, and
/// joined into maximal connected components. Coincident planar faces contribute
/// the parts of both region boundaries that lie in the other region.
pub fn surface_brep_intersection_events(
    surface: &NurbsSurface,
    brep: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceBrepIntersectionEvent>, GeometryError> {
    if let [face] = brep.faces()
        && crate::brep::face_covers_full_surface_domain(face, tolerance)?
        && (surface.plane(tolerance)?.is_none() || face.surface().plane(tolerance)?.is_none())
    {
        return Ok(
            surface_surface_intersection_events(surface, face.surface(), tolerance)?
                .into_iter()
                .map(|event| match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => {
                        SurfaceBrepIntersectionEvent::Point(point)
                    }
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                        SurfaceBrepIntersectionEvent::Curve(curve)
                    }
                })
                .collect(),
        );
    }
    let surface_plane = surface.plane(tolerance)?;
    let distance_tolerance = surface_brep_distance_tolerance(surface, brep, tolerance);
    let mut points = Vec::new();
    let mut curves = Vec::new();

    for face in brep.faces() {
        let face_plane = face.surface().plane(tolerance)?;
        let full_domain = crate::brep::face_covers_full_surface_domain(face, tolerance)?;
        let coincident =
            if let (Some(surface_plane), Some(face_plane)) = (surface_plane, face_plane) {
                planes_are_coincident(surface_plane, face_plane, tolerance, distance_tolerance)?
            } else {
                false
            };
        if coincident && !full_domain {
            let (face_points, face_curves) = coincident_planar_surface_brep_face_boundary(
                surface,
                brep,
                face,
                tolerance,
                distance_tolerance,
            )?;
            for point in face_points {
                push_unique_brep_point(&mut points, point, distance_tolerance);
            }
            curves.extend(face_curves);
            continue;
        }
        let face_events = surface_surface_intersection_events(surface, face.surface(), tolerance)?;

        for event in face_events {
            match event {
                SurfaceSurfaceIntersectionEvent::Point(point) => {
                    if full_domain
                        || point_on_brep_face(point, face, tolerance, distance_tolerance)?
                    {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                }
                SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                    if coincident || (face_plane.is_none() && full_domain) {
                        // The face covers its natural domain, already used by
                        // the surface pair intersection. Clipping the same
                        // perimeter again can shift endpoint parameters by
                        // a few ulps on elevated edge curves.
                        curves.push(brep_cylinder_transverse_section_domain(
                            curve, brep, face, tolerance,
                        )?);
                        continue;
                    }
                    let (face_points, face_curves) =
                        clip_curve_to_brep_face(&curve, brep, face, tolerance, distance_tolerance)?;
                    for point in face_points {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                    for curve in face_curves {
                        curves.push(brep_cylinder_transverse_section_domain(
                            curve, brep, face, tolerance,
                        )?);
                    }
                }
            }
        }
    }

    let (curves, points) =
        finalize_brep_intersection_geometry(points, curves, tolerance, distance_tolerance)?;

    Ok(curves
        .into_iter()
        .map(SurfaceBrepIntersectionEvent::Curve)
        .chain(points.into_iter().map(SurfaceBrepIntersectionEvent::Point))
        .collect())
}

/// The boundary of the common region of a planar surface patch and a
/// coincident trimmed planar face. Both sets of boundary curves are needed:
/// either region may cut through the interior of the other.
fn coincident_planar_surface_brep_face_boundary(
    surface: &NurbsSurface,
    brep: &Brep,
    face: &BrepFace,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<(Vec<Point3>, Vec<NurbsCurve>), GeometryError> {
    let mut points = Vec::new();
    let mut curves = Vec::new();
    let mut seen_edges = vec![false; brep.edges().len()];
    for face_loop in face.loops() {
        for trim in face_loop.trims() {
            let Some(edge_index) = trim.edge() else {
                continue;
            };
            if std::mem::replace(&mut seen_edges[edge_index], true) {
                continue;
            }
            for event in intersect_curve_with_planar_surface(
                brep.edges()[edge_index].curve(),
                surface,
                tolerance,
            )? {
                match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => curves.push(curve),
                }
            }
        }
    }
    let u = surface.domain_u();
    let v = surface.domain_v();
    for boundary in [
        surface.isocurve_u(*v.start())?,
        surface.isocurve_v(*u.end())?,
        surface.isocurve_u(*v.end())?,
        surface.isocurve_v(*u.start())?,
    ] {
        let (boundary_points, boundary_curves) =
            clip_curve_to_brep_face(&boundary, brep, face, tolerance, distance_tolerance)?;
        for point in boundary_points {
            push_unique_brep_point(&mut points, point, distance_tolerance);
        }
        curves.extend(boundary_curves);
    }
    Ok((points, curves))
}

/// Intersects the trimmed faces of two B-reps.
///
/// Single full-domain curved faces use the exact surface/surface path. The
/// multi-face path handles planar and curved faces, including exact trim loops.
/// Face-pair results are clipped against trim regions when needed;
/// then shared-topology duplicates are removed and connected pieces are joined
/// into maximal components. Coincident planar face pairs contribute their
/// shared region boundaries. Multiple planar area-overlap pairs join their
/// unique linear edges into traversable paths.
pub fn brep_brep_intersection_events(
    first: &Brep,
    second: &Brep,
    tolerance: Tolerance,
) -> Result<Vec<BrepBrepIntersectionEvent>, GeometryError> {
    if let ([first_face], [second_face]) = (first.faces(), second.faces())
        && crate::brep::face_covers_full_surface_domain(first_face, tolerance)?
        && crate::brep::face_covers_full_surface_domain(second_face, tolerance)?
        && (first_face.surface().plane(tolerance)?.is_none()
            || second_face.surface().plane(tolerance)?.is_none())
    {
        return Ok(surface_surface_intersection_events(
            first_face.surface(),
            second_face.surface(),
            tolerance,
        )?
        .into_iter()
        .map(|event| match event {
            SurfaceSurfaceIntersectionEvent::Point(point) => {
                BrepBrepIntersectionEvent::Point(point)
            }
            SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                BrepBrepIntersectionEvent::Curve(curve)
            }
        })
        .collect());
    }
    let distance_tolerance = brep_brep_distance_tolerance(first, second, tolerance);
    let mut points = Vec::new();
    let mut curves = Vec::new();
    let mut coincident_area_pairs = 0_usize;

    for first_face in first.faces() {
        let first_plane = first_face.surface().plane(tolerance)?;
        let first_full = crate::brep::face_covers_full_surface_domain(first_face, tolerance)?;
        for second_face in second.faces() {
            let second_plane = second_face.surface().plane(tolerance)?;
            let second_full = crate::brep::face_covers_full_surface_domain(second_face, tolerance)?;
            let coincident =
                if let (Some(first_plane), Some(second_plane)) = (first_plane, second_plane) {
                    planes_are_coincident(first_plane, second_plane, tolerance, distance_tolerance)?
                } else {
                    false
                };
            if coincident && (!first_full || !second_full) {
                let (pair_points, pair_curves) = coincident_planar_brep_faces_boundary(
                    first,
                    first_face,
                    second,
                    second_face,
                    tolerance,
                    distance_tolerance,
                )?;
                let (pair_curves, pair_points) = finalize_brep_intersection_geometry(
                    pair_points,
                    pair_curves,
                    tolerance,
                    distance_tolerance,
                )?;
                let mut has_area_overlap = false;
                for curve in &pair_curves {
                    if curve.is_closed()? {
                        has_area_overlap = true;
                        break;
                    }
                }
                if has_area_overlap {
                    coincident_area_pairs += 1;
                }
                for point in pair_points {
                    push_unique_brep_point(&mut points, point, distance_tolerance);
                }
                curves.extend(pair_curves);
                continue;
            }
            let face_events = surface_surface_intersection_events(
                first_face.surface(),
                second_face.surface(),
                tolerance,
            )?;
            if coincident {
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
                }
            }

            for event in face_events {
                match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => {
                        if (first_full
                            || point_on_brep_face(
                                point,
                                first_face,
                                tolerance,
                                distance_tolerance,
                            )?)
                            && (second_full
                                || point_on_brep_face(
                                    point,
                                    second_face,
                                    tolerance,
                                    distance_tolerance,
                                )?)
                        {
                            push_unique_brep_point(&mut points, point, distance_tolerance);
                        }
                    }
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                        if coincident {
                            curves.push(curve);
                            continue;
                        }
                        let (second_points, second_curves) = if second_full {
                            (Vec::new(), vec![curve])
                        } else {
                            clip_curve_to_brep_face(
                                &curve,
                                second,
                                second_face,
                                tolerance,
                                distance_tolerance,
                            )?
                        };
                        for point in second_points {
                            if point_on_brep_face(point, first_face, tolerance, distance_tolerance)?
                            {
                                push_unique_brep_point(&mut points, point, distance_tolerance);
                            }
                        }
                        for second_curve in second_curves {
                            let (first_points, first_curves) = if first_full {
                                (Vec::new(), vec![second_curve])
                            } else {
                                clip_curve_to_brep_face(
                                    &second_curve,
                                    first,
                                    first_face,
                                    tolerance,
                                    distance_tolerance,
                                )?
                            };
                            for point in first_points {
                                push_unique_brep_point(&mut points, point, distance_tolerance);
                            }
                            for curve in first_curves {
                                let curve = brep_cylinder_transverse_section_domain(
                                    curve, first, first_face, tolerance,
                                )?;
                                curves.push(brep_cylinder_transverse_section_domain(
                                    curve,
                                    second,
                                    second_face,
                                    tolerance,
                                )?);
                            }
                        }
                    }
                }
            }
        }
    }

    let (curves, points) = if coincident_area_pairs > 1 {
        finalize_multi_coincident_brep_geometry(points, curves, tolerance, distance_tolerance)?
    } else {
        finalize_brep_intersection_geometry(points, curves, tolerance, distance_tolerance)?
    };
    Ok(curves
        .into_iter()
        .map(BrepBrepIntersectionEvent::Curve)
        .chain(points.into_iter().map(BrepBrepIntersectionEvent::Point))
        .collect())
}

/// Several coincident planar faces may share boundary edges. Keep each linear
/// edge once, then cover the resulting graph with joined curve paths. Junctions
/// can have more than two incident edges, so the ordinary unambiguous polyline
/// join used for a single face region is not applicable here.
fn finalize_multi_coincident_brep_geometry(
    points: Vec<Point3>,
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<(Vec<NurbsCurve>, Vec<Point3>), GeometryError> {
    let mut segments = Vec::new();
    for curve in curves {
        if curve.degree() != 1 || curve.is_rational() {
            return Err(GeometryError::UnsupportedBrepBrepIntersection {
                context: "multiple coincident curved face regions",
            });
        }
        for segment in linear_curve_segments(&curve, tolerance, distance_tolerance)? {
            if !segments
                .iter()
                .any(|existing| linear_segments_match(existing, &segment, distance_tolerance))
            {
                segments.push(segment);
            }
        }
    }
    let mut curves =
        coincident_brep_graph::cover_unique_segments(&segments, tolerance, distance_tolerance)?;
    let mut isolated_points = Vec::new();
    for point in points {
        let on_curve = curves.iter().try_fold(false, |found, curve| {
            if found {
                return Ok::<bool, GeometryError>(true);
            }
            let parameter = curve.closest_parameter(point, tolerance)?;
            Ok(curve.evaluate(parameter)?.distance_to(point)? <= distance_tolerance * 2.0)
        })?;
        if !on_curve {
            isolated_points.push(point);
        }
    }
    isolated_points.sort_by(|left, right| compare_points(*left, *right));
    curves.sort_by(|left, right| {
        compare_points(
            left.control_points()[0].point(),
            right.control_points()[0].point(),
        )
    });
    Ok((curves, isolated_points))
}

fn coincident_planar_brep_faces_boundary(
    first: &Brep,
    first_face: &BrepFace,
    second: &Brep,
    second_face: &BrepFace,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<(Vec<Point3>, Vec<NurbsCurve>), GeometryError> {
    let mut points = Vec::new();
    let mut curves = Vec::new();
    for (source_brep, source_face, target_brep, target_face) in [
        (first, first_face, second, second_face),
        (second, second_face, first, first_face),
    ] {
        let mut seen_edges = vec![false; source_brep.edges().len()];
        for face_loop in source_face.loops() {
            for trim in face_loop.trims() {
                let Some(edge_index) = trim.edge() else {
                    continue;
                };
                if std::mem::replace(&mut seen_edges[edge_index], true) {
                    continue;
                }
                let (edge_points, edge_curves) = clip_curve_to_brep_face(
                    source_brep.edges()[edge_index].curve(),
                    target_brep,
                    target_face,
                    tolerance,
                    distance_tolerance,
                )?;
                for point in edge_points {
                    push_unique_brep_point(&mut points, point, distance_tolerance);
                }
                curves.extend(edge_curves);
            }
        }
    }
    Ok((points, curves))
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

/// Rhino's Intersect command doubles the angular parameter interval of a
/// transverse circular section from a capped or trimmed cylinder wall.
/// An untrimmed standalone cylinder face retains the original interval.
fn brep_cylinder_transverse_section_domain(
    curve: NurbsCurve,
    brep: &Brep,
    face: &BrepFace,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    if curve.degree() != 2 || !curve.is_rational() {
        return Ok(curve);
    }
    let Some((frame, _, _)) = face.surface().canonical_cylinder(tolerance)? else {
        return Ok(curve);
    };
    let axis = frame.z_axis().as_vector();
    let axial_positions = curve
        .control_points()
        .iter()
        .map(|control| frame.origin().vector_to(control.point())?.dot(axis))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let first_position = axial_positions[0];
    if axial_positions
        .iter()
        .any(|&position| (position - first_position).abs() > tolerance.absolute() * 2.0)
    {
        return Ok(curve);
    }
    if face.is_untrimmed(tolerance)? {
        if brep.faces().len() != 3 {
            return Ok(curve);
        }
        for other in brep.faces() {
            if !std::ptr::eq(other, face) && other.surface().plane(tolerance)?.is_none() {
                return Ok(curve);
            }
        }
    }
    let domain = curve.domain();
    let start = *domain.start();
    let span = *domain.end() - start;
    if span <= 0.0 || span > std::f64::consts::TAU + tolerance.angular() * 10.0 {
        return Ok(curve);
    }
    curve.try_reparameterized(2.0 * start..=2.0 * *domain.end())
}

fn finalize_brep_intersection_geometry(
    points: Vec<Point3>,
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<(Vec<NurbsCurve>, Vec<Point3>), GeometryError> {
    let mut linear_curves = Vec::new();
    let mut closed_curved = Vec::new();
    let mut open_curved = Vec::new();
    for curve in curves {
        if curve.degree() == 1 && !curve.is_rational() {
            linear_curves.push(curve);
        } else if curve.is_closed()? {
            if !closed_curved.contains(&curve) {
                closed_curved.push(curve);
            }
        } else {
            if !open_curved.contains(&curve) && !open_curved.contains(&curve.reversed()?) {
                open_curved.push(curve);
            }
        }
    }
    let linear_curves = join_brep_linear_curves(linear_curves, tolerance, distance_tolerance)?;
    let mut curves = Vec::new();
    if open_curved.is_empty() {
        curves.extend(linear_curves);
    } else {
        let mut open = Vec::new();
        for curve in linear_curves {
            if curve.is_closed()? {
                curves.push(curve);
            } else {
                open.push(Curve3::NurbsCurve(curve));
            }
        }
        open.extend(open_curved.into_iter().map(Curve3::NurbsCurve));
        curves.extend(
            join_curves(
                &open,
                CurveJoinOptions {
                    tolerance: distance_tolerance,
                    preserve_direction: false,
                    style: CurveJoinStyle::Batch,
                },
                tolerance,
            )?
            .into_iter()
            .map(|component| component.curve().as_ref().to_nurbs())
            .collect::<Result<Vec<_>, _>>()?,
        );
    }
    curves.extend(closed_curved);
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
) -> Result<Vec<NurbsCurve>, GeometryError> {
    debug_assert!(
        curves
            .iter()
            .all(|curve| curve.degree() == 1 && !curve.is_rational())
    );

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
        context: "coincident planar surfaces outside certified convex or monotone strip patches",
    };
    let mut first_polygon = certified_coincident_patch_polygon(first, tolerance)?;
    let mut second_polygon = certified_coincident_patch_polygon(second, tolerance)?;
    if first_polygon.is_none() || second_polygon.is_none() {
        let first_certified = if let Some(polygon) = &mut first_polygon {
            orient_and_validate_convex_polygon(polygon, plane.normal(), distance_tolerance)?
        } else {
            certified_monotone_planar_strip(first, plane, distance_tolerance)?
        };
        let second_certified = if let Some(polygon) = &mut second_polygon {
            orient_and_validate_convex_polygon(polygon, plane.normal(), distance_tolerance)?
        } else {
            certified_monotone_planar_strip(second, plane, distance_tolerance)?
        };
        if first_certified && second_certified {
            return coincident_planar_boundary_events(
                first,
                second,
                first_polygon.is_none(),
                tolerance,
                distance_tolerance,
            );
        }
        return Err(unsupported());
    }
    let mut first_polygon = first_polygon.ok_or_else(unsupported)?;
    let mut second_polygon = second_polygon.ok_or_else(unsupported)?;
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

/// Certifies a planar strip with a strictly monotone U coordinate and
/// separated V rows. Degree elevation may leave weights a few ulps from one;
/// those are accepted only when all weights remain equal to working precision.
/// Equal U coordinates in both rows make fixed-U rulings vertical in this
/// local frame. The B-spline convex-hull property then proves a one-to-one
/// parameterization of the region between the two edges.
fn certified_monotone_planar_strip(
    surface: &NurbsSurface,
    plane: Plane,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    if surface.degree_v() != 1 || surface.control_point_count_v() != 2 {
        return Ok(false);
    }
    let count_u = surface.control_point_count_u();
    let controls = surface.control_points();
    let reference_weight = controls[0].weight();
    if reference_weight == 0.0
        || controls.iter().any(|control| {
            let ratio = control.weight() / reference_weight;
            !ratio.is_finite() || (ratio - 1.0).abs() > Real::EPSILON * 64.0
        })
    {
        return Ok(false);
    }
    let origin = controls[0].point();
    let Ok(axis) = origin
        .vector_to(controls[count_u - 1].point())?
        .normalized_nonzero()
    else {
        return Ok(false);
    };
    let Ok(transverse) = plane
        .normal()
        .as_vector()
        .cross(axis.as_vector())?
        .normalized_nonzero()
    else {
        return Ok(false);
    };
    let mut previous_x = None;
    let mut gap_sign = 0_i8;
    for index in 0..count_u {
        let lower = origin.vector_to(controls[index].point())?;
        let upper = origin.vector_to(controls[count_u + index].point())?;
        let lower_x = lower.dot(axis.as_vector())?;
        let upper_x = upper.dot(axis.as_vector())?;
        let coordinate_scale = lower_x.abs().max(upper_x.abs()).max(1.0);
        if (lower_x - upper_x).abs() > Real::EPSILON * coordinate_scale * 64.0 {
            return Ok(false);
        }
        if previous_x.is_some_and(|previous| {
            lower_x
                <= previous + distance_tolerance * 2.0 + Real::EPSILON * coordinate_scale * 128.0
        }) {
            return Ok(false);
        }
        previous_x = Some(lower_x);
        let gap = controls[index]
            .point()
            .vector_to(controls[count_u + index].point())?
            .dot(transverse.as_vector())?;
        if gap.abs() <= distance_tolerance * 2.0 {
            return Ok(false);
        }
        let sign = if gap > 0.0 { 1 } else { -1 };
        if gap_sign != 0 && gap_sign != sign {
            return Ok(false);
        }
        gap_sign = sign;
    }
    Ok(true)
}

fn coincident_planar_boundary_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    first_is_strip: bool,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let mut points = Vec::new();
    let mut curves = Vec::new();
    let pairs = if first_is_strip {
        [(first, second), (second, first)]
    } else {
        [(second, first), (first, second)]
    };
    for (source, target) in pairs {
        for edge in source.natural_edge_curves()? {
            for event in intersect_curve_with_planar_surface(&edge, target, tolerance)? {
                match event {
                    SurfaceSurfaceIntersectionEvent::Point(point) => {
                        push_unique_brep_point(&mut points, point, distance_tolerance);
                    }
                    SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                        let duplicate = curves.iter().try_fold(false, |found, existing| {
                            if found {
                                Ok::<bool, GeometryError>(true)
                            } else {
                                coincident_boundary_curves_match(
                                    existing,
                                    &curve,
                                    distance_tolerance,
                                )
                            }
                        })?;
                        if !duplicate {
                            curves.push(curve);
                        }
                    }
                }
            }
        }
    }
    let (mut curves, _points) =
        finalize_brep_intersection_geometry(points, curves, tolerance, distance_tolerance)?;
    if let [curve] = curves.as_mut_slice()
        && curve.is_closed()?
        && let Some(oriented) = orient_coincident_strip_boundary(
            curve,
            if first_is_strip { first } else { second },
            if first_is_strip { second } else { first },
            tolerance,
            distance_tolerance,
        )?
    {
        *curve = oriented;
    }
    Ok(curves
        .into_iter()
        .map(SurfaceSurfaceIntersectionEvent::Curve)
        .collect())
}

fn orient_coincident_strip_boundary(
    boundary: &NurbsCurve,
    strip: &NurbsSurface,
    other: &NurbsSurface,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let Some(bottom) = strip.natural_edge_curves()?.into_iter().next() else {
        return Ok(None);
    };
    let Some(SurfaceSurfaceIntersectionEvent::Curve(bottom_piece)) =
        intersect_curve_with_planar_surface(&bottom, other, tolerance)?
            .into_iter()
            .find(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Curve(_)))
    else {
        return Ok(None);
    };
    let seam = bottom_piece.evaluate(*bottom_piece.domain().start())?;
    let mut domain_start = *bottom_piece.domain().start();
    for edge in other.natural_edge_curves()? {
        let parameter = edge.closest_parameter(seam, tolerance)?;
        if edge.evaluate(parameter)?.distance_to(seam)? <= distance_tolerance * 2.0 {
            domain_start = parameter;
            break;
        }
    }
    let seam_parameter = boundary.closest_parameter(seam, tolerance)?;
    if boundary.evaluate(seam_parameter)?.distance_to(seam)? > distance_tolerance * 2.0 {
        return Ok(None);
    }
    let span = *boundary.domain().end() - *boundary.domain().start();
    let end = domain_start + span;
    crate::require_finite([domain_start, end], "coincident strip boundary domain")?;
    Ok(Some(
        boundary
            .try_change_closed_seam(seam_parameter)?
            .try_reparameterized(domain_start..=end)?,
    ))
}

fn coincident_boundary_curves_match(
    first: &NurbsCurve,
    second: &NurbsCurve,
    distance_tolerance: Real,
) -> Result<bool, GeometryError> {
    if first == second || *first == second.reversed()? {
        return Ok(true);
    }
    if !first.is_linear_at_zero_tolerance()? || !second.is_linear_at_zero_tolerance()? {
        return Ok(false);
    }
    let endpoints = |curve: &NurbsCurve| -> Result<[Point3; 2], GeometryError> {
        let domain = curve.domain();
        Ok([
            curve.evaluate(*domain.start())?,
            curve.evaluate(*domain.end())?,
        ])
    };
    Ok(point_pairs_match(
        endpoints(first)?,
        endpoints(second)?,
        distance_tolerance,
    ))
}

fn is_four_sided_bilinear_patch(surface: &NurbsSurface) -> bool {
    surface.degree_u() == 1
        && surface.degree_v() == 1
        && surface.control_point_count_u() == 2
        && surface.control_point_count_v() == 2
}

fn certified_coincident_patch_polygon(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Option<Vec<Point3>>, GeometryError> {
    if is_four_sided_bilinear_patch(surface)
        && weights_have_common_sign(surface.control_points().iter().map(|point| point.weight()))
    {
        // At fixed U, a rational bilinear patch with weights of one sign
        // traces a straight segment between two monotonically traversed
        // opposite edges. For a convex corner quad those segments partition
        // the quad, so its corner polygon is the complete surface image.
        return Ok(Some(bilinear_patch_polygon(surface)));
    }
    match surface.try_affine_patch_corners(tolerance) {
        Ok(corners) => return Ok(Some(corners.to_vec())),
        Err(GeometryError::InvalidControlNet { .. } | GeometryError::Degenerate { .. }) => {}
        Err(error) => return Err(error),
    }
    match surface.try_projective_patch_corners(tolerance) {
        Ok(corners) => Ok(Some(corners.to_vec())),
        Err(GeometryError::InvalidControlNet { .. } | GeometryError::Degenerate { .. }) => Ok(None),
        Err(error) => Err(error),
    }
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
    first.midpoint(second)
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
            let edge_parameters = if let Some(contacts) = certifiable_axis_planar_edge_contacts(
                curve,
                edge_curve,
                tolerance,
                distance_tolerance,
            )? {
                contacts
            } else {
                curve
                    .intersections_with_curve(edge_curve, tolerance)?
                    .into_iter()
                    .map(|intersection| intersection.first_parameter())
                    .collect()
            };
            for edge_parameter in edge_parameters {
                let parameter = snap_parameter_to_interval(edge_parameter, start, end);
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

/// A positive-weight trim whose controls are strictly on one side of an
/// axis-aligned section plane can only meet that plane at clamped endpoints.
/// Certifying those contacts avoids the general curve/curve subdivision at
/// grazing seams, where both control hulls touch at one parameter endpoint.
fn certifiable_axis_planar_edge_contacts(
    curve: &NurbsCurve,
    edge: &NurbsCurve,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Option<Vec<Real>>, GeometryError> {
    if curve
        .control_points()
        .iter()
        .any(|point| point.weight() <= 0.0)
        || edge
            .control_points()
            .iter()
            .any(|point| point.weight() <= 0.0)
    {
        return Ok(None);
    }
    let coordinates = |point: Point3, axis: usize| point.to_array()[axis];
    let Some((axis, plane_coordinate)) = (0..3).find_map(|axis| {
        let first = coordinates(curve.control_points()[0].point(), axis);
        curve
            .control_points()
            .iter()
            .all(|point| coordinates(point.point(), axis) == first)
            .then_some((axis, first))
    }) else {
        return Ok(None);
    };
    let distances = edge
        .control_points()
        .iter()
        .map(|point| coordinates(point.point(), axis) - plane_coordinate)
        .collect::<Vec<_>>();
    let safe = distance_tolerance * 2.0;
    let nonzero = distances
        .iter()
        .copied()
        .filter(|distance| distance.abs() > safe)
        .collect::<Vec<_>>();
    if nonzero.is_empty() {
        return Ok(None);
    }
    let sign = nonzero[0].signum();
    if distances
        .iter()
        .any(|distance| *distance != 0.0 && distance.signum() != sign)
        || distances
            .iter()
            .any(|distance| distance.abs() > distance_tolerance && distance.abs() <= safe)
    {
        return Ok(None);
    }
    let near = distances
        .iter()
        .map(|distance| distance.abs() <= distance_tolerance)
        .collect::<Vec<_>>();
    let prefix = near.iter().take_while(|&&value| value).count();
    let suffix = near.iter().rev().take_while(|&&value| value).count();
    if prefix > edge.degree()
        || suffix > edge.degree()
        || near.iter().filter(|&&value| value).count() != prefix + suffix
    {
        return Ok(None);
    }
    let mut contacts = Vec::new();
    for endpoint in [
        (prefix > 0).then_some(*edge.domain().start()),
        (suffix > 0).then_some(*edge.domain().end()),
    ]
    .into_iter()
    .flatten()
    {
        let point = edge.evaluate(endpoint)?;
        let parameter = curve.closest_parameter(point, tolerance)?;
        if curve.evaluate(parameter)?.distance_to(point)? <= safe {
            contacts.push(parameter);
        }
    }
    Ok(Some(contacts))
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
    left.midpoint(right)
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

    #[test]
    fn scalar_midpoints_preserve_subnormal_rounding() {
        let unit = Real::from_bits(1);
        for (left, right, expected) in [(1.0, 2.0, 2.0), (-1.0, 2.0, 0.0), (-31.0, -30.0, -30.0)] {
            assert_eq!(finite_midpoint(left * unit, right * unit), expected * unit);
            assert_eq!(finite_midpoint(right * unit, left * unit), expected * unit);
        }
        assert_eq!(finite_midpoint(Real::MAX, Real::MAX), Real::MAX);
        assert_eq!(finite_midpoint(-Real::MAX, Real::MAX), 0.0);
    }

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

    #[test]
    fn nonplanar_bilinear_plane_intersection_preserves_two_exact_conics() {
        let saddle = NurbsSurface::try_bilinear([
            point(-1.0, -1.0, 1.0),
            point(1.0, -1.0, -1.0),
            point(1.0, 1.0, 1.0),
            point(-1.0, 1.0, -1.0),
        ])
        .unwrap();
        let plane = horizontal_rectangle(-2.0, 2.0, -2.0, 2.0, 0.25);
        let events =
            surface_surface_intersection_events(&saddle, &plane, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2, "{events:#?}");
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("expected a conic branch")
            };
            assert_eq!(curve.degree(), 2);
            for step in 0..=20 {
                let parameter = (*curve.domain().start() * (20 - step) as Real
                    + *curve.domain().end() * step as Real)
                    / 20.0;
                let point = curve.evaluate(parameter).unwrap();
                assert!((point.z() - 0.25).abs() < 1e-12, "{point:?}");
                assert!((point.x() * point.y() - 0.25).abs() < 1e-12, "{point:?}");
                assert!(point.x().abs() <= 1.0 + 1e-12 && point.y().abs() <= 1.0 + 1e-12);
            }
        }
        let quadrant = horizontal_rectangle(0.0, 2.0, 0.0, 2.0, 0.25);
        let clipped =
            surface_surface_intersection_events(&quadrant, &saddle, Tolerance::DEFAULT).unwrap();
        assert_eq!(clipped.len(), 1, "{clipped:#?}");
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &clipped[0] else {
            panic!("expected one clipped conic")
        };
        for step in 0..=20 {
            let parameter = (*curve.domain().start() * (20 - step) as Real
                + *curve.domain().end() * step as Real)
                / 20.0;
            let point = curve.evaluate(parameter).unwrap();
            assert!(point.x() >= -1e-12 && point.y() >= -1e-12);
            assert!((point.x() * point.y() - 0.25).abs() < 1e-12);
        }
        let plane = horizontal_rectangle(-2.0, 2.0, -2.0, 2.0, 0.25);
        let [_, north] = Brep::try_split_rectangular_surface_face_v(
            plane,
            -2.0..=2.0,
            -2.0..=2.0,
            0.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let trimmed =
            surface_brep_intersection_events(&saddle, &north, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(arc)] = trimmed.as_slice() else {
            panic!("expected one conic branch on the trimmed plane: {trimmed:#?}")
        };
        assert_eq!(arc.degree(), 2);
        let midpoint = (*arc.domain().start() + *arc.domain().end()) * 0.5;
        let point = arc.evaluate(midpoint).unwrap();
        assert!(point.x() > 0.0 && point.y() > 0.0);
        assert!((point.x() * point.y() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn rational_bilinear_plane_intersection_and_corner_contacts() {
        let corners = [
            point(-1.0, -1.0, 1.0),
            point(1.0, -1.0, -1.0),
            point(-1.0, 1.0, -1.0),
            point(1.0, 1.0, 1.0),
        ];
        let weighted = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            corners
                .into_iter()
                .zip([1.0, 2.0, 3.0, 1.5])
                .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
                .collect(),
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let plane = horizontal_rectangle(-2.0, 2.0, -2.0, 2.0, 0.25);
        let events =
            surface_surface_intersection_events(&weighted, &plane, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2, "{events:#?}");
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("expected a rational conic")
            };
            assert_eq!(curve.degree(), 2);
            for step in 0..=8 {
                let parameter = (*curve.domain().start() * (8 - step) as Real
                    + *curve.domain().end() * step as Real)
                    / 8.0;
                let point = curve.evaluate(parameter).unwrap();
                let (u, v) = weighted
                    .closest_parameters(point, Tolerance::DEFAULT)
                    .unwrap();
                assert!(point.distance_to(weighted.evaluate(u, v).unwrap()).unwrap() < 1e-10);
                assert!((point.z() - 0.25).abs() < 1e-12);
            }
        }

        let tangent_plane = horizontal_rectangle(-2.0, 2.0, -2.0, 2.0, 1.0);
        let points =
            surface_surface_intersection_events(&weighted, &tangent_plane, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(points.len(), 2, "{points:#?}");
        assert!(
            points
                .iter()
                .all(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Point(_)))
        );
    }

    #[test]
    fn bilinear_saddle_plane_through_center_keeps_both_rulings() {
        let saddle = NurbsSurface::try_bilinear([
            point(-1.0, -1.0, 1.0),
            point(1.0, -1.0, -1.0),
            point(1.0, 1.0, 1.0),
            point(-1.0, 1.0, -1.0),
        ])
        .unwrap();
        let plane = horizontal_rectangle(-2.0, 2.0, -2.0, 2.0, 0.0);
        let events =
            surface_surface_intersection_events(&saddle, &plane, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2, "{events:#?}");
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("expected an exact ruling")
            };
            assert_eq!(curve.degree(), 1);
            let endpoints = [*curve.domain().start(), *curve.domain().end()]
                .map(|parameter| curve.evaluate(parameter).unwrap());
            assert!(endpoints.iter().all(|point| point.z().abs() < 1e-12));
            assert!(
                endpoints.iter().all(|point| point.x().abs() < 1e-12)
                    || endpoints.iter().all(|point| point.y().abs() < 1e-12)
            );
        }
        let tilted = NurbsSurface::try_bilinear([
            point(-2.0, -2.0, -2.0),
            point(2.0, -2.0, 2.0),
            point(2.0, 2.0, 2.0),
            point(-2.0, 2.0, -2.0),
        ])
        .unwrap();
        let tilted_events =
            surface_surface_intersection_events(&tilted, &saddle, Tolerance::DEFAULT).unwrap();
        assert_eq!(tilted_events.len(), 2, "{tilted_events:#?}");
        for event in tilted_events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("expected a center or boundary ruling")
            };
            let midpoint = (*curve.domain().start() + *curve.domain().end()) * 0.5;
            let point = curve.evaluate(midpoint).unwrap();
            assert!((point.z() - point.x()).abs() < 1e-12);
            assert!(point.x().abs() < 1e-12 || (point.y() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn full_curved_brep_face_uses_exact_surface_intersection_curves() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap();
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        let cylinder_brep = Brep::try_surface_face(cylinder.clone(), Tolerance::DEFAULT).unwrap();
        let plane_brep = Brep::try_surface_face(plane.clone(), Tolerance::DEFAULT).unwrap();
        let expected_surface_events =
            surface_surface_intersection_events(&plane, &cylinder, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(expected_surface)] =
            expected_surface_events.as_slice()
        else {
            panic!("expected one cylinder section");
        };
        assert_eq!(
            surface_brep_intersection_events(&plane, &cylinder_brep, Tolerance::DEFAULT).unwrap(),
            vec![SurfaceBrepIntersectionEvent::Curve(
                expected_surface.clone()
            )]
        );
        let expected_brep_events =
            surface_surface_intersection_events(&cylinder, &plane, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(expected_brep)] =
            expected_brep_events.as_slice()
        else {
            panic!("expected one cylinder section");
        };
        assert_eq!(
            brep_brep_intersection_events(&cylinder_brep, &plane_brep, Tolerance::DEFAULT).unwrap(),
            vec![BrepBrepIntersectionEvent::Curve(expected_brep.clone())]
        );
    }

    #[test]
    fn curved_surface_intersects_a_trimmed_planar_brep_face() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let sphere_frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 2.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(sphere_frame, 2.0).unwrap();
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        let cut = NurbsCurve::try_new(
            1,
            vec![point(-3.0, 0.0, 2.0), point(3.0, 0.0, 2.0)],
            vec![0.0, 0.0, 6.0, 6.0],
        )
        .unwrap();
        let [south, north] = Brep::try_split_rectangular_surface_face_west_east(
            plane,
            -3.0..=3.0,
            -3.0..=3.0,
            [0.0, 0.0],
            cut,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        for surface in [&cylinder, &sphere] {
            for face in [&south, &north] {
                let events =
                    surface_brep_intersection_events(surface, face, Tolerance::DEFAULT).unwrap();
                let [SurfaceBrepIntersectionEvent::Curve(arc)] = events.as_slice() else {
                    panic!("expected one circular arc: {events:#?}");
                };
                assert!(!arc.is_closed().unwrap());
                assert!(
                    (arc.length(Tolerance::DEFAULT).unwrap() - 2.0 * std::f64::consts::PI).abs()
                        < 1e-8
                );
                for endpoint in [*arc.domain().start(), *arc.domain().end()] {
                    let point = arc.evaluate(endpoint).unwrap();
                    assert!(point.y().abs() < 1e-9);
                    assert!((point.x().abs() - 2.0).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn cylinder_surface_intersects_a_trimmed_sphere_face() {
        let cylinder_frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(cylinder_frame, 2.0, 0.0, 4.0).unwrap();
        let sphere_frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 2.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(sphere_frame, 5.0_f64.sqrt()).unwrap();
        let u = sphere.domain_u();
        let half = Brep::try_rectangular_surface_face(
            sphere.clone(),
            *u.start()..=(*u.start() + *u.end()) * 0.5,
            sphere.domain_v(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let events =
            surface_brep_intersection_events(&cylinder, &half, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2, "{events:#?}");
        for event in events {
            let SurfaceBrepIntersectionEvent::Curve(arc) = event else {
                panic!("expected a circular arc")
            };
            assert!(!arc.is_closed().unwrap());
            assert!(
                (arc.length(Tolerance::DEFAULT).unwrap() - 2.0 * std::f64::consts::PI).abs() < 1e-8
            );
            let point = arc.evaluate(*arc.domain().start()).unwrap();
            assert!((point.x().hypot(point.y()) - 2.0).abs() < 1e-8);
            assert!(((point.z() - 2.0).abs() - 1.0).abs() < 1e-8);
        }
    }

    #[test]
    fn plane_sections_of_closed_cylinder_use_the_curved_wall_face() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = Brep::try_cylinder(frame, 2.0, 0.0, 4.0, Tolerance::DEFAULT).unwrap();
        assert!(
            crate::brep::face_covers_full_surface_domain(&cylinder.faces()[0], Tolerance::DEFAULT)
                .unwrap()
        );
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        let expected = surface_surface_intersection_events(
            &plane,
            cylinder.faces()[0].surface(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(section)] = expected.as_slice() else {
            panic!("expected one circle from the cylinder wall");
        };
        let section = section
            .try_reparameterized(0.0..=2.0 * std::f64::consts::TAU)
            .unwrap();
        assert_eq!(
            surface_brep_intersection_events(&plane, &cylinder, Tolerance::DEFAULT).unwrap(),
            vec![SurfaceBrepIntersectionEvent::Curve(section.clone())]
        );
        let plane_brep = Brep::try_surface_face(plane, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            brep_brep_intersection_events(&plane_brep, &cylinder, Tolerance::DEFAULT).unwrap(),
            vec![BrepBrepIntersectionEvent::Curve(section.clone())]
        );
    }

    #[test]
    fn steep_plane_section_of_closed_cylinder_joins_curved_wall_and_caps() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = Brep::try_cylinder(frame, 2.0, 0.0, 4.0, Tolerance::DEFAULT).unwrap();
        let plane = NurbsSurface::try_bilinear([
            point(-3.0, -3.0, -4.0),
            point(3.0, -3.0, 8.0),
            point(3.0, 3.0, 8.0),
            point(-3.0, 3.0, -4.0),
        ])
        .unwrap();
        let section =
            surface_brep_intersection_events(&plane, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(section.len(), 1, "{section:#?}");
        let SurfaceBrepIntersectionEvent::Curve(curve) = &section[0] else {
            panic!("expected one section curve")
        };
        assert!(curve.is_closed().unwrap());
        assert_eq!(curve.degree(), 2);
        for fraction in 0..=32 {
            let domain = curve.domain();
            let parameter = (*domain.start()
                + fraction as Real / 32.0 * (*domain.end() - *domain.start()))
            .min(*domain.end());
            let sample = curve.evaluate(parameter).unwrap();
            assert!((sample.z() - (2.0 * sample.x() + 2.0)).abs() < 1e-8);
            let radius = sample.x().hypot(sample.y());
            assert!(radius <= 2.0 + 1e-8);
            assert!(sample.z() >= -1e-8 && sample.z() <= 4.0 + 1e-8);
            if sample.z() > 1e-8 && sample.z() < 4.0 - 1e-8 {
                assert!((radius - 2.0).abs() < 1e-8);
            }
        }
        let plane_brep = Brep::try_surface_face(plane, Tolerance::DEFAULT).unwrap();
        let brep_section =
            brep_brep_intersection_events(&plane_brep, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(brep_section.len(), 1, "{brep_section:#?}");
    }

    #[test]
    fn oblique_closed_cylinder_section_keeps_ellipse_parameterization() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = Brep::try_cylinder(frame, 2.0, 0.0, 4.0, Tolerance::DEFAULT).unwrap();
        let plane = NurbsSurface::try_bilinear([
            point(-3.0, -3.0, 0.5),
            point(3.0, -3.0, 3.5),
            point(3.0, 3.0, 3.5),
            point(-3.0, 3.0, 0.5),
        ])
        .unwrap();
        let expected = surface_surface_intersection_events(
            &plane,
            cylinder.faces()[0].surface(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(expected)] = expected.as_slice() else {
            panic!("expected one ellipse")
        };
        assert_eq!(
            surface_brep_intersection_events(&plane, &cylinder, Tolerance::DEFAULT).unwrap(),
            vec![SurfaceBrepIntersectionEvent::Curve(expected.clone())]
        );
    }

    #[test]
    fn trimmed_cylinder_wall_face_restricts_plane_sections_to_its_height() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let [low, high] = Brep::try_split_rectangular_surface_face_v(
            wall.clone(),
            wall.domain_u(),
            wall.domain_v(),
            2.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(!low.faces()[0].is_untrimmed(Tolerance::DEFAULT).unwrap());
        assert!(!high.faces()[0].is_untrimmed(Tolerance::DEFAULT).unwrap());
        let lower_plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 1.0);
        let upper_plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 3.0);
        for (brep, hits, misses) in [
            (&low, &lower_plane, &upper_plane),
            (&high, &upper_plane, &lower_plane),
        ] {
            let events = surface_brep_intersection_events(hits, brep, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 1, "{events:#?}");
            let SurfaceBrepIntersectionEvent::Curve(circle) = &events[0] else {
                panic!("expected one circular section")
            };
            assert!(circle.is_closed().unwrap());
            assert_eq!(circle.domain(), 0.0..=2.0 * std::f64::consts::TAU);
            assert!(
                surface_brep_intersection_events(misses, brep, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
            let plane_brep = Brep::try_surface_face(hits.clone(), Tolerance::DEFAULT).unwrap();
            let brep_events =
                brep_brep_intersection_events(&plane_brep, brep, Tolerance::DEFAULT).unwrap();
            assert_eq!(brep_events.len(), 1, "{brep_events:#?}");
        }
    }

    #[test]
    fn angularly_trimmed_cylinder_wall_returns_open_section_arcs() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let [west, east] = Brep::try_split_rectangular_surface_face_u(
            wall.clone(),
            wall.domain_u(),
            wall.domain_v(),
            std::f64::consts::PI,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        for (brep, domain) in [
            (&west, 0.0..=std::f64::consts::TAU),
            (&east, std::f64::consts::TAU..=2.0 * std::f64::consts::TAU),
        ] {
            let events =
                surface_brep_intersection_events(&plane, brep, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 1, "{events:#?}");
            let SurfaceBrepIntersectionEvent::Curve(arc) = &events[0] else {
                panic!("expected one circular arc")
            };
            assert_eq!(arc.degree(), 2);
            assert!(!arc.is_closed().unwrap());
            assert_eq!(arc.domain(), domain);
            let length = arc.length(Tolerance::DEFAULT).unwrap();
            assert!(
                (length - 2.0 * std::f64::consts::PI).abs() < 1e-8,
                "{length}"
            );
            let plane_brep = Brep::try_surface_face(plane.clone(), Tolerance::DEFAULT).unwrap();
            let brep_events =
                brep_brep_intersection_events(&plane_brep, brep, Tolerance::DEFAULT).unwrap();
            assert_eq!(brep_events.len(), 1, "{brep_events:#?}");
        }
    }

    #[test]
    fn curved_trim_on_cylinder_wall_clips_a_transverse_section() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let oblique = NurbsSurface::try_bilinear([
            point(-3.0, -3.0, 0.5),
            point(3.0, -3.0, 3.5),
            point(3.0, 3.0, 3.5),
            point(-3.0, 3.0, 0.5),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&oblique, &wall, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(ellipse)] = events.as_slice() else {
            panic!("expected an exact tilted cylindrical section")
        };
        let cutter = ellipse.try_trimmed(0.0..=std::f64::consts::TAU).unwrap();
        let [south, north] = Brep::try_split_rectangular_surface_face_west_east(
            wall.clone(),
            0.0..=std::f64::consts::PI,
            wall.domain_v(),
            [3.0, 1.0],
            cutter,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        for brep in [&south, &north] {
            let results =
                surface_brep_intersection_events(&plane, brep, Tolerance::DEFAULT).unwrap();
            assert_eq!(results.len(), 1, "{results:#?}");
            let SurfaceBrepIntersectionEvent::Curve(arc) = &results[0] else {
                panic!("expected one circular arc")
            };
            assert!(!arc.is_closed().unwrap());
            assert!((arc.length(Tolerance::DEFAULT).unwrap() - std::f64::consts::PI).abs() < 1e-7);
            let plane_brep = Brep::try_surface_face(plane.clone(), Tolerance::DEFAULT).unwrap();
            assert_eq!(
                brep_brep_intersection_events(&plane_brep, brep, Tolerance::DEFAULT)
                    .unwrap()
                    .len(),
                1
            );
        }
        let endpoint_plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 3.0);
        let endpoint =
            surface_brep_intersection_events(&endpoint_plane, &south, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            endpoint,
            vec![SurfaceBrepIntersectionEvent::Point(point(2.0, 0.0, 3.0))]
        );
    }

    #[test]
    fn cylinder_plane_circle_domain_and_winding_follow_surface_order() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        for (first, second, domain, first_y) in [
            (&plane, &cylinder, 0.0..=std::f64::consts::TAU, 2.0),
            (&cylinder, &plane, -std::f64::consts::TAU..=0.0, -2.0),
        ] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
                panic!("expected one circular section");
            };
            assert_eq!(circle.domain(), domain);
            assert!((circle.control_points()[1].point().y() - first_y).abs() < 1e-12);
        }
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
    fn control_hull_bounds_reject_separated_nonplanar_surfaces() {
        let warped = |offset: Real| {
            NurbsSurface::try_bilinear([
                point(offset, 0.0, 0.0),
                point(offset + 2.0, 0.0, 0.0),
                point(offset + 2.0, 2.0, 1.0),
                point(offset, 2.0, 0.0),
            ])
            .unwrap()
        };
        let first = warped(0.0);
        let separated = warped(20.0);
        for (left, right) in [(&first, &separated), (&separated, &first)] {
            assert!(
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
        let overlapping = warped(0.5);
        assert!(matches!(
            surface_surface_intersection_events(&first, &overlapping, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn sphere_cylinder_intersection_returns_exact_finite_circles() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap();
        let sphere_at = |height, radius| {
            NurbsSurface::try_sphere(frame.with_origin(point(0.0, 0.0, height)), radius).unwrap()
        };
        let sphere = sphere_at(2.5, 2.5);
        for (first, second) in [(&sphere, &cylinder), (&cylinder, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for (event, expected_z) in events.iter().zip([0.5, 4.5]) {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("expected exact circles, got {events:#?}")
                };
                assert_eq!(circle.degree(), 2);
                assert!(circle.is_closed().unwrap());
                assert!(
                    (circle.length(Tolerance::DEFAULT).unwrap() - 3.0 * std::f64::consts::PI).abs()
                        < 1e-8
                );
                let domain = circle.domain();
                for fraction in [0.0, 0.125, 0.33, 0.75] {
                    let sample = circle
                        .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                        .unwrap();
                    assert!((sample.z() - expected_z).abs() < 1e-9);
                    assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 1e-9);
                    assert!((sample.distance_to(point(0.0, 0.0, 2.5)).unwrap() - 2.5).abs() < 1e-9);
                }
            }
        }
        for (sphere, expected_z) in [(sphere_at(0.0, 2.5), 2.0), (sphere_at(2.5, 1.5), 2.5)] {
            let events =
                surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT)
                    .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
                panic!("expected one finite circle, got {events:#?}")
            };
            assert!(
                (circle.evaluate(*circle.domain().start()).unwrap().z() - expected_z).abs() < 1e-9
            );
        }
        let rim_cylinder = NurbsSurface::try_cylinder(frame, 1.5, 0.0, 2.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(
                &sphere_at(0.0, 2.5),
                &rim_cylinder,
                Tolerance::DEFAULT
            )
            .unwrap()
            .len(),
            1
        );
        assert!(
            surface_surface_intersection_events(
                &sphere_at(2.5, 1.0),
                &cylinder,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
        let near_tangent = surface_surface_intersection_events(
            &sphere_at(2.5, 1.5 + 1.0e-9),
            &cylinder,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(near_tangent.len(), 2);
        let offset_sphere =
            NurbsSurface::try_sphere(frame.with_origin(point(0.5, 0.0, 2.5)), 2.5).unwrap();
        let events =
            surface_surface_intersection_events(&offset_sphere, &cylinder, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("offset sphere and cylinder should meet in smooth curves")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            assert!(
                (curve.length(Tolerance::DEFAULT).unwrap() - 9.586_621_542_627_29).abs() < 1e-8
            );
            let domain = curve.domain();
            for index in 0..=64 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                let sample = curve.evaluate(parameter).unwrap();
                assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 2e-9);
                assert!((sample.distance_to(point(0.5, 0.0, 2.5)).unwrap() - 2.5).abs() < 2e-9);
            }
        }
    }

    #[test]
    fn sphere_cylinder_intersection_respects_rotated_axis_far_from_origin() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            crate::Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap();
        let sphere = NurbsSurface::try_sphere(
            frame.with_origin(frame.point_at([0.0, 0.0, 2.5]).unwrap()),
            2.5,
        )
        .unwrap();
        assert!(
            cylinder
                .canonical_cylinder(Tolerance::DEFAULT)
                .unwrap()
                .is_some()
        );
        assert!(
            sphere
                .canonical_sphere(Tolerance::DEFAULT)
                .unwrap()
                .is_some()
        );
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for (event, expected_axial) in events.iter().zip([0.5, 4.5]) {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("expected exact circles, got {events:#?}")
            };
            let sample = circle.evaluate(*circle.domain().start()).unwrap();
            let local = frame.coordinates_of(sample).unwrap();
            assert!((local[2] - expected_axial).abs() < 1e-7);
            assert!((local[0].hypot(local[1]) - 1.5).abs() < 1e-7);
        }
    }

    #[test]
    fn sphere_sphere_intersection_returns_exact_circle_in_both_orders() {
        let sphere = |center, radius| {
            NurbsSurface::try_sphere(
                crate::Frame3::try_from_normal(
                    center,
                    crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
                radius,
            )
            .unwrap()
        };
        let first = sphere(point(0.0, 0.0, 0.0), 3.0);
        let second = sphere(point(3.0, 0.0, 0.0), 2.0);
        let circle_x: Real = 7.0 / 3.0;
        let circle_radius = (9.0 - circle_x * circle_x).sqrt();
        for (order, (left, right)) in [(&first, &second), (&second, &first)]
            .into_iter()
            .enumerate()
        {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in &events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("expected exact semicircular arcs, got {events:#?}")
                };
                assert_eq!(curve.degree(), 2);
                assert!(!curve.is_closed().unwrap());
                assert!(
                    (curve.length(Tolerance::DEFAULT).unwrap()
                        - std::f64::consts::PI * circle_radius)
                        .abs()
                        < 1e-8
                );
                let domain = curve.domain();
                for fraction in [0.0, 0.125, 0.33, 0.75, 1.0] {
                    let sample = curve
                        .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                        .unwrap();
                    assert!((sample.x() - circle_x).abs() < 1e-9);
                    assert!((sample.distance_to(point(0.0, 0.0, 0.0)).unwrap() - 3.0).abs() < 1e-9);
                    assert!((sample.distance_to(point(3.0, 0.0, 0.0)).unwrap() - 2.0).abs() < 1e-9);
                }
            }
            if order == 0 {
                for (event, sign) in events.iter().zip([1.0, -1.0]) {
                    let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                        unreachable!()
                    };
                    let domain = arc.domain();
                    let middle = arc
                        .evaluate(0.5 * (*domain.start() + *domain.end()))
                        .unwrap();
                    assert!((middle.y() - sign * circle_radius).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn sphere_sphere_intersection_handles_tangent_disjoint_and_coincident_cases() {
        let sphere = |center, radius| {
            NurbsSurface::try_sphere(
                crate::Frame3::try_from_normal(
                    center,
                    crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
                radius,
            )
            .unwrap()
        };
        let first = sphere(point(0.0, 0.0, 0.0), 3.0);
        for (other, expected) in [
            (sphere(point(5.0, 0.0, 0.0), 2.0), point(3.0, 0.0, 0.0)),
            (sphere(point(1.0, 0.0, 0.0), 2.0), point(3.0, 0.0, 0.0)),
            (sphere(point(1.0, 0.0, 0.0), 4.0), point(-3.0, 0.0, 0.0)),
        ] {
            for (left, right) in [(&first, &other), (&other, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert!(
                    matches!(events.as_slice(), [SurfaceSurfaceIntersectionEvent::Point(p)] if p.distance_to(expected).unwrap() < 1e-9)
                );
            }
        }
        for other in [
            sphere(point(6.0, 0.0, 0.0), 2.0),
            sphere(point(0.5, 0.0, 0.0), 1.0),
            sphere(point(0.0, 0.0, 0.0), 1.0),
        ] {
            assert!(
                surface_surface_intersection_events(&first, &other, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
        let coincident = sphere(point(0.0, 0.0, 0.0), 3.0);
        assert!(matches!(
            surface_surface_intersection_events(&first, &coincident, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
        let near_external_tangent = sphere(point(5.0 - 1.0e-9, 0.0, 0.0), 2.0);
        let events =
            surface_surface_intersection_events(&first, &near_external_tangent, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Curve(_)))
        );
    }

    #[test]
    fn sphere_sphere_intersection_is_stable_far_from_origin() {
        let center = point(1.0e8, -1.0e8, 1.0e8);
        let frame = crate::Frame3::try_from_normal(
            center,
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let other_frame = crate::Frame3::try_from_normal(
            point(1.0e8 + 2.0, -1.0e8, 1.0e8),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let second = NurbsSurface::try_sphere(other_frame, 2.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                SurfaceSurfaceIntersectionEvent::Curve(_),
                SurfaceSurfaceIntersectionEvent::Curve(_)
            ]
        ));
    }

    #[test]
    fn sphere_plane_intersection_returns_exact_circle_and_respects_argument_order() {
        let frame = crate::Frame3::try_from_normal(
            point(5.0, 5.0, 1.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let patch = horizontal_surface(0.0);
        for (first, second) in [(&sphere, &patch), (&patch, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("expected a section circle, got {events:#?}")
            };
            assert_eq!(curve.degree(), 2);
            assert!(curve.is_closed().unwrap());
            for fraction in [0.0, 0.125, 0.33, 0.75] {
                let domain = curve.domain();
                let point = curve
                    .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                    .unwrap();
                assert!(point.z().abs() < 1e-10);
                assert!((point.distance_to(frame.origin()).unwrap() - 2.0).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn sphere_plane_intersection_clips_to_finite_patch_and_reports_tangency() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 1.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let half_patch = horizontal_rectangle(0.0, 3.0, -3.0, 3.0, 0.0);
        let events =
            surface_surface_intersection_events(&sphere, &half_patch, Tolerance::DEFAULT).unwrap();
        let curves = events
            .iter()
            .filter_map(|event| match event {
                SurfaceSurfaceIntersectionEvent::Curve(curve) => Some(curve),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(curves.len(), 2);
        for curve in curves {
            assert!(
                (curve.length(Tolerance::DEFAULT).unwrap()
                    - 0.5 * std::f64::consts::PI * 3.0_f64.sqrt())
                .abs()
                    < 1e-7
            );
            for parameter in [*curve.domain().start(), *curve.domain().end()] {
                let point = curve.evaluate(parameter).unwrap();
                assert!(point.x() >= -1e-8);
                assert!(point.z().abs() < 1e-8);
            }
        }

        let tangent_patch = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 3.0);
        let tangent =
            surface_surface_intersection_events(&sphere, &tangent_patch, Tolerance::DEFAULT)
                .unwrap();
        assert!(
            matches!(tangent.as_slice(), [SurfaceSurfaceIntersectionEvent::Point(p)] if p.distance_to(point(0.0, 0.0, 3.0)).unwrap() < 1e-10)
        );
        let disjoint = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 4.0);
        assert!(
            surface_surface_intersection_events(&sphere, &disjoint, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sphere_plane_intersection_handles_rotated_finite_patch() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0, 2.0, 3.0),
            crate::Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let patch = NurbsSurface::try_bilinear([
            frame.point_at([-3.0, -3.0, 0.0]).unwrap(),
            frame.point_at([3.0, -3.0, 0.0]).unwrap(),
            frame.point_at([3.0, 3.0, 0.0]).unwrap(),
            frame.point_at([-3.0, 3.0, 0.0]).unwrap(),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &patch, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
            panic!("expected a rotated great circle, got {events:#?}")
        };
        assert!(circle.is_closed().unwrap());
        assert!(
            (circle.length(Tolerance::DEFAULT).unwrap() - 4.0 * std::f64::consts::PI).abs() < 1e-7
        );
        for control in circle.control_points() {
            assert!(
                patch
                    .plane(Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap()
                    .signed_distance_to(control.point())
                    .unwrap()
                    .abs()
                    < 1e-9
            );
        }
    }

    #[test]
    fn sphere_plane_intersection_is_not_mistaken_for_tangency_far_from_origin() {
        let center = point(1.0e8, -1.0e8, 1.0e8 + 1.0);
        let frame = crate::Frame3::try_from_normal(
            center,
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let patch =
            horizontal_rectangle(1.0e8 - 3.0, 1.0e8 + 3.0, -1.0e8 - 3.0, -1.0e8 + 3.0, 1.0e8);
        let events =
            surface_surface_intersection_events(&sphere, &patch, Tolerance::DEFAULT).unwrap();
        assert!(matches!(
            events.as_slice(),
            [SurfaceSurfaceIntersectionEvent::Curve(_)]
        ));
    }

    #[test]
    fn cylinder_plane_sections_are_exact_circles_and_parallel_lines() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let horizontal = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 2.0);
        let circle =
            surface_surface_intersection_events(&cylinder, &horizontal, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = circle.as_slice() else {
            panic!("expected one circular section, got {circle:#?}")
        };
        assert!(circle.is_closed().unwrap());
        assert!(
            (circle.length(Tolerance::DEFAULT).unwrap() - 4.0 * std::f64::consts::PI).abs() < 1e-7
        );

        let vertical = |x| {
            NurbsSurface::try_bilinear([
                point(x, -3.0, -1.0),
                point(x, 3.0, -1.0),
                point(x, 3.0, 5.0),
                point(x, -3.0, 5.0),
            ])
            .unwrap()
        };
        let crossing =
            surface_surface_intersection_events(&cylinder, &vertical(0.0), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(crossing.len(), 2);
        for event in crossing {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                panic!("expected a straight generatrix")
            };
            assert_eq!(line.degree(), 1);
            assert!((line.length(Tolerance::DEFAULT).unwrap() - 4.0).abs() < 1e-9);
        }
        let clipped_patch = NurbsSurface::try_bilinear([
            point(0.0, -3.0, 1.0),
            point(0.0, 3.0, 1.0),
            point(0.0, 3.0, 3.0),
            point(0.0, -3.0, 3.0),
        ])
        .unwrap();
        let clipped =
            surface_surface_intersection_events(&cylinder, &clipped_patch, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(clipped.len(), 2);
        for event in clipped {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                panic!("expected a clipped straight generatrix")
            };
            assert!((line.length(Tolerance::DEFAULT).unwrap() - 2.0).abs() < 1e-9);
        }
        let tangent =
            surface_surface_intersection_events(&cylinder, &vertical(2.0), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(tangent.len(), 2);
        assert_eq!(tangent[0], tangent[1]);
        let disjoint =
            surface_surface_intersection_events(&cylinder, &vertical(3.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(disjoint.is_empty());
        let oblique = NurbsSurface::try_bilinear([
            point(-3.0, -3.0, 0.5),
            point(3.0, -3.0, 3.5),
            point(3.0, 3.0, 3.5),
            point(-3.0, 3.0, 0.5),
        ])
        .unwrap();
        let oblique_events =
            surface_surface_intersection_events(&cylinder, &oblique, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(ellipse)] = oblique_events.as_slice() else {
            panic!("expected one exact oblique elliptical section, got {oblique_events:#?}")
        };
        assert!(ellipse.is_closed().unwrap());
        assert_eq!(ellipse.degree(), 2);
        assert!((ellipse.length(Tolerance::DEFAULT).unwrap() - 13.31833512).abs() < 1e-5);
        for control in ellipse.control_points() {
            assert!(
                oblique
                    .plane(Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap()
                    .signed_distance_to(control.point())
                    .unwrap()
                    .abs()
                    < 1e-9
            );
        }
        let steep = NurbsSurface::try_bilinear([
            point(-3.0, -3.0, -4.0),
            point(3.0, -3.0, 8.0),
            point(3.0, 3.0, 8.0),
            point(-3.0, 3.0, -4.0),
        ])
        .unwrap();
        let clipped_ellipse =
            surface_surface_intersection_events(&cylinder, &steep, Tolerance::DEFAULT).unwrap();
        assert_eq!(clipped_ellipse.len(), 2);
        for event in clipped_ellipse {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("expected exact elliptical arc sections")
            };
            assert!(!arc.is_closed().unwrap());
            assert!((arc.length(Tolerance::DEFAULT).unwrap() - 4.51582693).abs() < 1e-5);
            for parameter in [*arc.domain().start(), *arc.domain().end()] {
                let point = arc.evaluate(parameter).unwrap();
                assert!(point.z() >= -1e-8 && point.z() <= 4.0 + 1e-8);
                assert!((point.x().hypot(point.y()) - 2.0).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn oblique_cylinder_section_respects_rotated_axis() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0, 2.0, 3.0),
            crate::Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
        let patch = NurbsSurface::try_bilinear([
            frame.point_at([-3.0, -3.0, 0.5]).unwrap(),
            frame.point_at([3.0, -3.0, 3.5]).unwrap(),
            frame.point_at([3.0, 3.0, 3.5]).unwrap(),
            frame.point_at([-3.0, 3.0, 0.5]).unwrap(),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&cylinder, &patch, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(ellipse)] = events.as_slice() else {
            panic!("expected a rotated elliptical section, got {events:#?}")
        };
        assert!(ellipse.is_closed().unwrap());
        assert!((ellipse.length(Tolerance::DEFAULT).unwrap() - 13.31833512).abs() < 1e-5);
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
                context: "coincident planar surfaces outside certified convex or monotone strip patches",
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
            surface_surface_intersection_events(&horizontal, &quadratic, Tolerance::DEFAULT)
                .unwrap(),
            identical
        );
        let elevated = quadratic
            .try_change_degree(3, 2, false)
            .unwrap()
            .try_insert_knot_u(5.0, 1)
            .unwrap()
            .try_insert_knot_v(6.0, 1)
            .unwrap();
        assert_eq!(
            surface_surface_intersection_events(&horizontal, &elevated, Tolerance::DEFAULT)
                .unwrap(),
            identical
        );
        let partial = surface_surface_intersection_events(
            &elevated,
            &horizontal_rectangle(5.0, 15.0, 0.0, 10.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(partial)] = partial.as_slice() else {
            panic!("expected elevated partial overlap, got {partial:#?}");
        };
        assert!(partial.is_closed().unwrap());
        assert!((partial.length(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1e-9);
        let shared_edge = surface_surface_intersection_events(
            &elevated,
            &horizontal_rectangle(10.0, 20.0, 0.0, 10.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(shared_edge)] = shared_edge.as_slice() else {
            panic!("expected elevated shared edge, got {shared_edge:#?}");
        };
        assert!((shared_edge.length(Tolerance::DEFAULT).unwrap() - 10.0).abs() < 1e-9);
        let elevated_brep = Brep::try_surface_face(elevated, Tolerance::DEFAULT).unwrap();
        let horizontal_brep =
            Brep::try_surface_face(horizontal.clone(), Tolerance::DEFAULT).unwrap();
        let surface_events =
            surface_brep_intersection_events(&horizontal, &elevated_brep, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(surface_boundary)] = surface_events.as_slice()
        else {
            panic!("expected affine surface/B-rep perimeter, got {surface_events:#?}");
        };
        assert!(surface_boundary.is_closed().unwrap());
        assert!((surface_boundary.length(Tolerance::DEFAULT).unwrap() - 40.0).abs() < 1e-9);
        let brep_events =
            brep_brep_intersection_events(&elevated_brep, &horizontal_brep, Tolerance::DEFAULT)
                .unwrap();
        let [BrepBrepIntersectionEvent::Curve(brep_boundary)] = brep_events.as_slice() else {
            panic!("expected affine B-rep perimeter, got {brep_events:#?}");
        };
        assert!(brep_boundary.is_closed().unwrap());
        assert!((brep_boundary.length(Tolerance::DEFAULT).unwrap() - 40.0).abs() < 1e-9);
        let bent_boundary = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(5.0, 2.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(5.0, 10.0, 0.0),
                point(10.0, 10.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
            vec![0.0, 0.0, 10.0, 10.0],
        )
        .unwrap();
        let bent_events =
            surface_surface_intersection_events(&horizontal, &bent_boundary, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(bent_perimeter)] = bent_events.as_slice()
        else {
            panic!("a contained bent strip must produce its perimeter, got {bent_events:#?}")
        };
        assert!(bent_perimeter.is_closed().unwrap());
        assert_eq!(bent_perimeter.degree(), 2);
        for target in [
            point(0.0, 0.0, 0.0),
            point(5.0, 1.0, 0.0),
            point(10.0, 10.0, 0.0),
        ] {
            let parameter = bent_perimeter
                .closest_parameter(target, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                bent_perimeter
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-9
            );
        }
        let partial_plane = horizontal_rectangle(2.0, 8.0, -1.0, 5.0, 0.0);
        for (first, second) in [
            (&partial_plane, &bent_boundary),
            (&bent_boundary, &partial_plane),
        ] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(perimeter)] = events.as_slice() else {
                panic!("a clipped bent strip must produce one perimeter, got {events:#?}")
            };
            assert!(perimeter.is_closed().unwrap());
            assert_eq!(perimeter.degree(), 2);
            for target in [
                point(2.0, 0.64, 0.0),
                point(5.0, 1.0, 0.0),
                point(8.0, 0.64, 0.0),
                point(8.0, 5.0, 0.0),
                point(2.0, 5.0, 0.0),
            ] {
                let parameter = perimeter
                    .closest_parameter(target, Tolerance::DEFAULT)
                    .unwrap();
                assert!(
                    perimeter
                        .evaluate(parameter)
                        .unwrap()
                        .distance_to(target)
                        .unwrap()
                        < 1e-9
                );
            }
        }
        let bent_brep = Brep::try_surface_face(bent_boundary.clone(), Tolerance::DEFAULT).unwrap();
        let surface_brep_events =
            surface_brep_intersection_events(&partial_plane, &bent_brep, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(surface_brep_perimeter)] =
            surface_brep_events.as_slice()
        else {
            panic!("a bent B-rep face must produce the clipped perimeter")
        };
        assert!(surface_brep_perimeter.is_closed().unwrap());
        let partial_brep =
            Brep::try_surface_face(partial_plane.clone(), Tolerance::DEFAULT).unwrap();
        let brep_events =
            brep_brep_intersection_events(&partial_brep, &bent_brep, Tolerance::DEFAULT).unwrap();
        let [BrepBrepIntersectionEvent::Curve(brep_perimeter)] = brep_events.as_slice() else {
            panic!("coincident bent and planar B-rep faces must produce one perimeter")
        };
        assert!(brep_perimeter.is_closed().unwrap());
        for plane in [
            horizontal_rectangle(10.0, 20.0, 10.0, 20.0, 0.0),
            horizontal_rectangle(20.0, 30.0, 0.0, 10.0, 0.0),
        ] {
            assert!(
                surface_surface_intersection_events(&plane, &bent_boundary, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
        let rotated_strip = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            bent_boundary
                .control_points()
                .iter()
                .map(|control| point(-control.point().y(), control.point().x(), 0.0))
                .collect(),
            bent_boundary.knots_u().to_vec(),
            bent_boundary.knots_v().to_vec(),
        )
        .unwrap();
        let rotated_events = surface_surface_intersection_events(
            &horizontal_rectangle(-12.0, 2.0, -2.0, 12.0, 0.0),
            &rotated_strip,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(rotated_perimeter)] = rotated_events.as_slice()
        else {
            panic!("a rotated bent strip must retain one perimeter, got {rotated_events:#?}")
        };
        assert!(rotated_perimeter.is_closed().unwrap());
        let elevated_strip = bent_boundary
            .try_change_degree(3, 1, false)
            .unwrap()
            .try_insert_knot_u(5.0, 1)
            .unwrap();
        let elevated_events = surface_surface_intersection_events(
            &horizontal_rectangle(-2.0, 12.0, -2.0, 12.0, 0.0),
            &elevated_strip,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(elevated_perimeter)] =
            elevated_events.as_slice()
        else {
            panic!("a refined bent strip must retain one perimeter, got {elevated_events:#?}")
        };
        assert!(elevated_perimeter.is_closed().unwrap());
        for target in [
            point(0.0, 0.0, 0.0),
            point(5.0, 1.0, 0.0),
            point(10.0, 10.0, 0.0),
        ] {
            let parameter = elevated_perimeter
                .closest_parameter(target, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                elevated_perimeter
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-9
            );
        }
    }

    #[test]
    fn coincident_curved_planar_strips_keep_the_exact_shared_boundary() {
        let point = |x, y| point(x, y, 0.0);
        let strip = |bottom: [Real; 3], top: Real| {
            NurbsSurface::try_new(
                2,
                1,
                3,
                2,
                vec![
                    point(0.0, bottom[0]),
                    point(5.0, bottom[1]),
                    point(10.0, bottom[2]),
                    point(0.0, top),
                    point(5.0, top),
                    point(10.0, top),
                ],
                vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0],
                vec![0.0, 0.0, 10.0, 10.0],
            )
            .unwrap()
        };
        let lower = strip([0.0, 2.0, 0.0], 10.0);
        let upper = strip([2.0, 3.0, 2.0], 12.0);
        for (first, second) in [(&lower, &upper), (&upper, &lower)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(perimeter)] = events.as_slice() else {
                panic!("two overlapping bent strips must share one perimeter, got {events:#?}")
            };
            assert!(perimeter.is_closed().unwrap());
            assert_eq!(perimeter.degree(), 2);
            for target in [
                point(0.0, 2.0),
                point(5.0, 2.5),
                point(10.0, 2.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ] {
                let parameter = perimeter
                    .closest_parameter(target, Tolerance::DEFAULT)
                    .unwrap();
                assert!(
                    perimeter
                        .evaluate(parameter)
                        .unwrap()
                        .distance_to(target)
                        .unwrap()
                        < 1e-9
                );
            }
        }
    }

    #[test]
    fn coincident_rational_bilinear_patches_use_their_convex_corner_region() {
        let weighted = |sign: Real| {
            NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                [
                    (point(0.0, 0.0, 0.0), 1.0),
                    (point(0.0, 10.0, 0.0), 2.0),
                    (point(10.0, 0.0, 0.0), 5.0),
                    (point(10.0, 10.0, 0.0), 11.0),
                ]
                .into_iter()
                .map(|(point, weight)| crate::WeightedPoint3::try_new(point, sign * weight))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
                vec![0.0, 0.0, 10.0, 10.0],
                vec![0.0, 0.0, 10.0, 10.0],
            )
            .unwrap()
        };
        let shifted = horizontal_rectangle(5.0, 15.0, 0.0, 10.0, 0.0);
        for sign in [1.0, -1.0] {
            let rational = weighted(sign);
            let events =
                surface_surface_intersection_events(&rational, &shifted, Tolerance::DEFAULT)
                    .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(overlap)] = events.as_slice() else {
                panic!("expected one rational overlap perimeter, got {events:#?}");
            };
            assert!(overlap.is_closed().unwrap());
            assert!((overlap.length(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1e-9);
            let actual = overlap
                .control_points()
                .iter()
                .map(|point| point.point())
                .collect::<Vec<_>>();
            for corner in [
                point(5.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(5.0, 10.0, 0.0),
            ] {
                assert!(
                    actual
                        .iter()
                        .any(|point| point.is_near(corner, Tolerance::DEFAULT))
                );
            }
            let reversed =
                surface_surface_intersection_events(&shifted, &rational, Tolerance::DEFAULT)
                    .unwrap();
            assert_eq!(reversed.len(), 1);
            let first_brep = Brep::try_surface_face(rational.clone(), Tolerance::DEFAULT).unwrap();
            let second_brep = Brep::try_surface_face(shifted.clone(), Tolerance::DEFAULT).unwrap();
            let brep_events =
                brep_brep_intersection_events(&first_brep, &second_brep, Tolerance::DEFAULT)
                    .unwrap();
            let [BrepBrepIntersectionEvent::Curve(brep_overlap)] = brep_events.as_slice() else {
                panic!("expected one rational B-rep overlap perimeter, got {brep_events:#?}");
            };
            assert!(brep_overlap.is_closed().unwrap());
            assert!((brep_overlap.length(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1e-9);
            let edge = surface_surface_intersection_events(
                &rational,
                &horizontal_rectangle(10.0, 20.0, 0.0, 10.0, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(edge)] = edge.as_slice() else {
                panic!("expected a rational shared edge, got {edge:#?}");
            };
            assert!((edge.length(Tolerance::DEFAULT).unwrap() - 10.0).abs() < 1e-9);
        }
    }

    #[test]
    fn coincident_projective_patches_survive_degree_elevation_and_knot_refinement() {
        let corners = [
            point(0.0, 0.0, 0.0),
            point(5.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 20.0 / 3.0, 0.0),
        ];
        let perimeter = (0..4)
            .map(|index| {
                corners[index]
                    .distance_to(corners[(index + 1) % 4])
                    .unwrap()
            })
            .sum::<Real>();
        let enclosing = horizontal_rectangle(-1.0, 8.0, -1.0, 8.0, 0.0);
        for sign in [1.0, -1.0] {
            let source = NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                [
                    (corners[0], 1.0),
                    (corners[1], 2.0),
                    (corners[3], 1.5),
                    (corners[2], 2.5),
                ]
                .into_iter()
                .map(|(point, weight)| crate::WeightedPoint3::try_new(point, sign * weight))
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
                vec![0.0, 0.0, 1.0, 1.0],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap();
            let refined = source
                .try_change_degree(3, 2, false)
                .unwrap()
                .try_insert_knot_u(0.4, 2)
                .unwrap()
                .try_insert_knot_v(0.65, 1)
                .unwrap();
            let certified = refined
                .try_projective_patch_corners(Tolerance::DEFAULT)
                .unwrap();
            for (actual, expected) in certified.into_iter().zip(corners) {
                assert!(actual.is_near(expected, Tolerance::DEFAULT));
            }
            let events =
                surface_surface_intersection_events(&refined, &enclosing, Tolerance::DEFAULT)
                    .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(boundary)] = events.as_slice() else {
                panic!("expected projective overlap perimeter, got {events:#?}");
            };
            assert!(boundary.is_closed().unwrap());
            assert!((boundary.length(Tolerance::DEFAULT).unwrap() - perimeter).abs() < 1e-9);
            for (actual, expected) in boundary.control_points().iter().zip(corners) {
                assert!(actual.point().is_near(expected, Tolerance::DEFAULT));
            }
            let partial = surface_surface_intersection_events(
                &refined,
                &horizontal_rectangle(2.0, 8.0, -1.0, 8.0, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(partial)] = partial.as_slice() else {
                panic!("expected projective partial overlap, got {partial:#?}");
            };
            let partial_corners = [
                point(2.0, 0.0, 0.0),
                corners[1],
                corners[2],
                point(2.0, 16.0 / 3.0, 0.0),
            ];
            let partial_perimeter = (0..4)
                .map(|index| {
                    partial_corners[index]
                        .distance_to(partial_corners[(index + 1) % 4])
                        .unwrap()
                })
                .sum::<Real>();
            assert!(partial.is_closed().unwrap());
            assert!((partial.length(Tolerance::DEFAULT).unwrap() - partial_perimeter).abs() < 1e-9);
            for corner in partial_corners {
                assert!(
                    partial
                        .control_points()
                        .iter()
                        .any(|control| control.point().is_near(corner, Tolerance::DEFAULT))
                );
            }
            let edge = surface_surface_intersection_events(
                &refined,
                &horizontal_rectangle(0.0, 5.0, -5.0, 0.0, 0.0),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(edge)] = edge.as_slice() else {
                panic!("expected projective shared edge, got {edge:#?}");
            };
            assert!((edge.length(Tolerance::DEFAULT).unwrap() - 5.0).abs() < 1e-9);
            let brep = Brep::try_surface_face(refined, Tolerance::DEFAULT).unwrap();
            let surface_events =
                surface_brep_intersection_events(&enclosing, &brep, Tolerance::DEFAULT).unwrap();
            let [SurfaceBrepIntersectionEvent::Curve(surface_boundary)] = surface_events.as_slice()
            else {
                panic!("expected projective surface/B-rep perimeter, got {surface_events:#?}");
            };
            assert!(
                (surface_boundary.length(Tolerance::DEFAULT).unwrap() - perimeter).abs() < 1e-9
            );
            let enclosing_brep =
                Brep::try_surface_face(enclosing.clone(), Tolerance::DEFAULT).unwrap();
            let brep_events =
                brep_brep_intersection_events(&brep, &enclosing_brep, Tolerance::DEFAULT).unwrap();
            let [BrepBrepIntersectionEvent::Curve(brep_boundary)] = brep_events.as_slice() else {
                panic!("expected projective B-rep perimeter, got {brep_events:#?}");
            };
            assert!((brep_boundary.length(Tolerance::DEFAULT).unwrap() - perimeter).abs() < 1e-9);
        }
        let mixed = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [
                (corners[0], 1.0),
                (corners[1], -2.0),
                (corners[3], 1.5),
                (corners[2], 2.5),
            ]
            .into_iter()
            .map(|(point, weight)| crate::WeightedPoint3::try_new(point, weight))
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(
            mixed.try_projective_patch_corners(Tolerance::DEFAULT),
            Err(GeometryError::InvalidControlNet {
                context: "projective patch requires weights of one sign"
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

        let assert_box_edges =
            |events: Vec<BrepBrepIntersectionEvent>, minimum: [Real; 3], maximum: [Real; 3]| {
                let mut actual_edges = Vec::new();
                for event in events {
                    let BrepBrepIntersectionEvent::Curve(curve) = event else {
                        panic!("coincident boxes must intersect only along their edges")
                    };
                    assert_eq!(curve.degree(), 1);
                    actual_edges.extend(
                        curve
                            .control_points()
                            .windows(2)
                            .map(|controls| [controls[0].point(), controls[1].point()]),
                    );
                }
                assert_eq!(actual_edges.len(), 12);
                for axis in 0..3 {
                    let other_axes = (0..3).filter(|other| *other != axis).collect::<Vec<_>>();
                    for first_fixed in [minimum[other_axes[0]], maximum[other_axes[0]]] {
                        for second_fixed in [minimum[other_axes[1]], maximum[other_axes[1]]] {
                            let mut start = minimum;
                            let mut end = minimum;
                            end[axis] = maximum[axis];
                            start[other_axes[0]] = first_fixed;
                            end[other_axes[0]] = first_fixed;
                            start[other_axes[1]] = second_fixed;
                            end[other_axes[1]] = second_fixed;
                            let expected = [
                                point(start[0], start[1], start[2]),
                                point(end[0], end[1], end[2]),
                            ];
                            assert_eq!(
                                actual_edges
                                    .iter()
                                    .filter(|edge| point_pairs_match(**edge, expected, 1e-10))
                                    .count(),
                                1,
                                "each box edge must occur once: {expected:?}"
                            );
                        }
                    }
                }
            };
        let coincident = brep_brep_intersection_events(&first, &first, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            coincident.len(),
            4,
            "Rhino joins identical box edges into four paths"
        );
        assert_box_edges(coincident, [0.0; 3], [10.0; 3]);
        let coaxial = box_brep_with_intervals([[0.0, 10.0], [0.0, 10.0], [5.0, 15.0]]);
        let overlap = brep_brep_intersection_events(&first, &coaxial, Tolerance::DEFAULT).unwrap();
        assert_box_edges(overlap, [0.0, 0.0, 5.0], [10.0, 10.0, 10.0]);
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

        let coincident =
            brep_brep_intersection_events(&horizontal, &horizontal, Tolerance::DEFAULT).unwrap();
        assert_eq!(coincident.len(), 2, "outer and hole loops: {coincident:#?}");
        assert!(coincident.iter().all(|event| matches!(
            event,
            BrepBrepIntersectionEvent::Curve(curve) if curve.is_closed().unwrap()
        )));
    }

    #[test]
    fn brep_brep_intersection_traces_coincident_disk_and_partial_faces() {
        let circle = Circle3::try_new(
            point(0.0, 0.0, 0.0),
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let disk = Brep::try_planar_face(&circle, Tolerance::DEFAULT).unwrap();
        let broad = Brep::try_surface_face(
            horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let events = brep_brep_intersection_events(&disk, &broad, Tolerance::DEFAULT).unwrap();
        let [BrepBrepIntersectionEvent::Curve(perimeter)] = events.as_slice() else {
            panic!("expected one exact disk perimeter, got {events:#?}")
        };
        assert!(perimeter.is_closed().unwrap());
        assert_eq!(perimeter.control_points(), circle.control_points());

        let [south, _north] = Brep::try_split_rectangular_surface_face_v(
            horizontal_rectangle(0.0, 4.0, 0.0, 4.0, 0.0),
            0.0..=4.0,
            0.0..=4.0,
            2.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let partial = Brep::try_surface_face(
            horizontal_rectangle(1.0, 3.0, -1.0, 3.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (first, second) in [(&south, &partial), (&partial, &south)] {
            let events = brep_brep_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [BrepBrepIntersectionEvent::Curve(perimeter)] = events.as_slice() else {
                panic!("expected one partial overlap perimeter, got {events:#?}")
            };
            assert!(perimeter.is_closed().unwrap());
            for corner in [
                point(1.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 2.0, 0.0),
                point(1.0, 2.0, 0.0),
            ] {
                let parameter = perimeter
                    .closest_parameter(corner, Tolerance::DEFAULT)
                    .unwrap();
                assert!(
                    perimeter
                        .evaluate(parameter)
                        .unwrap()
                        .distance_to(corner)
                        .unwrap()
                        < 1e-9
                );
            }
        }
        let edge_neighbor = Brep::try_surface_face(
            horizontal_rectangle(4.0, 6.0, 0.0, 2.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge_events =
            brep_brep_intersection_events(&south, &edge_neighbor, Tolerance::DEFAULT).unwrap();
        let [BrepBrepIntersectionEvent::Curve(edge)] = edge_events.as_slice() else {
            panic!("expected one shared face edge, got {edge_events:#?}")
        };
        let endpoints = [
            edge.control_points().first().unwrap().point(),
            edge.control_points().last().unwrap().point(),
        ];
        assert!(endpoints.contains(&point(4.0, 0.0, 0.0)));
        assert!(endpoints.contains(&point(4.0, 2.0, 0.0)));

        let vertex_neighbor = Brep::try_surface_face(
            horizontal_rectangle(4.0, 6.0, 2.0, 4.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let vertex_events =
            brep_brep_intersection_events(&south, &vertex_neighbor, Tolerance::DEFAULT).unwrap();
        let [BrepBrepIntersectionEvent::Point(contact)] = vertex_events.as_slice() else {
            panic!("expected one shared face vertex, got {vertex_events:#?}")
        };
        assert!(contact.is_near(point(4.0, 2.0, 0.0), Tolerance::DEFAULT));
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
    fn surface_brep_intersection_clips_face_holes_and_traces_coincident_boundaries() {
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

        let coincident =
            surface_brep_intersection_events(&horizontal_surface(0.0), &face, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(coincident.len(), 2, "outer and hole loops: {coincident:#?}");
        assert!(coincident.iter().all(|event| matches!(
            event,
            SurfaceBrepIntersectionEvent::Curve(curve) if curve.is_closed().unwrap()
        )));
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
    fn surface_brep_intersection_traces_coincident_disk_and_partial_rectangle() {
        let disk_boundary = Circle3::try_new(
            point(0.0, 0.0, 0.0),
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let disk = Brep::try_planar_face(&disk_boundary, Tolerance::DEFAULT).unwrap();
        let broad_plane = horizontal_rectangle(-3.0, 3.0, -3.0, 3.0, 0.0);
        let disk_events =
            surface_brep_intersection_events(&broad_plane, &disk, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(circle)] = disk_events.as_slice() else {
            panic!("expected one exact disk boundary, got {disk_events:#?}")
        };
        assert!(circle.is_closed().unwrap());
        assert_eq!(circle.degree(), disk_boundary.degree());
        assert_eq!(circle.control_points(), disk_boundary.control_points());

        let half_plane = horizontal_rectangle(0.0, 3.0, -3.0, 3.0, 0.0);
        let half_events =
            surface_brep_intersection_events(&half_plane, &disk, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(half_perimeter)] = half_events.as_slice() else {
            panic!("expected one half-disk perimeter, got {half_events:#?}")
        };
        assert!(half_perimeter.is_closed().unwrap());
        for endpoint in [point(0.0, -2.0, 0.0), point(0.0, 2.0, 0.0)] {
            let parameter = half_perimeter
                .closest_parameter(endpoint, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                half_perimeter
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(endpoint)
                    .unwrap()
                    < 1.0e-9
            );
        }
        let tangent_plane = horizontal_rectangle(2.0, 3.0, -3.0, 3.0, 0.0);
        let tangent_events =
            surface_brep_intersection_events(&tangent_plane, &disk, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Point(contact)] = tangent_events.as_slice() else {
            panic!("expected one disk tangent point, got {tangent_events:#?}")
        };
        assert!(contact.is_near(point(2.0, 0.0, 0.0), Tolerance::DEFAULT));

        let rectangle = horizontal_rectangle(0.0, 4.0, 0.0, 4.0, 0.0);
        let [south, _north] = Brep::try_split_rectangular_surface_face_v(
            rectangle,
            0.0..=4.0,
            0.0..=4.0,
            2.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let partial_plane = horizontal_rectangle(1.0, 3.0, -1.0, 3.0, 0.0);
        let partial_events =
            surface_brep_intersection_events(&partial_plane, &south, Tolerance::DEFAULT).unwrap();
        let [SurfaceBrepIntersectionEvent::Curve(perimeter)] = partial_events.as_slice() else {
            panic!("expected one overlap perimeter, got {partial_events:#?}")
        };
        assert!(perimeter.is_closed().unwrap());
        let corners = [
            point(1.0, 0.0, 0.0),
            point(3.0, 0.0, 0.0),
            point(3.0, 2.0, 0.0),
            point(1.0, 2.0, 0.0),
        ];
        for corner in corners {
            let parameter = perimeter
                .closest_parameter(corner, Tolerance::DEFAULT)
                .unwrap();
            assert!(
                perimeter
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(corner)
                    .unwrap()
                    < 1.0e-9
            );
        }
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
