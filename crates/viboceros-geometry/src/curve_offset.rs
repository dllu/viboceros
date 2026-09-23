//! Analytic offsets and tolerance-checked smooth approximations in oriented planes.

use std::cmp::Ordering;

use crate::exact_scalar::rational;

mod ellipse;
mod nurbs;
mod polycurve;
use ellipse::{
    ellipse_offset_side, ellipse_region_contains, ellipse_through_distance, offset_ellipse,
};
use nurbs::{
    linear_nurbs_proxy, nurbs_offset_side, nurbs_region_contains, nurbs_region_inward_sign,
    nurbs_through_distance, offset_nurbs, offset_nurbs_chamfer, offset_nurbs_open_gaps,
};
use polycurve::offset_proxy;

use crate::{
    Circle3, CircularArc3, Curve3, CurveSegment3, GeometryError, LineSegment,
    MAX_POLYCURVE_SEGMENTS, Point3, PolyCurve3, Polyline3, Real, Tolerance, UnitVector3, Vector3,
};

/// How an offset polyline joins neighboring segments at a convex corner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CurveOffsetCornerStyle {
    #[default]
    Sharp,
    Chamfer,
    Round,
    None,
}

impl Curve3 {
    /// Classify two closed offset boundaries without curve intersection when
    /// their boxes are separate or their supporting circles are parallel.
    /// `None` asks the caller to use the general curve intersection solver.
    pub fn offset_region_boundary_relation(
        &self,
        other: &Self,
        tolerance: Tolerance,
    ) -> Result<Option<bool>, GeometryError> {
        let a = self.as_ref().tight_bounds(tolerance)?;
        let b = other.as_ref().tight_bounds(tolerance)?;
        let coordinate_scale = [a.min(), a.max(), b.min(), b.max()]
            .into_iter()
            .flat_map(Point3::to_array)
            .fold(1.0_f64, |scale, value| scale.max(value.abs()));
        // Circle NURBS controls can extend beyond attained bounds. Leave a
        // margin relative to the coordinate scale used by the fallback solver.
        let threshold = tolerance
            .absolute()
            .max(4.0 * tolerance.relative() * coordinate_scale);
        let a_min = a.min().to_array();
        let a_max = a.max().to_array();
        let b_min = b.min().to_array();
        let b_max = b.max().to_array();
        if (0..3).any(|axis| {
            a_max[axis] < b_min[axis] && b_min[axis] - a_max[axis] > threshold
                || b_max[axis] < a_min[axis] && a_min[axis] - b_max[axis] > threshold
        }) {
            return Ok(Some(false));
        }
        let circle = |curve: &Curve3| -> Result<Option<Circle3>, GeometryError> {
            match curve {
                Curve3::Circle(circle) => Ok(Some(*circle)),
                Curve3::Arc(arc) if arc.is_closed() => Ok(Some(Circle3::try_from_frame(
                    arc.center(),
                    arc.radius(),
                    arc.x_axis(),
                    arc.normal()?,
                    tolerance,
                )?)),
                _ => Ok(None),
            }
        };
        let (Some(first), Some(second)) = (circle(self)?, circle(other)?) else {
            return Ok(None);
        };
        let first_normal = first.normal()?;
        let second_normal = second.normal()?;
        // Exact parallelism keeps the planar radial test from excluding an
        // intersection between slightly tilted, very large circles.
        if first_normal
            .as_vector()
            .cross(second_normal.as_vector())?
            .length()?
            != 0.0
        {
            return Ok(None);
        }
        let between = first.center().vector_to(second.center())?;
        if between.dot(first_normal.as_vector())?.abs() > threshold {
            return Ok(Some(false));
        }
        let distance = in_plane_radius(
            first.center(),
            second.center(),
            first.x_axis(),
            first.y_axis(),
        )?;
        let sum = first.radius() + second.radius();
        let difference = (first.radius() - second.radius()).abs();
        if !sum.is_finite() {
            return Ok(None);
        }
        if distance > sum && distance - sum > threshold
            || distance < difference && difference - distance > threshold
        {
            return Ok(Some(false));
        }
        if distance == sum
            || distance == difference
            || distance > difference + threshold && distance + threshold < sum
        {
            return Ok(Some(true));
        }
        Ok(None)
    }

