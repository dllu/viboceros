//! Exact offsets of analytic curves in an oriented plane.

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
}

impl Curve3 {
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

    /// Offset with the requested corner treatment for planar polylines.
    /// Analytic curve families have no polyline corners and ignore `corner`.
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
            Self::Polyline(polyline) => {
                offset_polyline(polyline, distance, plane_normal, tolerance, corner)
            }
            _ => Err(GeometryError::UnsupportedCurveOffset),
        }
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
                        context: "offset chamfer under model precision",
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
    if corner_style == CurveOffsetCornerStyle::Round && corners.iter().any(|(a, b)| a != b) {
        return Ok(Curve3::PolyCurve(round_offset_polyline(
            polyline, &corners, normal, tolerance,
        )?));
    }
    let mut result = Vec::with_capacity(vertices.len() + corners.len());
    for &(incoming, outgoing) in &corners {
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
    use crate::{LineSegment, Point3, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
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
}
