use super::*;
use crate::{CircularArc3, CurveSegment3, PolyCurve3};

#[derive(Clone, Copy)]
struct RoundedCorner {
    incoming: Point3,
    outgoing: Point3,
    arc: Option<CircularArc3>,
    setback: Real,
}

impl Polyline3 {
    /// Replaces each nonstraight corner with an exact tangent circular arc.
    /// Adjacent arcs may meet at a shared tangent point, but may not overlap.
    /// The result is built before the caller changes any document object.
    pub fn try_fillet_corners(
        &self,
        radius: Real,
        tolerance: Tolerance,
    ) -> Result<PolyCurve3, GeometryError> {
        require_finite([radius], "fillet radius")?;
        if radius <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "fillet radius",
            });
        }
        let closed = self.is_closed();
        let count = self.vertices.len() - usize::from(closed);
        let segment_count = if closed { count } else { count - 1 };
        if segment_count
            .checked_mul(2)
            .is_none_or(|maximum| maximum > crate::polycurve::MAX_POLYCURVE_SEGMENTS)
        {
            return Err(GeometryError::InvalidPolyCurve {
                context: "too many fillet segments",
            });
        }
        let corners = (0..count)
            .map(|index| {
                let vertex = self.vertices[index];
                if !closed && (index == 0 || index + 1 == count) {
                    return Ok(RoundedCorner {
                        incoming: vertex,
                        outgoing: vertex,
                        arc: None,
                        setback: 0.0,
                    });
                }
                let previous = self.vertices[(index + count - 1) % count];
                let next = self.vertices[(index + 1) % count];
                rounded_corner(previous, vertex, next, radius, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut pieces = Vec::with_capacity(segment_count * 2);
        // Rhino retains the source's closed seam at the incoming tangent
        // point of its first corner, then traverses that arc before edge 0.
        if closed && let Some(arc) = corners[0].arc {
            pieces.push(CurveSegment3::Arc(arc));
        }
        for index in 0..segment_count {
            let next = (index + 1) % count;
            let length = self.vertices[index].distance_to(self.vertices[index + 1])?;
            let remainder = length - (corners[index].setback + corners[next].setback);
            let roundoff = 16.0 * Real::EPSILON * length;
            if remainder < -roundoff {
                return Err(GeometryError::Degenerate {
                    context: "fillet corners overlap on a segment",
                });
            }
            if remainder > roundoff {
                pieces.push(CurveSegment3::Line(LineSegment::try_new(
                    corners[index].outgoing,
                    corners[next].incoming,
                    tolerance,
                )?));
            }
            if (!closed || next != 0)
                && let Some(arc) = corners[next].arc
            {
                pieces.push(CurveSegment3::Arc(arc));
            }
        }
        PolyCurve3::try_new(pieces)
    }
}

fn rounded_corner(
    previous: Point3,
    vertex: Point3,
    next: Point3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<RoundedCorner, GeometryError> {
    let incoming = previous
        .vector_to(vertex)?
        .normalized_nonzero()?
        .as_vector();
    let outgoing = vertex.vector_to(next)?.normalized_nonzero()?.as_vector();
    let cross = incoming.cross(outgoing)?;
    let turn = cross.length()?.atan2(incoming.dot(outgoing)?);
    if turn <= tolerance.angular() {
        return Ok(RoundedCorner {
            incoming: vertex,
            outgoing: vertex,
            arc: None,
            setback: 0.0,
        });
    }
    if std::f64::consts::PI - turn <= tolerance.angular() {
        return Err(GeometryError::Degenerate {
            context: "reversing fillet corner",
        });
    }
    let setback = radius * (turn * 0.5).tan();
    require_finite([setback], "fillet setback")?;
    let start = vertex.translated(incoming.scaled(-setback)?)?;
    let end = vertex.translated(outgoing.scaled(setback)?)?;
    let normal = cross.normalized_nonzero()?.as_vector();
    let center = start.translated(normal.cross(incoming)?.scaled(radius)?)?;
    let bisector = Vector3::try_new(
        incoming.x() + outgoing.x(),
        incoming.y() + outgoing.y(),
        incoming.z() + outgoing.z(),
    )?;
    let middle_direction = normal.cross(bisector)?.scaled(-1.0)?.normalized_nonzero()?;
    let middle = center.translated(middle_direction.as_vector().scaled(radius)?)?;
    let arc = CircularArc3::try_from_three_points(start, middle, end, tolerance)?;
    Ok(RoundedCorner {
        incoming: start,
        outgoing: end,
        arc: Some(arc),
        setback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn open_and_closed_corners_have_exact_tangent_arcs() {
        let open = Polyline3::try_new(
            vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 4., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let rounded = open.try_fillet_corners(1., Tolerance::DEFAULT).unwrap();
        assert_eq!(rounded.segments().len(), 3);
        let CurveSegment3::Arc(arc) = &rounded.segments()[1] else {
            panic!("middle segment is a tangent arc")
        };
        assert!((arc.radius() - 1.).abs() < 1e-12);
        assert!(arc.start().unwrap().distance_to(p(3., 0., 0.)).unwrap() < 1e-12);
        assert!(arc.end().unwrap().distance_to(p(4., 1., 0.)).unwrap() < 1e-12);

        let clockwise = Polyline3::try_new(
            vec![p(0., 0., 0.), p(4., 0., 0.), p(4., -4., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_fillet_corners(1., Tolerance::DEFAULT)
        .unwrap();
        let CurveSegment3::Arc(clockwise_arc) = &clockwise.segments()[1] else {
            panic!("clockwise turn has an arc")
        };
        assert!(
            clockwise_arc
                .start()
                .unwrap()
                .distance_to(p(3., 0., 0.))
                .unwrap()
                < 1e-12
        );
        assert!(
            clockwise_arc
                .end()
                .unwrap()
                .distance_to(p(4., -1., 0.))
                .unwrap()
                < 1e-12
        );

        let closed = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(4., 0., 0.),
                p(4., 4., 0.),
                p(0., 4., 0.),
                p(0., 0., 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let rounded = closed.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(rounded.segments().len(), 8);
        assert!(rounded.is_closed().unwrap());
        let start = rounded.segments()[0]
            .evaluate(*rounded.segments()[0].domain().start())
            .unwrap();
        assert!(start.distance_to(p(0., 0.5, 0.)).unwrap() < 1e-12);
    }

    #[test]
    fn overlapping_fillets_fail_before_creating_a_result() {
        let source = Polyline3::try_new(
            vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(source.try_fillet_corners(1.1, Tolerance::DEFAULT).is_err());

        let square = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(4., 0., 0.),
                p(4., 4., 0.),
                p(0., 4., 0.),
                p(0., 0., 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let touching = square.try_fillet_corners(2., Tolerance::DEFAULT).unwrap();
        assert_eq!(touching.segments().len(), 4);
        assert!(touching.is_closed().unwrap());
        assert!(square.try_fillet_corners(2.1, Tolerance::DEFAULT).is_err());
    }
}