    /// Whether a point is strictly inside a supported closed offset region.
    /// Open curves return `None`; a point on the boundary is ambiguous.
    pub fn offset_region_contains(
        &self,
        point: Point3,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Option<bool>, GeometryError> {
        let circle = match self {
            Self::Circle(circle) => Some(*circle),
            Self::Arc(arc) if arc.is_closed() => Some(Circle3::try_from_frame(
                arc.center(),
                arc.radius(),
                arc.x_axis(),
                arc.normal()?,
                tolerance,
            )?),
            _ => None,
        };
        if let Some(circle) = circle {
            let relative = circle.center().vector_to(point)?;
            if relative.dot(circle.normal()?.as_vector())?.abs() > tolerance.absolute() {
                return Ok(Some(false));
            }
            let radial = in_plane_radius(circle.center(), point, circle.x_axis(), circle.y_axis())?;
            if (radial - circle.radius()).abs() <= tolerance.absolute() {
                return Err(GeometryError::AmbiguousCurveOffsetSide);
            }
            return Ok(Some(radial < circle.radius()));
        }
        if let Self::Ellipse(ellipse) = self {
            return ellipse_region_contains(*ellipse, point, tolerance).map(Some);
        }
        if let Self::NurbsCurve(curve) = self {
            return if curve.is_closed()? {
                if let Some(polyline) = linear_nurbs_proxy(curve, tolerance)? {
                    Self::Polyline(polyline).offset_region_contains(point, plane_normal, tolerance)
                } else {
                    nurbs_region_contains(curve, point, plane_normal, tolerance).map(Some)
                }
            } else {
                Ok(None)
            };
        }
        if let Self::PolyCurve(curve) = self {
            return offset_proxy(curve, tolerance)?.offset_region_contains(
                point,
                plane_normal,
                tolerance,
            );
        }
        let Self::Polyline(polyline) = self else {
            return Ok(None);
        };
        if !polyline.is_closed() {
            return Ok(None);
        }
        let normal = polyline_offset_normal(polyline, plane_normal, tolerance)?;
        let origin = polyline.vertices()[0];
        if origin.vector_to(point)?.dot(normal.as_vector())?.abs() > tolerance.absolute() {
            return Ok(Some(false));
        }
        if polyline
            .closest_point(point, tolerance)?
            .distance_to(point)?
            <= tolerance.absolute()
        {
            return Err(GeometryError::AmbiguousCurveOffsetSide);
        }
        let x_axis = polyline.vertices()[0].direction_to(polyline.vertices()[1])?;
        let y_axis = normal
            .as_vector()
            .cross(x_axis.as_vector())?
            .normalized_nonzero()?;
        let relative = origin.vector_to(point)?;
        let px = relative.dot(x_axis.as_vector())?;
        let py = relative.dot(y_axis.as_vector())?;
        let mut inside = false;
        for edge in polyline.vertices().windows(2) {
            let a = origin.vector_to(edge[0])?;
            let b = origin.vector_to(edge[1])?;
            let (ax, ay) = (a.dot(x_axis.as_vector())?, a.dot(y_axis.as_vector())?);
            let (bx, by) = (b.dot(x_axis.as_vector())?, b.dot(y_axis.as_vector())?);
            if (ay > py) != (by > py) && px < ax + (py - ay) * (bx - ax) / (by - ay) {
                inside = !inside;
            }
        }
        Ok(Some(inside))
    }

    /// Signed offset direction toward the interior of a closed region.
    pub fn offset_region_inward_sign(
        &self,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Option<Real>, GeometryError> {
        match self {
            Self::Circle(_) => return Ok(Some(1.0)),
            Self::Arc(arc) if arc.is_closed() => return Ok(Some(1.0)),
            Self::Ellipse(_) => return Ok(Some(1.0)),
            Self::NurbsCurve(curve) if curve.is_closed()? => {
                if let Some(polyline) = linear_nurbs_proxy(curve, tolerance)? {
                    return Self::Polyline(polyline)
                        .offset_region_inward_sign(plane_normal, tolerance);
                }
                return nurbs_region_inward_sign(curve, plane_normal, tolerance).map(Some);
            }
            Self::PolyCurve(curve) => {
                return offset_proxy(curve, tolerance)?
                    .offset_region_inward_sign(plane_normal, tolerance);
            }
            Self::Polyline(polyline) if polyline.is_closed() => {
                let normal = polyline_offset_normal(polyline, plane_normal, tolerance)?;
                let origin = polyline.vertices()[0];
                let x_axis = origin.direction_to(polyline.vertices()[1])?;
                let y_axis = normal
                    .as_vector()
                    .cross(x_axis.as_vector())?
                    .normalized_nonzero()?;
                let scale = polyline
                    .vertices()
                    .iter()
                    .try_fold(0.0_f64, |largest, point| {
                        Ok::<_, GeometryError>(largest.max(origin.distance_to(*point)?))
                    })?;
                let projected = polyline
                    .vertices()
                    .iter()
                    .map(|point| {
                        let relative = origin.vector_to(*point)?;
                        Ok::<_, GeometryError>([
                            relative.dot(x_axis.as_vector())? / scale,
                            relative.dot(y_axis.as_vector())? / scale,
                        ])
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                validate_simple_offset_region(&projected)?;
                let mut area2 = 0.0;
                for edge in projected.windows(2) {
                    area2 += edge[0][0].mul_add(edge[1][1], -(edge[0][1] * edge[1][0]));
                }
                if area2.abs() <= tolerance.relative() {
                    return Err(GeometryError::DegenerateOffsetRegion);
                }
                return Ok(Some(area2.signum()));
            }
            _ => {}
        }
        Ok(None)
    }

    /// A point on a supported closed region's boundary for nesting tests.
    pub fn offset_region_boundary_point(&self) -> Result<Option<Point3>, GeometryError> {
        match self {
            Self::Circle(circle) => Ok(Some(circle.point_at_angle(0.0)?)),
            Self::Arc(arc) if arc.is_closed() => Ok(Some(arc.start()?)),
            Self::Ellipse(ellipse) => Ok(Some(ellipse.point_at_angle(0.0)?)),
            Self::NurbsCurve(curve) if curve.is_closed()? => {
                Ok(Some(curve.evaluate(*curve.domain().start())?))
            }
            Self::PolyCurve(curve) if curve.is_closed()? => Ok(Some(
                curve.segments()[0].evaluate(*curve.segments()[0].domain().start())?,
            )),
            Self::Polyline(polyline) if polyline.is_closed() => Ok(Some(polyline.vertices()[0])),
            _ => Ok(None),
        }
    }

    /// Offset left of the curve direction for positive `distance`. Lines use
    /// `plane_normal`; circular curves use their own oriented supporting plane.
    /// Native parameter intervals and analytic representations are retained.
    pub fn try_offset(
        &self,
        distance: Real,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.try_offset_with_corner_style(
            distance,
            plane_normal,
            tolerance,
            CurveOffsetCornerStyle::Sharp,
        )
    }

    /// Offset with the requested corner treatment for planar polylines and
    /// supported kinked NURBS/polycurves. Analytic families ignore `corner`.
    pub fn try_offset_with_corner_style(
        &self,
        distance: Real,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
        corner: CurveOffsetCornerStyle,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0.0 {
            return Err(GeometryError::InvalidCurveOffsetDistance);
        }
        if corner == CurveOffsetCornerStyle::None
            && matches!(
                self,
                Self::Polyline(_) | Self::NurbsCurve(_) | Self::PolyCurve(_)
            )
        {
            let mut pieces = self.try_offset_parts(distance, plane_normal, tolerance, corner)?;
            return if pieces.len() == 1 {
                Ok(pieces.remove(0))
            } else {
                Err(GeometryError::DisconnectedCurveOffset)
            };
        }
        match self {
            Self::Line(line) => {
                let left = plane_normal
                    .as_vector()
                    .cross(line.direction(tolerance)?.as_vector())?
                    .normalized(tolerance)?;
                let shift = left.as_vector().scaled(distance)?;
                let start = line.start().translated(shift)?;
                let end = line.end().translated(shift)?;
                if start == line.start() || end == line.end() {
                    return Err(GeometryError::Degenerate {
                        context: "offset line under model precision",
                    });
                }
                Ok(Self::Line(line.try_with_endpoints(
                    Some(start),
                    Some(end),
                    tolerance,
                )?))
            }
            Self::Circle(circle) => Ok(Self::Circle(offset_circle(*circle, distance, tolerance)?)),
            Self::Arc(arc) => {
                let circle = Circle3::try_from_frame(
                    arc.center(),
                    arc.radius(),
                    arc.x_axis(),
                    arc.normal()?,
                    tolerance,
                )?;
                let offset = offset_circle(circle, distance, tolerance)?;
                Ok(Self::Arc(
                    CircularArc3::try_from_circle_sweep(offset, arc.sweep_radians())?
                        .try_reparameterized(arc.domain())?,
                ))
            }
            Self::Ellipse(ellipse) => offset_ellipse(*ellipse, distance, tolerance),
            Self::NurbsCurve(curve) => {
                if let Some(polyline) = linear_nurbs_proxy(curve, tolerance)? {
                    return Self::Polyline(polyline).try_offset_with_corner_style(
                        distance,
                        plane_normal,
                        tolerance,
                        corner,
                    );
                }
                if corner == CurveOffsetCornerStyle::Chamfer {
                    return offset_nurbs_chamfer(curve, distance, plane_normal, tolerance);
                }
                Ok(Self::NurbsCurve(offset_nurbs(
                    curve,
                    distance,
                    plane_normal,
                    tolerance,
                )?))
            }
            Self::Polyline(polyline) => {
                offset_polyline(polyline, distance, plane_normal, tolerance, corner)
            }
            Self::PolyCurve(curve) => offset_proxy(curve, tolerance)?.try_offset_with_corner_style(
                distance,
                plane_normal,
                tolerance,
                corner,
            ),
        }
    }

    /// Return every connected offset piece. `None` leaves convex polyline and
    /// NURBS corner gaps open and trims supported concave NURBS junctions.
    pub fn try_offset_parts(
        &self,
        distance: Real,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
        corner: CurveOffsetCornerStyle,
    ) -> Result<Vec<Self>, GeometryError> {
        if corner == CurveOffsetCornerStyle::None {
            if !distance.is_finite() || distance == 0.0 {
                return Err(GeometryError::InvalidCurveOffsetDistance);
            }
            if let Self::Polyline(polyline) = self {
                return offset_polyline_open_gaps(polyline, distance, plane_normal, tolerance);
            }
            if let Self::NurbsCurve(curve) = self {
                if curve.is_closed()?
                    && let Some(polyline) = linear_nurbs_proxy(curve, tolerance)?
                {
                    return Self::Polyline(polyline).try_offset_parts(
                        distance,
                        plane_normal,
                        tolerance,
                        corner,
                    );
                }
                return offset_nurbs_open_gaps(curve, distance, plane_normal, tolerance);
            }
            if let Self::PolyCurve(curve) = self {
                return offset_proxy(curve, tolerance)?.try_offset_parts(
                    distance,
                    plane_normal,
                    tolerance,
                    corner,
                );
            }
            return Ok(vec![self.try_offset_with_corner_style(
                distance,
                plane_normal,
                tolerance,
                CurveOffsetCornerStyle::Sharp,
            )?]);
        }
        Ok(vec![self.try_offset_with_corner_style(
            distance,
            plane_normal,
            tolerance,
            corner,
        )?])
    }

    /// Find an offset that contains `through` within absolute model tolerance.
    /// Planar polylines try the nearest segment's perpendicular distance and
    /// the nearest corner's radius or chamfer construction when applicable.
    pub fn try_offset_through_point(
        &self,
        through: Point3,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
        corner: CurveOffsetCornerStyle,
    ) -> Result<(Real, Vec<Self>), GeometryError> {
        if let Self::NurbsCurve(curve) = self
            && (corner != CurveOffsetCornerStyle::None || curve.is_closed()?)
            && let Some(polyline) = linear_nurbs_proxy(curve, tolerance)?
        {
            return Self::Polyline(polyline).try_offset_through_point(
                through,
                plane_normal,
                tolerance,
                corner,
            );
        }
        if let Self::PolyCurve(curve) = self {
            return offset_proxy(curve, tolerance)?.try_offset_through_point(
                through,
                plane_normal,
                tolerance,
                corner,
            );
        }
        let (origin, normal, candidates) = match self {
            Self::Line(line) => {
                let direction = line.direction(tolerance)?;
                let left = plane_normal
                    .as_vector()
                    .cross(direction.as_vector())?
                    .normalized(tolerance)?;
                let offset_plane_normal = direction
                    .as_vector()
                    .cross(left.as_vector())?
                    .normalized(tolerance)?;
                (
                    line.start(),
                    offset_plane_normal,
                    vec![line.start().vector_to(through)?.dot(left.as_vector())?],
                )
            }
            Self::Circle(circle) => (
                circle.center(),
                circle.normal()?,
                vec![
                    circle.radius()
                        - in_plane_radius(
                            circle.center(),
                            through,
                            circle.x_axis(),
                            circle.y_axis(),
                        )?,
                ],
            ),
            Self::Arc(arc) => (
                arc.center(),
                arc.normal()?,
                vec![
                    arc.radius()
                        - in_plane_radius(arc.center(), through, arc.x_axis(), arc.y_axis())?,
                ],
            ),
            Self::Ellipse(ellipse) => (
                ellipse.center(),
                ellipse.normal()?,
                vec![ellipse_through_distance(*ellipse, through, tolerance)?],
            ),
            Self::NurbsCurve(curve) => (
                curve.evaluate(*curve.domain().start())?,
                nurbs::offset_plane(curve, plane_normal, tolerance)?,
                vec![nurbs_through_distance(
                    curve,
                    through,
                    plane_normal,
                    tolerance,
                )?],
            ),
            Self::Polyline(polyline) => {
                let normal = polyline_offset_normal(polyline, plane_normal, tolerance)?;
                let mut nearest = None;
                for segment in polyline.segments() {
                    let closest = segment.closest_point(through, tolerance)?;
                    let separation = closest.distance_to(through)?;
                    if nearest.is_none_or(|(best, _)| separation < best) {
                        let left = normal
                            .as_vector()
                            .cross(segment.direction(tolerance)?.as_vector())?;
                        let signed = segment.start().vector_to(through)?.dot(left)?;
                        nearest = Some((separation, signed));
                    }
                }
                let signed = nearest.expect("a polyline has segments").1;
                let mut candidates = vec![signed];
                if matches!(
                    corner,
                    CurveOffsetCornerStyle::Round | CurveOffsetCornerStyle::Chamfer
                ) {
                    let vertex_count = if polyline.is_closed() {
                        polyline.segment_count()
                    } else {
                        polyline.vertices().len()
                    };
                    let nearest_vertex =
                        (0..vertex_count).try_fold((Real::INFINITY, 0), |best, index| {
                            let distance = polyline.vertices()[index].distance_to(through)?;
                            Ok::<_, GeometryError>(if distance < best.0 {
                                (distance, index)
                            } else {
                                best
                            })
                        })?;
                    if corner == CurveOffsetCornerStyle::Round && signed != 0.0 {
                        candidates.push(signed.signum() * nearest_vertex.0);
                    } else if corner == CurveOffsetCornerStyle::Chamfer
                        && let Some(distance) = chamfer_distance_through_point(
                            polyline,
                            nearest_vertex.1,
                            through,
                            normal,
                            tolerance,
                        )?
                    {
                        candidates.push(distance);
                    }
                }
                (polyline.vertices()[0], normal, candidates)
            }
            _ => return Err(GeometryError::UnsupportedCurveOffset),
        };
        if origin.vector_to(through)?.dot(normal.as_vector())?.abs() > tolerance.absolute() {
            return Err(GeometryError::OffsetThroughPointOffPlane);
        }
        for distance in candidates {
            if !distance.is_finite() || distance.abs() <= tolerance.absolute() {
                continue;
            }
            if let Ok(parts) = self.try_offset_parts(distance, plane_normal, tolerance, corner)
                && parts.iter().any(|part| {
                    offset_curve_distance_to_point(part, through, tolerance)
                        .is_ok_and(|separation| separation <= tolerance.absolute())
                })
            {
                return Ok((distance, parts));
            }
        }
        Err(GeometryError::OffsetThroughPointNoSolution)
    }

    /// Returns which signed offset reaches `side` for this curve's oriented
    /// plane. A point on the supporting locus is ambiguous even if it lies
    /// beyond a finite line or arc endpoint.
    pub fn offset_side(
        &self,
        side: Point3,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        if let Self::NurbsCurve(curve) = self
            && curve.is_closed()?
            && let Some(polyline) = linear_nurbs_proxy(curve, tolerance)?
        {
            return Self::Polyline(polyline).offset_side(side, plane_normal, tolerance);
        }
        if let Self::PolyCurve(curve) = self {
            return offset_proxy(curve, tolerance)?.offset_side(side, plane_normal, tolerance);
        }
        let signed = match self {
            Self::Line(line) => {
                let left = plane_normal
                    .as_vector()
                    .cross(line.direction(tolerance)?.as_vector())?
                    .normalized(tolerance)?;
                line.start().vector_to(side)?.dot(left.as_vector())?
            }
            Self::Circle(circle) => {
                circle.radius()
                    - in_plane_radius(circle.center(), side, circle.x_axis(), circle.y_axis())?
            }
            Self::Arc(arc) => {
                arc.radius() - in_plane_radius(arc.center(), side, arc.x_axis(), arc.y_axis())?
            }
            Self::Ellipse(ellipse) => {
                return ellipse_offset_side(*ellipse, side, tolerance);
            }
            Self::NurbsCurve(curve) => {
                return nurbs_offset_side(curve, side, plane_normal, tolerance);
            }
            Self::Polyline(polyline) => {
                let normal = polyline_offset_normal(polyline, plane_normal, tolerance)?;
                let mut nearest = None;
                for segment in polyline.segments() {
                    let closest = segment.closest_point(side, tolerance)?;
                    let separation = closest.distance_to(side)?;
                    if nearest.is_none_or(|(best, _)| separation < best) {
                        let left = normal
                            .as_vector()
                            .cross(segment.direction(tolerance)?.as_vector())?;
                        let signed = segment.start().vector_to(side)?.dot(left)?;
                        nearest = Some((separation, signed));
                    }
                }
                nearest.expect("a polyline has segments").1
            }
            _ => return Err(GeometryError::UnsupportedCurveOffset),
        };
        if signed.abs() <= tolerance.absolute() {
            return Err(GeometryError::AmbiguousCurveOffsetSide);
        }
        Ok(signed.signum())
    }
}

#[derive(Clone, Copy)]
struct OffsetRegionEdge2 {
    a: [Real; 2],
    b: [Real; 2],
    min_x: Real,
    max_x: Real,
    min_y: Real,
    max_y: Real,
}

/// Sweep by the first projected coordinate and test only overlapping edge
/// boxes. Exact rational predicates resolve crossings and vertex contacts
/// that a rounded cross product could miss near collinearity.
fn validate_simple_offset_region(points: &[[Real; 2]]) -> Result<(), GeometryError> {
    let edges = points
        .windows(2)
        .map(|pair| OffsetRegionEdge2 {
            a: pair[0],
            b: pair[1],
            min_x: pair[0][0].min(pair[1][0]),
            max_x: pair[0][0].max(pair[1][0]),
            min_y: pair[0][1].min(pair[1][1]),
            max_y: pair[0][1].max(pair[1][1]),
        })
        .collect::<Vec<_>>();
    for index in 0..edges.len() {
        let previous = points[(index + edges.len() - 1) % edges.len()];
        let vertex = points[index];
        let next = points[index + 1];
        if offset_region_orient2(previous, vertex, next) == Ordering::Equal
            && !(previous[0].min(next[0]) <= vertex[0]
                && vertex[0] <= previous[0].max(next[0])
                && previous[1].min(next[1]) <= vertex[1]
                && vertex[1] <= previous[1].max(next[1]))
        {
            return Err(GeometryError::SelfIntersectingOffsetRegion);
        }
    }
    let mut order = (0..edges.len()).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| edges[a].min_x.total_cmp(&edges[b].min_x));
    let mut active: Vec<usize> = Vec::new();
    for index in order {
        let edge = edges[index];
        active.retain(|&other| edges[other].max_x >= edge.min_x);
        for &other in &active {
            if index.abs_diff(other) == 1 || index.abs_diff(other) + 1 == edges.len() {
                continue;
            }
            let candidate = edges[other];
            if candidate.max_y < edge.min_y || edge.max_y < candidate.min_y {
                continue;
            }
            let signs = [
                offset_region_orient2(edge.a, edge.b, candidate.a),
                offset_region_orient2(edge.a, edge.b, candidate.b),
                offset_region_orient2(candidate.a, candidate.b, edge.a),
                offset_region_orient2(candidate.a, candidate.b, edge.b),
            ];
            if (signs.iter().all(|sign| *sign != Ordering::Equal)
                && signs[0] != signs[1]
                && signs[2] != signs[3])
                || (signs[0] == Ordering::Equal && offset_region_on_segment(edge, candidate.a))
                || (signs[1] == Ordering::Equal && offset_region_on_segment(edge, candidate.b))
                || (signs[2] == Ordering::Equal && offset_region_on_segment(candidate, edge.a))
                || (signs[3] == Ordering::Equal && offset_region_on_segment(candidate, edge.b))
            {
                return Err(GeometryError::SelfIntersectingOffsetRegion);
            }
        }
        active.push(index);
    }
    Ok(())
}

fn offset_region_on_segment(edge: OffsetRegionEdge2, point: [Real; 2]) -> bool {
    edge.min_x <= point[0]
        && point[0] <= edge.max_x
        && edge.min_y <= point[1]
        && point[1] <= edge.max_y
}

fn offset_region_orient2(a: [Real; 2], b: [Real; 2], c: [Real; 2]) -> Ordering {
    let abx = b[0] - a[0];
    let aby = b[1] - a[1];
    let acx = c[0] - a[0];
    let acy = c[1] - a[1];
    let positive = abx * acy;
    let negative = aby * acx;
    let determinant = positive - negative;
    // Projected coordinates are scaled to unit size. Far from zero, this
    // conservative error bound avoids rational arithmetic at ordinary corners.
    let error = 64.0 * Real::EPSILON * (positive.abs() + negative.abs());
    if determinant.abs() > error {
        return determinant.total_cmp(&0.0);
    }
    let (ax, ay) = (rational(a[0]), rational(a[1]));
    let (bx, by) = (rational(b[0]), rational(b[1]));
    let (cx, cy) = (rational(c[0]), rational(c[1]));
    ((bx - &ax) * (cy - &ay) - (by - &ay) * (cx - &ax)).cmp(&rational(0.0))
}

fn offset_curve_distance_to_point(
    curve: &Curve3,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    match curve {
        Curve3::Line(line) => line.closest_point(point, tolerance)?.distance_to(point),
        Curve3::Circle(circle) => {
            let radial = in_plane_radius(circle.center(), point, circle.x_axis(), circle.y_axis())?;
            let height = circle
                .center()
                .vector_to(point)?
                .dot(circle.normal()?.as_vector())?;
            Ok((radial - circle.radius()).hypot(height))
        }
        Curve3::Arc(arc) => offset_arc_distance_to_point(*arc, point),
        Curve3::NurbsCurve(curve) => curve
            .evaluate(curve.closest_parameter(point, tolerance)?)?
            .distance_to(point),
        Curve3::Polyline(polyline) => polyline.closest_point(point, tolerance)?.distance_to(point),
        Curve3::PolyCurve(polycurve) => {
            polycurve
                .segments()
                .iter()
                .try_fold(Real::INFINITY, |best, segment| {
                    let distance = match segment {
                        CurveSegment3::Line(line) => {
                            line.closest_point(point, tolerance)?.distance_to(point)?
                        }
                        CurveSegment3::Arc(arc) => offset_arc_distance_to_point(*arc, point)?,
                        CurveSegment3::Polyline(polyline) => polyline
                            .closest_point(point, tolerance)?
                            .distance_to(point)?,
                        CurveSegment3::NurbsCurve(curve) => curve
                            .evaluate(curve.closest_parameter(point, tolerance)?)?
                            .distance_to(point)?,
                    };
                    Ok(best.min(distance))
                })
        }
        _ => Err(GeometryError::UnsupportedCurveOffset),
    }
}

fn offset_arc_distance_to_point(arc: CircularArc3, point: Point3) -> Result<Real, GeometryError> {
    let radial = arc.center().vector_to(point)?;
    let mut angle = radial
        .dot(arc.y_axis().as_vector())?
        .atan2(radial.dot(arc.x_axis().as_vector())?);
    if angle < 0.0 {
        angle += std::f64::consts::TAU;
    }
    if angle <= arc.sweep_radians() {
        arc.point_at(angle / arc.sweep_radians())?
            .distance_to(point)
    } else {
        Ok(arc
            .point_at(0.0)?
            .distance_to(point)?
            .min(arc.point_at(1.0)?.distance_to(point)?))
    }
}

fn chamfer_distance_through_point(
    polyline: &Polyline3,
    vertex_index: usize,
    through: Point3,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Option<Real>, GeometryError> {
    let segment_count = polyline.segment_count();
    if !polyline.is_closed() && (vertex_index == 0 || vertex_index == segment_count) {
        return Ok(None);
    }
    let previous = if vertex_index == 0 {
        segment_count - 1
    } else {
        vertex_index - 1
    };
    let vertices = polyline.vertices();
    let left_before = normal.as_vector().cross(
        vertices[previous]
            .direction_to(vertices[previous + 1])?
            .as_vector(),
    )?;
    let left_after = normal.as_vector().cross(
        vertices[vertex_index]
            .direction_to(vertices[vertex_index + 1])?
            .as_vector(),
    )?;
    let determinant = left_before.cross(left_after)?.dot(normal.as_vector())?;
    if determinant.abs() <= tolerance.angular() {
        return Ok(None);
    }
    let radial = vertices[vertex_index].vector_to(through)?;
    let first = radial.cross(left_after)?.dot(normal.as_vector())? / determinant;
    let second = left_before.cross(radial)?.dot(normal.as_vector())? / determinant;
    if first * second < 0.0 {
        return Ok(None);
    }
    Ok(Some(first + second))
}

fn polyline_offset_normal(
    polyline: &Polyline3,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<UnitVector3, GeometryError> {
    let origin = polyline.vertices()[0];
    let first = polyline.vertices()[0].direction_to(polyline.vertices()[1])?;
    let inferred = polyline.vertices().iter().skip(2).find_map(|point| {
        let direction = origin.direction_to(*point).ok()?;
        let cross = first.as_vector().cross(direction.as_vector()).ok()?;
        (cross.length().ok()? > tolerance.angular())
            .then(|| cross.normalized_nonzero().ok())
            .flatten()
    });
    let normal = if let Some(inferred) = inferred {
        if inferred.as_vector().dot(fallback.as_vector())? < 0.0 {
            inferred.opposite()
        } else {
            inferred
        }
    } else {
        fallback
    };
    if first.as_vector().dot(normal.as_vector())?.abs() > tolerance.angular() {
        return Err(GeometryError::NonPlanarPolyline);
    }
    let largest_radius = polyline
        .vertices()
        .iter()
        .try_fold(0.0_f64, |maximum, point| {
            Ok::<_, GeometryError>(maximum.max(origin.distance_to(*point)?))
        })?;
    let permitted = tolerance
        .absolute()
        .max(tolerance.relative() * largest_radius);
    for point in polyline.vertices() {
        if origin.vector_to(*point)?.dot(normal.as_vector())?.abs() > permitted {
            return Err(GeometryError::NonPlanarPolyline);
        }
    }
    Ok(normal)
}

fn offset_polyline(
    polyline: &Polyline3,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
    corner_style: CurveOffsetCornerStyle,
) -> Result<Curve3, GeometryError> {
    let layout = polyline_offset_layout(polyline, distance, fallback, tolerance, corner_style)?;
    let normal = layout.normal;
    let corners = &layout.corners;
    let vertices = polyline.vertices();
    let closed = polyline.is_closed();
    if corner_style == CurveOffsetCornerStyle::Round && corners.iter().any(|(a, b)| a != b) {
        return Ok(Curve3::PolyCurve(round_offset_polyline(
            polyline, corners, normal, tolerance,
        )?));
    }
    let mut result = Vec::with_capacity(vertices.len() + corners.len());
    for &(incoming, outgoing) in corners {
        result.push(incoming);
        if incoming != outgoing {
            result.push(outgoing);
        }
    }
    if closed {
        result.push(result[0]);
    }
    let parameters = if result.len() == vertices.len() {
        polyline.parameters().to_vec()
    } else {
        let source = polyline.parameters();
        let start = source[0];
        let end = source[source.len() - 1];
        let span = end - start;
        let last = result.len() - 1;
        (0..=last)
            .map(|index| {
                if index == last {
                    end
                } else {
                    start + span * (index as Real / last as Real)
                }
            })
            .collect()
    };
    Ok(Curve3::Polyline(Polyline3::try_with_parameters(
        result, parameters, tolerance,
    )?))
}

struct OffsetPolylineLayout {
    normal: UnitVector3,
    corners: Vec<(Point3, Point3)>,
}

fn polyline_offset_layout(
    polyline: &Polyline3,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
    corner_style: CurveOffsetCornerStyle,
) -> Result<OffsetPolylineLayout, GeometryError> {
    let normal = polyline_offset_normal(polyline, fallback, tolerance)?;
    let vertices = polyline.vertices();
    let closed = polyline.is_closed();
    let segment_count = polyline.segment_count();
    let directions = polyline
        .segments()
        .map(|segment| segment.direction(tolerance).map(UnitVector3::as_vector))
        .collect::<Result<Vec<_>, _>>()?;
    let lefts = directions
        .iter()
        .map(|direction| normal.as_vector().cross(*direction))
        .collect::<Result<Vec<_>, _>>()?;
    let mut corners = Vec::with_capacity(vertices.len());
    let unique_count = if closed {
        segment_count
    } else {
        vertices.len()
    };
    for index in 0..unique_count {
        let corner = if !closed && index == 0 {
            let point = vertices[0].translated(lefts[0].scaled(distance)?)?;
            (point, point)
        } else if !closed && index == segment_count {
            let point = vertices[index].translated(lefts[segment_count - 1].scaled(distance)?)?;
            (point, point)
        } else {
            let previous = if index == 0 {
                segment_count - 1
            } else {
                index - 1
            };
            let turn = directions[previous]
                .cross(directions[index])?
                .dot(normal.as_vector())?;
            if corner_style != CurveOffsetCornerStyle::Sharp
                && turn * distance < -tolerance.angular() * distance.abs()
            {
                let pair = (
                    vertices[index].translated(lefts[previous].scaled(distance)?)?,
                    vertices[index].translated(lefts[index].scaled(distance)?)?,
                );
                if pair.0 == pair.1 {
                    return Err(GeometryError::Degenerate {
                        context: "offset corner gap under model precision",
                    });
                }
                pair
            } else {
                let point = sharp_offset_corner(
                    vertices[index],
                    directions[previous],
                    directions[index],
                    lefts[previous],
                    lefts[index],
                    normal,
                    distance,
                    tolerance,
                )?;
                (point, point)
            }
        };
        if corner.0 == vertices[index] || corner.1 == vertices[index] {
            return Err(GeometryError::Degenerate {
                context: "offset polyline under model precision",
            });
        }
        corners.push(corner);
    }
    for index in 0..segment_count {
        let start = corners[index].1;
        let end = corners[(index + 1) % corners.len()].0;
        let advance = vertices[index]
            .vector_to(vertices[index + 1])?
            .dot(start.vector_to(end)?)?;
        if advance <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "collapsed offset polyline segment",
            });
        }
    }
    Ok(OffsetPolylineLayout { normal, corners })
}

fn offset_polyline_open_gaps(
    polyline: &Polyline3,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    let layout = polyline_offset_layout(
        polyline,
        distance,
        fallback,
        tolerance,
        CurveOffsetCornerStyle::None,
    )?;
    let corners = &layout.corners;
    let gaps = corners
        .iter()
        .filter(|(incoming, outgoing)| incoming != outgoing)
        .count();
    if gaps == 0 {
        let mut vertices = corners.iter().map(|corner| corner.0).collect::<Vec<_>>();
        if polyline.is_closed() {
            vertices.push(vertices[0]);
        }
        return Ok(vec![Curve3::Polyline(Polyline3::try_with_parameters(
            vertices,
            polyline.parameters().to_vec(),
            tolerance,
        )?)]);
    }

    let mut pieces = Vec::with_capacity(gaps + usize::from(!polyline.is_closed()));
    if polyline.is_closed() {
        let first_gap = corners
            .iter()
            .position(|(a, b)| a != b)
            .expect("a gap exists");
        let mut current = vec![corners[first_gap].1];
        for step in 0..polyline.segment_count() {
            let next = (first_gap + step + 1) % corners.len();
            current.push(corners[next].0);
            if corners[next].0 != corners[next].1 {
                pieces.push(offset_polyline_piece(
                    std::mem::take(&mut current),
                    tolerance,
                )?);
                if step + 1 < polyline.segment_count() {
                    current.push(corners[next].1);
                }
            }
        }
    } else {
        let mut current = vec![corners[0].1];
        for index in 0..polyline.segment_count() {
            let next = index + 1;
            current.push(corners[next].0);
            if corners[next].0 != corners[next].1 {
                pieces.push(offset_polyline_piece(
                    std::mem::take(&mut current),
                    tolerance,
                )?);
                current.push(corners[next].1);
            }
        }
        pieces.push(offset_polyline_piece(current, tolerance)?);
    }
    Ok(pieces)
}

fn offset_polyline_piece(
    vertices: Vec<Point3>,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    if vertices.len() == 2 {
        Ok(Curve3::Line(LineSegment::try_new(
            vertices[0],
            vertices[1],
            tolerance,
        )?))
    } else {
        Ok(Curve3::Polyline(Polyline3::try_new(vertices, tolerance)?))
    }
}

fn round_offset_polyline(
    source: &Polyline3,
    corners: &[(Point3, Point3)],
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let arc_count = corners.iter().filter(|(start, end)| start != end).count();
    let segment_count = source.segment_count().saturating_add(arc_count);
    if segment_count > MAX_POLYCURVE_SEGMENTS {
        return Err(GeometryError::InvalidPolyCurve {
            context: "segment count is outside the supported range",
        });
    }
    let mut segments = Vec::<CurveSegment3>::with_capacity(segment_count);
    for index in 0..source.segment_count() {
        let next = (index + 1) % corners.len();
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            corners[index].1,
            corners[next].0,
            tolerance,
        )?));
        let (start, end) = corners[next];
        if start == end {
            continue;
        }
        let center = source.vertices()[next];
        let start_radial = center.vector_to(start)?.normalized_nonzero()?;
        let end_radial = center.vector_to(end)?.normalized_nonzero()?;
        let sine = start_radial
            .as_vector()
            .cross(end_radial.as_vector())?
            .dot(normal.as_vector())?;
        let cosine = start_radial.as_vector().dot(end_radial.as_vector())?;
        let sweep = sine.abs().atan2(cosine);
        let arc_normal = if sine < 0.0 {
            normal.opposite()
        } else {
            normal
        };
        let circle = Circle3::try_from_center_point(center, start, arc_normal, tolerance)?;
        segments.push(CurveSegment3::Arc(CircularArc3::try_from_circle_sweep(
            circle, sweep,
        )?));
    }
    PolyCurve3::try_new(segments)?.try_reparameterized(source.domain())
}

#[allow(clippy::too_many_arguments)]
fn sharp_offset_corner(
    vertex: Point3,
    previous: Vector3,
    next: Vector3,
    previous_left: Vector3,
    next_left: Vector3,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Point3, GeometryError> {
    let sine = previous.cross(next)?.dot(normal.as_vector())?;
    if sine.abs() <= tolerance.angular() {
        if previous.dot(next)? <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "reversing offset polyline corner",
            });
        }
        return vertex.translated(previous_left.scaled(distance)?);
    }
    let first = vertex.translated(previous_left.scaled(distance)?)?;
    let second = vertex.translated(next_left.scaled(distance)?)?;
    let gap = first.vector_to(second)?;
    let along = gap.cross(next)?.dot(normal.as_vector())? / sine;
    first.translated(previous.scaled(along)?)
}

fn in_plane_radius(
    center: Point3,
    point: Point3,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
) -> Result<Real, GeometryError> {
    let radial = center.vector_to(point)?;
    Ok(radial
        .dot(x_axis.as_vector())?
        .hypot(radial.dot(y_axis.as_vector())?))
}

fn offset_circle(
    circle: Circle3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Circle3, GeometryError> {
    let radius = circle.radius() - distance;
    if !radius.is_finite() || radius <= tolerance.absolute() || radius == circle.radius() {
        return Err(GeometryError::Degenerate {
            context: "offset circle radius",
        });
    }
    Circle3::try_from_frame(
        circle.center(),
        radius,
        circle.x_axis(),
        circle.normal()?,
        tolerance,
    )?
    .try_reparameterized(circle.domain())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ControlPointCurveClosure, CurveSegment3, LineSegment, NurbsCurve, Point3, PolyCurve3,
        Vector3,
    };

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn open_nurbs_offset_follows_parabola_normal() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(4.0, 2.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap(),
        );
        assert_eq!(
            source.offset_side(point(0.0, 1.0, 0.0), normal, tol),
            Ok(1.0)
        );
        let Curve3::NurbsCurve(offset) = source.try_offset(0.35, normal, tol).unwrap() else {
            panic!("NURBS offset")
        };
        assert_eq!(offset.domain(), 0.0..=1.0);
        for index in 0..=128 {
            let t = index as Real / 128.0;
            let speed = 1.0_f64.hypot(t);
            let expected = point(4.0 * t - 0.35 * t / speed, 2.0 * t * t + 0.35 / speed, 0.0);
            assert!(offset.evaluate(t).unwrap().distance_to(expected).unwrap() <= tol.absolute());
        }
        let (_, through_parts) = source
            .try_offset_through_point(
                point(0.0, 0.35, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        let Curve3::NurbsCurve(through) = &through_parts[0] else {
            panic!("NURBS through-point offset")
        };
        assert!(
            through
                .evaluate(0.0)
                .unwrap()
                .distance_to(point(0.0, 0.35, 0.0))
                .unwrap()
                <= tol.absolute()
        );
    }

    #[test]
    fn closed_rational_nurbs_circle_offset_stays_closed() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle = Circle3::try_new(point(1.0, 2.0, 0.0), 5.0, normal, tol).unwrap();
        let source = Curve3::NurbsCurve(circle.to_nurbs().unwrap());
        let Curve3::NurbsCurve(offset) = source.try_offset(-0.8, normal, tol).unwrap() else {
            panic!("closed NURBS offset")
        };
        assert!(offset.is_closed().unwrap());
        for index in 0..=128 {
            let t = *offset.domain().start()
                + (*offset.domain().end() - *offset.domain().start()) * index as Real / 128.0;
            let radius = offset
                .evaluate(t)
                .unwrap()
                .distance_to(circle.center())
                .unwrap();
            assert!((radius - 5.8).abs() <= tol.absolute());
        }
    }

    #[test]
    fn closed_nurbs_regions_classify_orientation_containment_and_crossing() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle = Circle3::try_new(point(1.0, 2.0, 0.0), 5.0, normal, tol).unwrap();
        let curve = circle.to_nurbs().unwrap();
        let forward = Curve3::NurbsCurve(curve.clone());
        let reverse = Curve3::NurbsCurve(curve.reversed().unwrap());
        assert_eq!(
            forward.offset_region_inward_sign(normal, tol),
            Ok(Some(1.0))
        );
        assert_eq!(
            reverse.offset_region_inward_sign(normal, tol),
            Ok(Some(-1.0))
        );
        for region in [&forward, &reverse] {
            assert_eq!(
                region.offset_region_contains(point(1.0, 2.0, 0.0), normal, tol),
                Ok(Some(true))
            );
            assert_eq!(
                region.offset_region_contains(point(7.0, 2.0, 0.0), normal, tol),
                Ok(Some(false))
            );
            assert_eq!(
                region.offset_region_contains(point(6.0, 2.0, 0.0), normal, tol),
                Err(GeometryError::AmbiguousCurveOffsetSide)
            );
        }
        let bow_tie = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 2.0, 0.0),
                    point(0.0, 2.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
            )
            .unwrap(),
        );
        assert_eq!(
            bow_tie.offset_region_inward_sign(normal, tol),
            Err(GeometryError::SelfIntersectingOffsetRegion)
        );
    }

    #[test]
    fn concave_periodic_nurbs_region_uses_exact_trim_containment() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            3,
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 1.0, 0.0),
                point(1.5, 1.5, 0.0),
                point(1.5, 3.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(-1.0, 2.0, 0.0),
            ],
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        let region = Curve3::NurbsCurve(curve);
        assert_eq!(region.offset_region_inward_sign(normal, tol), Ok(Some(1.0)));
        assert_eq!(
            region.offset_region_contains(point(0.5, 2.0, 0.0), normal, tol),
            Ok(Some(true))
        );
        assert_eq!(
            region.offset_region_contains(point(3.0, 2.0, 0.0), normal, tol),
            Ok(Some(false))
        );
        assert_eq!(
            region.offset_region_contains(point(0.5, 2.0, 0.1), normal, tol),
            Ok(Some(false))
        );
        let Curve3::NurbsCurve(offset) = region.try_offset(-0.05, normal, tol).unwrap() else {
            panic!("concave periodic NURBS offset")
        };
        assert!(offset.is_closed().unwrap());
    }

    #[test]
    fn closed_nurbs_region_sweep_handles_hundreds_of_spans() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let count = 300;
        let mut controls = (0..count)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as Real / count as Real;
                point(10.0 * angle.cos(), 10.0 * angle.sin(), 0.0)
            })
            .collect::<Vec<_>>();
        controls.push(controls[0]);
        let mut knots = vec![0.0, 0.0];
        knots.extend((1..count).map(|k| k as Real));
        knots.extend([count as Real, count as Real]);
        let region = Curve3::NurbsCurve(NurbsCurve::try_new(1, controls, knots).unwrap());
        assert_eq!(region.offset_region_inward_sign(normal, tol), Ok(Some(1.0)));
    }

    #[test]
    fn nurbs_offset_rejects_nonplanar_source_and_joins_linear_kink() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let nonplanar = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(2.0, 1.0, 0.0),
                    point(3.0, 1.0, 1.0),
                ],
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap(),
        );
        assert_eq!(
            nonplanar.try_offset(0.2, normal, tol),
            Err(GeometryError::NonPlanarCurveOffset)
        );
        let kink = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(1.0, 1.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let Curve3::Polyline(offset) = kink.try_offset(0.2, normal, tol).unwrap() else {
            panic!("sharp linear NURBS corner")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(0.0, 0.2, 0.0),
                point(0.8, 0.2, 0.0),
                point(0.8, 1.0, 0.0)
            ]
        );
        assert!(matches!(
            kink.try_offset_with_corner_style(-0.2, normal, tol, CurveOffsetCornerStyle::Round),
            Ok(Curve3::PolyCurve(_))
        ));
        assert!(matches!(
            kink.try_offset_with_corner_style(-0.2, normal, tol, CurveOffsetCornerStyle::Chamfer),
            Ok(Curve3::Polyline(_))
        ));
    }

    #[test]
    fn convex_nurbs_kink_none_keeps_separate_offset_pieces() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(1.0, 1.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let parts = source
            .try_offset_parts(-0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 2);
        let [Curve3::NurbsCurve(first), Curve3::NurbsCurve(second)] = parts.as_slice() else {
            panic!("separate smooth pieces")
        };
        assert_eq!(first.domain(), 0.0..=1.0);
        assert_eq!(second.domain(), 1.0..=2.0);
        assert!(
            first
                .evaluate(1.0)
                .unwrap()
                .distance_to(point(1.0, -0.2, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        assert!(
            second
                .evaluate(1.0)
                .unwrap()
                .distance_to(point(1.2, 0.0, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        assert_eq!(
            source.try_offset_with_corner_style(-0.2, normal, tol, CurveOffsetCornerStyle::None),
            Err(GeometryError::DisconnectedCurveOffset)
        );
        let joined = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(joined)] = joined.as_slice() else {
            panic!("concave NURBS trim")
        };
        assert_eq!(joined.domain(), 0.0..=2.0);
        assert_eq!(joined.segments().len(), 2);
        let first_end = joined.segments()[0]
            .evaluate(*joined.segments()[0].domain().end())
            .unwrap();
        let second_start = joined.segments()[1]
            .evaluate(*joined.segments()[1].domain().start())
            .unwrap();
        assert!(first_end.distance_to(point(0.8, 0.2, 0.0)).unwrap() <= tol.absolute());
        assert!(first_end.distance_to(second_start).unwrap() <= tol.absolute());
    }

    #[test]
    fn curved_nurbs_chamfer_bridges_convex_gap() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, -0.5, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(2.5, 1.0, 0.0),
                    point(2.0, 2.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let parts = source
            .try_offset_parts(-0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 2);
        let Curve3::PolyCurve(joined) = source
            .try_offset_with_corner_style(-0.2, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("curved NURBS chamfer")
        };
        assert_eq!(joined.domain(), 0.0..=2.0);
        assert_eq!(joined.segments().len(), 3);
        let CurveSegment3::Line(bridge) = &joined.segments()[1] else {
            panic!("straight corner bridge")
        };
        assert!(
            bridge
                .start()
                .distance_to(parts[0].as_ref().end_point().unwrap())
                .unwrap()
                <= tol.absolute()
        );
        assert!(
            bridge
                .end()
                .distance_to(parts[1].as_ref().start_point().unwrap())
                .unwrap()
                <= tol.absolute()
        );
    }

    #[test]
    fn curved_polycurve_kink_none_preserves_convex_gap() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0), tol).unwrap();
        let circle =
            Circle3::try_from_center_point(point(1.0, 0.0, 0.0), point(2.0, 0.0, 0.0), normal, tol)
                .unwrap();
        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
        let source = Curve3::PolyCurve(
            PolyCurve3::try_new(vec![CurveSegment3::Line(line), CurveSegment3::Arc(arc)]).unwrap(),
        );
        let parts = source
            .try_offset_parts(-0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 2);
        let Curve3::PolyCurve(chamfered) = source
            .try_offset_with_corner_style(-0.2, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("mixed polycurve chamfer")
        };
        assert_eq!(chamfered.segments().len(), 3);
        assert!(matches!(chamfered.segments()[1], CurveSegment3::Line(_)));
        let [Curve3::NurbsCurve(first), Curve3::NurbsCurve(second)] = parts.as_slice() else {
            panic!("mixed polycurve pieces")
        };
        assert!(
            first
                .evaluate(*first.domain().end())
                .unwrap()
                .distance_to(point(2.0, -0.2, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        assert!(
            second
                .evaluate(*second.domain().start())
                .unwrap()
                .distance_to(point(2.2, 0.0, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        let joined = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(joined)] = joined.as_slice() else {
            panic!("curved concave trim")
        };
        assert_eq!(joined.segments().len(), 2);
        let first_end = joined.segments()[0]
            .evaluate(*joined.segments()[0].domain().end())
            .unwrap();
        let expected_x = 1.0 + (0.8_f64.powi(2) - 0.2_f64.powi(2)).sqrt();
        assert!(first_end.distance_to(point(expected_x, 0.2, 0.0)).unwrap() <= tol.absolute());
    }

    #[test]
    fn nurbs_none_combines_concave_joins_and_separates_convex_gaps() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(2.0, 2.0, 0.0),
                    point(4.0, 2.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
            )
            .unwrap(),
        );
        let outputs = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(outputs.len(), 2);
        let [Curve3::PolyCurve(joined), Curve3::NurbsCurve(last)] = outputs.as_slice() else {
            panic!("joined and open pieces")
        };
        assert_eq!(joined.domain(), 0.0..=2.0);
        assert_eq!(joined.segments().len(), 2);
        assert_eq!(last.domain(), 2.0..=3.0);
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(2.0, 2.0, 0.0),
                    point(0.0, 2.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
            )
            .unwrap(),
        );
        let outputs = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(joined)] = outputs.as_slice() else {
            panic!("two concave joins")
        };
        assert_eq!(joined.segments().len(), 3);
        let middle = &joined.segments()[1];
        assert!(
            middle
                .evaluate(*middle.domain().start())
                .unwrap()
                .distance_to(point(1.8, 0.2, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        assert!(
            middle
                .evaluate(*middle.domain().end())
                .unwrap()
                .distance_to(point(1.8, 1.8, 0.0))
                .unwrap()
                <= tol.absolute()
        );
    }

    #[test]
    fn quadratic_c0_knot_trims_without_full_order_split() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(2.0, 1.0, 0.0),
                    point(2.0, 2.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        let output = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(joined)] = output.as_slice() else {
            panic!("quadratic C0 trim")
        };
        assert_eq!(joined.segments().len(), 2);
        let first_end = joined.segments()[0]
            .evaluate(*joined.segments()[0].domain().end())
            .unwrap();
        assert!(first_end.distance_to(point(1.8, 0.2, 0.0)).unwrap() <= tol.absolute());
    }

    #[test]
    fn closed_linear_nurbs_uses_exact_polyline_corner_styles() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let polygon = Polyline3::try_with_parameters(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            vec![10.0, 12.0, 14.0, 16.0, 20.0],
            tol,
        )
        .unwrap();
        let source = Curve3::NurbsCurve(polygon.to_native_nurbs().unwrap());
        assert_eq!(source.offset_region_inward_sign(normal, tol), Ok(Some(1.0)));
        assert_eq!(
            source.offset_region_contains(point(2.0, 2.0, 0.0), normal, tol),
            Ok(Some(true))
        );
        let Curve3::Polyline(inner) = source.try_offset(0.5, normal, tol).unwrap() else {
            panic!("closed linear NURBS sharp offset")
        };
        assert!(inner.is_closed());
        assert_eq!(inner.domain(), 10.0..=20.0);
        assert_eq!(inner.vertices()[0], point(0.5, 0.5, 0.0));
        assert!(
            matches!(source.try_offset_parts(0.5, normal, tol, CurveOffsetCornerStyle::None).unwrap().as_slice(), [Curve3::Polyline(polyline)] if polyline.is_closed())
        );
        assert!(matches!(
            source.try_offset_with_corner_style(-0.5, normal, tol, CurveOffsetCornerStyle::Round),
            Ok(Curve3::PolyCurve(_))
        ));
        assert_eq!(
            source
                .try_offset_parts(-0.5, normal, tol, CurveOffsetCornerStyle::None)
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn closed_quadratic_cornered_nurbs_none_joins_inward_and_opens_outward() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 2.0, 0.0),
                    point(4.0, 4.0, 0.0),
                    point(2.0, 4.0, 0.0),
                    point(0.0, 4.0, 0.0),
                    point(0.0, 2.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
            )
            .unwrap(),
        );
        assert_eq!(source.offset_region_inward_sign(normal, tol), Ok(Some(1.0)));
        let inward = source
            .try_offset_parts(0.5, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(inner)] = inward.as_slice() else {
            panic!("closed inward trim")
        };
        assert!(inner.is_closed().unwrap());
        assert_eq!(inner.segments().len(), 4);
        assert_eq!(inner.domain(), 0.0..=4.0);
        let outward = source
            .try_offset_parts(-0.5, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(outward.len(), 4);
        assert!(
            outward
                .iter()
                .all(|piece| matches!(piece, Curve3::NurbsCurve(_)))
        );
        let Curve3::NurbsCurve(first) = &outward[0] else {
            unreachable!()
        };
        assert!(
            first
                .evaluate(*first.domain().start())
                .unwrap()
                .distance_to(point(0.0, -0.5, 0.0))
                .unwrap()
                <= tol.absolute()
        );
        let Curve3::NurbsCurve(square) = source else {
            unreachable!()
        };
        let relocated =
            Curve3::NurbsCurve(square.try_split_at_parameters(&[0.5]).unwrap().remove(0));
        let relocated_inward = relocated
            .try_offset_parts(0.5, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(relocated_inner)] = relocated_inward.as_slice() else {
            panic!("smooth seam inward trim")
        };
        assert!(relocated_inner.is_closed().unwrap());
        assert_eq!(relocated_inner.segments().len(), 4);
        assert_eq!(relocated_inner.domain(), 1.0..=5.0);
        assert_eq!(
            relocated
                .try_offset_parts(-0.5, normal, tol, CurveOffsetCornerStyle::None)
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn closed_quadratic_mixed_corners_join_across_cycle() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 0.5, 0.0),
                    point(4.0, 1.0, 0.0),
                    point(2.5, 1.0, 0.0),
                    point(1.0, 1.0, 0.0),
                    point(1.0, 2.5, 0.0),
                    point(1.0, 4.0, 0.0),
                    point(0.5, 4.0, 0.0),
                    point(0.0, 4.0, 0.0),
                    point(0.0, 2.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![
                    0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0, 6.0,
                ],
            )
            .unwrap(),
        );
        let inward = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(inner)] = inward.as_slice() else {
            panic!("one open inward group")
        };
        assert_eq!(inner.segments().len(), 6);
        assert!(!inner.is_closed().unwrap());
        let outward = source
            .try_offset_parts(-0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(outward.len(), 5);
        assert_eq!(
            outward
                .iter()
                .filter(|piece| matches!(piece, Curve3::PolyCurve(_)))
                .count(),
            1
        );
    }

    #[test]
    fn closed_curved_quadratic_nurbs_none_trims_all_corners() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(2.0, -0.5, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.5, 2.0, 0.0),
                    point(4.0, 4.0, 0.0),
                    point(2.0, 4.5, 0.0),
                    point(0.0, 4.0, 0.0),
                    point(-0.5, 2.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
            )
            .unwrap(),
        );
        let inward = source
            .try_offset_parts(0.4, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::PolyCurve(inner)] = inward.as_slice() else {
            panic!("closed curved inward trim")
        };
        assert!(inner.is_closed().unwrap());
        assert_eq!(inner.segments().len(), 4);
        assert_eq!(
            source
                .try_offset_parts(-0.4, normal, tol, CurveOffsetCornerStyle::None)
                .unwrap()
                .len(),
            4
        );
        let Curve3::PolyCurve(chamfered) = source
            .try_offset_with_corner_style(-0.4, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("closed curved chamfer")
        };
        assert!(chamfered.is_closed().unwrap());
        assert_eq!(chamfered.segments().len(), 8);
        assert_eq!(chamfered.domain(), 0.0..=4.0);
    }

    #[test]
    fn closed_cubic_with_one_concave_seam_trims_its_own_offset() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                3,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(0.0, 4.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap(),
        );
        let inward = source
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::NurbsCurve(inner)] = inward.as_slice() else {
            panic!("one closed trimmed offset")
        };
        assert!(inner.is_closed().unwrap());
        assert!(*inner.domain().start() > 0.0);
        assert!(*inner.domain().end() < 1.0);
        let outward = source
            .try_offset_parts(-0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::NurbsCurve(outer)] = outward.as_slice() else {
            panic!("one open outward offset")
        };
        assert!(!outer.is_closed().unwrap());
        let Curve3::NurbsCurve(loop_curve) = source else {
            unreachable!()
        };
        let relocated = Curve3::NurbsCurve(
            loop_curve
                .try_split_at_parameters(&[0.5])
                .unwrap()
                .remove(0),
        );
        let relocated_inward = relocated
            .try_offset_parts(0.2, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        let [Curve3::NurbsCurve(relocated_inner)] = relocated_inward.as_slice() else {
            panic!("one trimmed offset with a smooth seam")
        };
        assert!(relocated_inner.is_closed().unwrap());
        assert!(*relocated_inner.domain().start() > 1.0);
        assert!(*relocated_inner.domain().end() < 2.0);
    }

    #[test]
    fn two_span_closed_nurbs_region_rejects_extra_crossing() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let crossed = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(0.0, 2.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    point(2.0, 2.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        );
        assert_eq!(
            crossed.offset_region_inward_sign(normal, tol),
            Err(GeometryError::SelfIntersectingOffsetRegion)
        );
    }

    #[test]
    fn straight_tilted_nurbs_uses_construction_normal_projection() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::NurbsCurve(
            NurbsCurve::try_new(
                1,
                vec![point(0.0, 0.0, 0.0), point(1.0, 0.0, 1.0)],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap(),
        );
        let Curve3::NurbsCurve(offset) = source.try_offset(0.5, normal, tol).unwrap() else {
            panic!("tilted straight NURBS offset")
        };
        for index in 0..=16 {
            let t = index as Real / 16.0;
            assert!(
                offset
                    .evaluate(t)
                    .unwrap()
                    .distance_to(point(t, 0.5, t))
                    .unwrap()
                    <= tol.absolute()
            );
        }
    }

    #[test]
    fn linear_polycurve_offsets_use_polyline_corner_rules_and_outer_domain() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let first = LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0), tol).unwrap();
        let second = LineSegment::try_new(point(4.0, 0.0, 0.0), point(4.0, 4.0, 0.0), tol).unwrap();
        let source = Curve3::PolyCurve(
            PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::Line(first), CurveSegment3::Line(second)],
                vec![10.0, 12.0, 20.0],
            )
            .unwrap(),
        );
        let Curve3::Polyline(offset) = source.try_offset(1.0, normal, tol).unwrap() else {
            panic!("linear polycurve offset")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 4.0, 0.0)
            ]
        );
        assert_eq!(offset.domain(), 10.0..=20.0);
        let parts = source
            .try_offset_parts(-1.0, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 2);
        assert!(matches!(
            source.try_offset_with_corner_style(-1.0, normal, tol, CurveOffsetCornerStyle::Round),
            Ok(Curve3::PolyCurve(_))
        ));
        assert!(matches!(
            source.try_offset_with_corner_style(-1.0, normal, tol, CurveOffsetCornerStyle::None),
            Err(GeometryError::DisconnectedCurveOffset)
        ));
    }

    #[test]
    fn linear_polycurve_proxy_maps_leaf_vertex_parameters() {
        let tol = Tolerance::DEFAULT;
        let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0), tol).unwrap();
        let leaf = Polyline3::try_with_parameters(
            vec![
                point(4.0, 0.0, 0.0),
                point(6.0, 0.0, 0.0),
                point(6.0, 2.0, 0.0),
            ],
            vec![5.0, 7.0, 11.0],
            tol,
        )
        .unwrap();
        let curve = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::Line(line), CurveSegment3::Polyline(leaf)],
            vec![10.0, 14.0, 26.0],
        )
        .unwrap();
        let Curve3::Polyline(proxy) = offset_proxy(&curve, tol).unwrap() else {
            panic!("linear proxy")
        };
        assert_eq!(
            proxy.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(6.0, 0.0, 0.0),
                point(6.0, 2.0, 0.0)
            ]
        );
        assert_eq!(proxy.parameters(), &[10.0, 14.0, 18.0, 26.0]);
    }

    #[test]
    fn linear_nurbs_leaf_joins_line_polycurve_corner() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0), tol).unwrap();
        let leaf = NurbsCurve::try_new(
            1,
            vec![point(1.0, 0.0, 0.0), point(1.0, 1.0, 0.0)],
            vec![5.0, 5.0, 8.0, 8.0],
        )
        .unwrap();
        let source = Curve3::PolyCurve(
            PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::Line(line), CurveSegment3::NurbsCurve(leaf)],
                vec![10.0, 20.0, 30.0],
            )
            .unwrap(),
        );
        let Curve3::Polyline(offset) = source.try_offset(0.2, normal, tol).unwrap() else {
            panic!("mixed linear polycurve offset")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(0.0, 0.2, 0.0),
                point(0.8, 0.2, 0.0),
                point(0.8, 1.0, 0.0)
            ]
        );
        assert_eq!(offset.domain(), 10.0..=30.0);
    }

    #[test]
    fn smooth_polycurve_offsets_follow_source_normal() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let parabola = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(4.0, 2.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let (first, second) = parabola.try_split(0.5).unwrap();
        let source = Curve3::PolyCurve(
            PolyCurve3::try_with_segment_domains(
                vec![
                    CurveSegment3::NurbsCurve(first),
                    CurveSegment3::NurbsCurve(second),
                ],
                vec![10.0, 20.0, 30.0],
            )
            .unwrap(),
        );
        let Curve3::NurbsCurve(offset) = source.try_offset(0.35, normal, tol).unwrap() else {
            panic!("smooth polycurve offset")
        };
        assert_eq!(offset.domain(), 10.0..=30.0);
        for index in 0..=128 {
            let t = index as Real / 128.0;
            let speed = 1.0_f64.hypot(t);
            let expected = point(4.0 * t - 0.35 * t / speed, 2.0 * t * t + 0.35 / speed, 0.0);
            assert!(
                offset
                    .evaluate(10.0 + 20.0 * t)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    <= tol.absolute()
            );
        }
        let (_, through) = source
            .try_offset_through_point(
                point(0.0, 0.35, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert!(matches!(&through[0], Curve3::NurbsCurve(_)));
    }

    #[test]
    fn ellipse_offsets_follow_analytic_normal_and_preserve_closed_domain() {
        let tol = Tolerance::DEFAULT;
        let x = Vector3::try_new(1.0, 0.0, 0.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let y = Vector3::try_new(0.0, 1.0, 0.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let ellipse = crate::Ellipse3::try_new(point(1.0, 2.0, 3.0), 5.0, 3.0, x, y, tol)
            .unwrap()
            .try_reparameterized(10.0..=20.0)
            .unwrap();
        let source = Curve3::Ellipse(ellipse);
        assert_eq!(source.offset_side(point(1.0, 2.0, 3.0), x, tol), Ok(1.0));
        assert_eq!(source.offset_side(point(9.0, 2.0, 3.0), x, tol), Ok(-1.0));
        assert_eq!(
            source.offset_side(point(6.0, 2.0, 3.0), x, tol),
            Err(GeometryError::AmbiguousCurveOffsetSide)
        );
        let Curve3::NurbsCurve(offset) = source.try_offset(-0.8, x, tol).unwrap() else {
            panic!("smooth offset")
        };
        assert!(offset.is_closed().unwrap());
        assert_eq!(offset.domain(), 10.0..=20.0);
        for index in 0..=256 {
            let angle = std::f64::consts::TAU * index as Real / 256.0;
            let (sine, cosine) = angle.sin_cos();
            let speed = (5.0 * sine).hypot(3.0 * cosine);
            let expected = point(
                1.0 + 5.0 * cosine + 0.8 * 3.0 * cosine / speed,
                2.0 + 3.0 * sine + 0.8 * 5.0 * sine / speed,
                3.0,
            );
            let parameter = 10.0 + 10.0 * index as Real / 256.0;
            assert!(
                offset
                    .evaluate(parameter)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    <= tol.absolute()
            );
        }
        assert!(matches!(
            source.try_offset(1.8, x, tol),
            Err(GeometryError::Degenerate { .. })
        ));
    }

    #[test]
    fn closed_region_rejects_nonadjacent_crossings_and_touches() {
        let crossing = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [0.0, 4.0],
            [2.0, -1.0],
            [0.0, 0.0],
        ];
        let touching = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [0.0, 4.0],
            [2.0, 0.0],
            [0.0, 0.0],
        ];
        let simple_concave = [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [2.0, 2.0],
            [0.0, 4.0],
            [0.0, 0.0],
        ];
        let adjacent_collinear = [
            [0.0, 0.0],
            [2.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [0.0, 4.0],
            [0.0, 0.0],
        ];
        let adjacent_backtrack = [
            [0.0, 0.0],
            [4.0, 0.0],
            [2.0, 0.0],
            [4.0, 4.0],
            [0.0, 4.0],
            [0.0, 0.0],
        ];
        for boundary in [&crossing[..], &touching[..], &adjacent_backtrack[..]] {
            assert_eq!(
                validate_simple_offset_region(boundary),
                Err(GeometryError::SelfIntersectingOffsetRegion)
            );
        }
        for boundary in [&simple_concave[..], &adjacent_collinear[..]] {
            assert_eq!(validate_simple_offset_region(boundary), Ok(()));
        }
    }

    #[test]
    fn offset_region_orientation_resolves_near_collinear_binary64_points() {
        let tiny = 2.0_f64.powi(-50);
        let perturbation = 2.0_f64.powi(-100);
        let a = [0.0, 0.0];
        let b = [1.0, tiny];
        let above = [2.0, 2.0 * tiny + perturbation];
        let below = [2.0, 2.0 * tiny - perturbation];
        assert_eq!(offset_region_orient2(a, b, above), Ordering::Greater);
        assert_eq!(offset_region_orient2(a, b, below), Ordering::Less);
    }

    #[test]
    fn circular_region_boundary_relation_handles_nesting_crossing_and_tangency() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle = |x, radius| {
            Curve3::Circle(Circle3::try_new(point(x, 0.0, 0.0), radius, normal, tol).unwrap())
        };
        let outer = circle(0.0, 10.0);
        assert_eq!(
            outer.offset_region_boundary_relation(&circle(0.0, 3.0), tol),
            Ok(Some(false))
        );
        assert_eq!(
            circle(0.0, 4.0).offset_region_boundary_relation(&circle(4.0, 4.0), tol),
            Ok(Some(true))
        );
        assert_eq!(
            circle(0.0, 4.0).offset_region_boundary_relation(&circle(8.0, 4.0), tol),
            Ok(Some(true))
        );
        assert_eq!(
            circle(0.0, 4.0).offset_region_boundary_relation(&circle(20.0, 4.0), tol),
            Ok(Some(false))
        );
        let tilted = Curve3::Circle(
            Circle3::try_new(
                point(0.0, 0.0, 0.0),
                3.0,
                Vector3::try_new(0.0, 1.0, 1.0)
                    .unwrap()
                    .normalized(tol)
                    .unwrap(),
                tol,
            )
            .unwrap(),
        );
        assert_eq!(
            outer.offset_region_boundary_relation(&tilted, tol),
            Ok(None)
        );
    }

    #[test]
    fn line_offset_preserves_interval_and_rejects_ambiguous_side() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0), tol)
            .unwrap()
            .try_reparameterized(7.0..=9.0)
            .unwrap();
        let curve = Curve3::Line(line);
        assert_eq!(
            curve
                .offset_side(point(2.0, 5.0, 0.0), normal, tol)
                .unwrap(),
            1.0
        );
        assert_eq!(
            curve
                .offset_side(point(2.0, -5.0, 0.0), normal, tol)
                .unwrap(),
            -1.0
        );
        assert_eq!(
            curve.offset_side(point(5.0, 0.0, 0.0), normal, tol),
            Err(GeometryError::AmbiguousCurveOffsetSide)
        );
        let Curve3::Line(offset) = curve.try_offset(2.0, normal, tol).unwrap() else {
            panic!("line")
        };
        assert_eq!(offset.start(), point(0.0, 2.0, 0.0));
        assert_eq!(offset.end(), point(4.0, 2.0, 0.0));
        assert_eq!(offset.domain(), 7.0..=9.0);
    }

    #[test]
    fn circular_offsets_are_exact_and_keep_domains() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle = Circle3::try_new(point(1.0, 2.0, 0.0), 5.0, normal, tol)
            .unwrap()
            .try_reparameterized(10.0..=20.0)
            .unwrap();
        let curve = Curve3::Circle(circle);
        assert_eq!(
            curve
                .offset_side(point(1.0, 2.0, 0.0), normal, tol)
                .unwrap(),
            1.0
        );
        assert_eq!(
            curve
                .offset_side(point(8.0, 2.0, 0.0), normal, tol)
                .unwrap(),
            -1.0
        );
        let Curve3::Circle(offset) = curve.try_offset(-2.0, normal, tol).unwrap() else {
            panic!("circle")
        };
        assert_eq!(offset.radius(), 7.0);
        assert_eq!(offset.domain(), circle.domain());
        assert!(curve.try_offset(5.0, normal, tol).is_err());

        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
            .unwrap()
            .try_reparameterized(3.0..=4.0)
            .unwrap();
        let Curve3::Arc(offset) = Curve3::Arc(arc).try_offset(2.0, normal, tol).unwrap() else {
            panic!("arc")
        };
        assert_eq!(offset.radius(), 3.0);
        assert_eq!(offset.sweep_radians(), arc.sweep_radians());
        assert_eq!(offset.domain(), arc.domain());
    }

    #[test]
    fn unrepresentable_offsets_fail_instead_of_returning_the_input() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = Curve3::Line(
            LineSegment::try_new(point(0.0, 1e16, 0.0), point(4.0, 1e16, 0.0), tol).unwrap(),
        );
        assert!(matches!(
            line.try_offset(1.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
        ));
        let circle =
            Curve3::Circle(Circle3::try_new(point(0.0, 0.0, 0.0), 1e16, normal, tol).unwrap());
        assert!(matches!(
            circle.try_offset(1.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
        ));
    }

    #[test]
    fn sharp_open_and_closed_polyline_offsets_keep_vertex_parameters() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let open = Polyline3::try_with_parameters(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
            ],
            vec![2.0, 5.0, 9.0],
            tol,
        )
        .unwrap();
        let curve = Curve3::Polyline(open);
        assert_eq!(
            curve
                .offset_side(point(1.0, 2.0, 0.0), normal, tol)
                .unwrap(),
            1.0
        );
        let Curve3::Polyline(offset) = curve.try_offset(1.0, normal, tol).unwrap() else {
            panic!("polyline")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 4.0, 0.0)
            ]
        );
        assert_eq!(offset.parameters(), &[2.0, 5.0, 9.0]);

        let closed = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            tol,
        )
        .unwrap();
        let curve = Curve3::Polyline(closed);
        assert_eq!(
            curve
                .offset_side(point(2.0, -2.0, 0.0), normal, tol)
                .unwrap(),
            -1.0
        );
        let Curve3::Polyline(inner) = curve.try_offset(1.0, normal, tol).unwrap() else {
            panic!("polyline")
        };
        assert!(inner.is_closed());
        assert_eq!(
            inner.vertices(),
            &[
                point(1.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 2.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(1.0, 1.0, 0.0)
            ]
        );
        assert_eq!(inner.parameters(), &[0.0, 1.0, 2.0, 3.0, 4.0]);
        let Curve3::Polyline(outer) = curve.try_offset(-1.0, normal, tol).unwrap() else {
            panic!("polyline")
        };
        assert_eq!(
            outer.vertices(),
            &[
                point(-1.0, -1.0, 0.0),
                point(5.0, -1.0, 0.0),
                point(5.0, 4.0, 0.0),
                point(-1.0, 4.0, 0.0),
                point(-1.0, -1.0, 0.0)
            ]
        );
    }

    #[test]
    fn polyline_offset_rejects_collapse_nonplanarity_and_reversing_corners() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let rectangle = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        assert!(matches!(
            rectangle.try_offset(2.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
                | Err(GeometryError::DegeneratePolylineSegment { .. })
        ));
        let nonplanar = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 1.0),
                ],
                tol,
            )
            .unwrap(),
        );
        assert_eq!(
            nonplanar.try_offset(1.0, normal, tol),
            Err(GeometryError::NonPlanarPolyline)
        );
        let reversal = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        assert!(matches!(
            reversal.try_offset(1.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
        ));
    }

    #[test]
    fn polyline_offsets_in_its_own_rotated_plane() {
        let tol = Tolerance::DEFAULT;
        let construction_normal = Vector3::try_new(0.0, 1.0, 0.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let curve = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 0.0, -3.0),
                    point(0.0, 0.0, -3.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        assert_eq!(
            curve
                .offset_side(point(2.0, 0.0, -1.0), construction_normal, tol)
                .unwrap(),
            1.0
        );
        let Curve3::Polyline(offset) = curve.try_offset(1.0, construction_normal, tol).unwrap()
        else {
            panic!("polyline")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(1.0, 0.0, -1.0),
                point(3.0, 0.0, -1.0),
                point(3.0, 0.0, -2.0),
                point(1.0, 0.0, -2.0),
                point(1.0, 0.0, -1.0),
            ]
        );
    }

    #[test]
    fn chamfer_bridges_convex_gaps_and_preserves_source_domain() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let open = Curve3::Polyline(
            Polyline3::try_with_parameters(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                vec![2.0, 5.0, 9.0],
                tol,
            )
            .unwrap(),
        );
        let Curve3::Polyline(chamfer) = open
            .try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("chamfer polyline")
        };
        assert_eq!(
            chamfer.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(4.0, 1.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(5.0, -4.0, 0.0),
            ]
        );
        assert_eq!(chamfer.domain(), 2.0..=9.0);

        let inner = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 4.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let Curve3::Polyline(offset) = inner
            .try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("inner polyline")
        };
        assert_eq!(
            offset.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 4.0, 0.0),
            ]
        );
    }

    #[test]
    fn closed_outward_chamfer_joins_all_corners() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let Curve3::Polyline(offset) = source
            .try_offset_with_corner_style(-1.0, normal, tol, CurveOffsetCornerStyle::Chamfer)
            .unwrap()
        else {
            panic!("closed polyline")
        };
        assert!(offset.is_closed());
        assert_eq!(offset.domain(), 0.0..=4.0);
        assert_eq!(
            offset.vertices(),
            &[
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(4.0, -1.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(5.0, 3.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(-1.0, 3.0, 0.0),
                point(-1.0, 0.0, 0.0),
            ]
        );
    }

    #[test]
    fn round_corner_is_an_exact_tangent_arc() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_with_parameters(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                vec![2.0, 5.0, 9.0],
                tol,
            )
            .unwrap(),
        );
        let Curve3::PolyCurve(offset) = source
            .try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::Round)
            .unwrap()
        else {
            panic!("round polycurve")
        };
        assert_eq!(offset.domain(), 2.0..=9.0);
        assert_eq!(offset.segments().len(), 3);
        let [
            CurveSegment3::Line(first),
            CurveSegment3::Arc(arc),
            CurveSegment3::Line(last),
        ] = offset.segments()
        else {
            panic!("line-arc-line")
        };
        assert_eq!(first.start(), point(0.0, 1.0, 0.0));
        assert_eq!(first.end(), point(4.0, 1.0, 0.0));
        assert_eq!(last.start(), point(5.0, 0.0, 0.0));
        assert_eq!(last.end(), point(5.0, -4.0, 0.0));
        assert_eq!(arc.center(), point(4.0, 0.0, 0.0));
        assert_eq!(arc.radius(), 1.0);
        assert!((arc.sweep_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-14);
        let (_, start_tangent) = CurveSegment3::Arc(*arc)
            .as_ref()
            .evaluate_with_derivative(*arc.domain().start())
            .unwrap();
        let (_, end_tangent) = CurveSegment3::Arc(*arc)
            .as_ref()
            .evaluate_with_derivative(*arc.domain().end())
            .unwrap();
        assert!(
            start_tangent
                .normalized_nonzero()
                .unwrap()
                .as_vector()
                .dot(first.direction(tol).unwrap().as_vector())
                .unwrap()
                > 1.0 - 1e-12
        );
        assert!(
            end_tangent
                .normalized_nonzero()
                .unwrap()
                .as_vector()
                .dot(last.direction(tol).unwrap().as_vector())
                .unwrap()
                > 1.0 - 1e-12
        );
    }

    #[test]
    fn closed_round_offset_has_one_arc_per_convex_corner() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let Curve3::PolyCurve(offset) = source
            .try_offset_with_corner_style(-1.0, normal, tol, CurveOffsetCornerStyle::Round)
            .unwrap()
        else {
            panic!("round polycurve")
        };
        assert_eq!(offset.domain(), 0.0..=4.0);
        assert_eq!(offset.segments().len(), 8);
        assert!(offset.is_closed().unwrap());
        assert_eq!(
            offset
                .segments()
                .iter()
                .filter(|s| matches!(s, CurveSegment3::Arc(_)))
                .count(),
            4
        );
    }

    #[test]
    fn round_corner_respects_rotated_native_plane() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 1.0, 0.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 0.0, 4.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let Curve3::PolyCurve(offset) = source
            .try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::Round)
            .unwrap()
        else {
            panic!("round polycurve")
        };
        let [
            CurveSegment3::Line(first),
            CurveSegment3::Arc(arc),
            CurveSegment3::Line(last),
        ] = offset.segments()
        else {
            panic!("line-arc-line")
        };
        assert_eq!(first.end(), point(4.0, 0.0, -1.0));
        assert_eq!(last.start(), point(5.0, 0.0, 0.0));
        assert_eq!(arc.center(), point(4.0, 0.0, 0.0));
        assert_eq!(arc.normal().unwrap(), normal.opposite());
    }

    #[test]
    fn none_style_splits_open_convex_gap_and_keeps_concave_join() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 4.0, 0.0),
                    point(8.0, 4.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        assert_eq!(
            source.try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::None,),
            Err(GeometryError::DisconnectedCurveOffset)
        );
        let parts = source
            .try_offset_parts(1.0, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 2);
        let [Curve3::Polyline(first), Curve3::Line(last)] = parts.as_slice() else {
            panic!("open pieces")
        };
        assert_eq!(
            first.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 4.0, 0.0),
            ]
        );
        assert_eq!(last.start(), point(4.0, 5.0, 0.0));
        assert_eq!(last.end(), point(8.0, 5.0, 0.0));
    }

    #[test]
    fn none_style_splits_closed_outward_square_into_four_lines() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 3.0, 0.0),
                    point(0.0, 3.0, 0.0),
                    point(0.0, 0.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let parts = source
            .try_offset_parts(-1.0, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert_eq!(parts.len(), 4);
        let expected = [
            (point(0.0, -1.0, 0.0), point(4.0, -1.0, 0.0)),
            (point(5.0, 0.0, 0.0), point(5.0, 3.0, 0.0)),
            (point(4.0, 4.0, 0.0), point(0.0, 4.0, 0.0)),
            (point(-1.0, 3.0, 0.0), point(-1.0, 0.0, 0.0)),
        ];
        for (part, (start, end)) in parts.iter().zip(expected) {
            let Curve3::Line(line) = part else {
                panic!("line piece")
            };
            assert_eq!((line.start(), line.end()), (start, end));
        }
    }

    #[test]
    fn none_style_without_gaps_preserves_polyline_domain() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_with_parameters(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, 4.0, 0.0),
                ],
                vec![2.0, 5.0, 9.0],
                tol,
            )
            .unwrap(),
        );
        let parts = source
            .try_offset_parts(1.0, normal, tol, CurveOffsetCornerStyle::None)
            .unwrap();
        assert!(matches!(
            source.try_offset_with_corner_style(1.0, normal, tol, CurveOffsetCornerStyle::None),
            Ok(Curve3::Polyline(_))
        ));
        let [Curve3::Polyline(result)] = parts.as_slice() else {
            panic!("single polyline")
        };
        assert_eq!(result.parameters(), &[2.0, 5.0, 9.0]);
        assert_eq!(
            result.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 4.0, 0.0),
            ]
        );
    }

    #[test]
    fn through_point_offsets_analytic_curves_and_checks_actual_locus() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = Curve3::Line(
            LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0), tol).unwrap(),
        );
        let (distance, parts) = line
            .try_offset_through_point(
                point(2.0, 2.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert_eq!(distance, 2.0);
        let [Curve3::Line(offset)] = parts.as_slice() else {
            panic!("line offset")
        };
        assert_eq!(offset.start(), point(0.0, 2.0, 0.0));
        assert_eq!(
            line.try_offset_through_point(
                point(5.0, 2.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            ),
            Err(GeometryError::OffsetThroughPointNoSolution)
        );
        assert_eq!(
            line.try_offset_through_point(
                point(2.0, 2.0, 1.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            ),
            Err(GeometryError::OffsetThroughPointOffPlane)
        );
        let slope = Curve3::Line(
            LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 4.0), tol).unwrap(),
        );
        let (distance, parts) = slope
            .try_offset_through_point(
                point(2.0, 2.0, 2.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert_eq!(distance, 2.0);
        assert!(matches!(parts.as_slice(), [Curve3::Line(_)]));
        assert_eq!(
            slope.try_offset_through_point(
                point(2.0, 2.0, 3.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            ),
            Err(GeometryError::OffsetThroughPointOffPlane)
        );

        let x_axis = Vector3::try_new(1.0, 0.0, 0.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle =
            Circle3::try_from_frame(point(0.0, 0.0, 0.0), 5.0, x_axis, normal, tol).unwrap();
        let (distance, parts) = Curve3::Circle(circle)
            .try_offset_through_point(
                point(7.0, 0.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert_eq!(distance, -2.0);
        let [Curve3::Circle(offset)] = parts.as_slice() else {
            panic!("circle offset")
        };
        assert_eq!(offset.radius(), 7.0);

        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
        let (distance, parts) = Curve3::Arc(arc)
            .try_offset_through_point(
                point(0.0, 7.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert_eq!(distance, -2.0);
        assert!(matches!(parts.as_slice(), [Curve3::Arc(_)]));
        assert_eq!(
            Curve3::Arc(arc).try_offset_through_point(
                point(-7.0, 0.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            ),
            Err(GeometryError::OffsetThroughPointNoSolution)
        );
    }

    #[test]
    fn through_point_finds_line_and_round_polyline_offsets() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let source = Curve3::Polyline(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    point(4.0, -4.0, 0.0),
                ],
                tol,
            )
            .unwrap(),
        );
        let (distance, parts) = source
            .try_offset_through_point(
                point(2.0, 1.0, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            )
            .unwrap();
        assert_eq!(distance, 1.0);
        assert!(matches!(parts.as_slice(), [Curve3::Polyline(_)]));
        let root = 0.5_f64.sqrt();
        let (distance, parts) = source
            .try_offset_through_point(
                point(4.0 + root, root, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Round,
            )
            .unwrap();
        assert!((distance - 1.0).abs() <= tol.absolute());
        assert!(matches!(parts.as_slice(), [Curve3::PolyCurve(_)]));
        let (distance, parts) = source
            .try_offset_through_point(
                point(4.5, 0.5, 0.0),
                normal,
                tol,
                CurveOffsetCornerStyle::Chamfer,
            )
            .unwrap();
        assert!((distance - 1.0).abs() <= tol.absolute());
        assert!(matches!(parts.as_slice(), [Curve3::Polyline(_)]));
        assert_eq!(
            source.try_offset_through_point(
                point(2.0, 1.0, 0.1),
                normal,
                tol,
                CurveOffsetCornerStyle::Sharp,
            ),
            Err(GeometryError::OffsetThroughPointOffPlane)
        );
    }
}
