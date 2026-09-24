use super::*;

impl PolyCurve3 {
    /// Fillets the corners of a polycurve whose leaves are exactly straight.
    /// Linear NURBS knot spans and polyline vertices remain separate corners.
    /// Curved leaves are rejected before changing any caller-owned geometry.
    pub fn try_fillet_corners(
        &self,
        radius: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut vertices = Vec::new();
        for segment in &self.segments {
            match segment {
                CurveSegment3::Line(line) => {
                    append_straight_vertices(&mut vertices, &[line.start(), line.end()])?;
                }
                CurveSegment3::Polyline(polyline) => {
                    append_straight_vertices(&mut vertices, polyline.vertices())?;
                }
                CurveSegment3::NurbsCurve(curve) => {
                    for (start, end) in curve.spans() {
                        let span = curve.try_trimmed(start..=end)?;
                        if !span.is_linear_at_zero_tolerance()? {
                            return Err(unsupported_straight_polycurve());
                        }
                        append_straight_vertices(
                            &mut vertices,
                            &[
                                span.evaluate(*span.domain().start())?,
                                span.evaluate(*span.domain().end())?,
                            ],
                        )?;
                    }
                }
                CurveSegment3::Arc(_) => return Err(unsupported_straight_polycurve()),
            }
        }
        if self.is_closed()? && vertices.first() != vertices.last() {
            return Err(unsupported_straight_polycurve());
        }
        Polyline3::try_new(vertices, tolerance)?.try_fillet_corners(radius, tolerance)
    }
}

fn append_straight_vertices(
    destination: &mut Vec<Point3>,
    vertices: &[Point3],
) -> Result<(), GeometryError> {
    if vertices.len() < 2 {
        return Err(unsupported_straight_polycurve());
    }
    let skip = usize::from(!destination.is_empty());
    if destination
        .len()
        .checked_add(vertices.len() - skip)
        .is_none_or(|count| count > MAX_POLYCURVE_SEGMENTS / 2 + 1)
    {
        return Err(GeometryError::InvalidPolyCurve {
            context: "too many fillet corners",
        });
    }
    if let (Some(&last), Some(&first)) = (destination.last(), vertices.first())
        && last != first
    {
        return Err(unsupported_straight_polycurve());
    }
    destination.extend(vertices.iter().skip(skip).copied());
    Ok(())
}

fn unsupported_straight_polycurve() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "FilletCorners requires straight polycurve leaves with exact junctions",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, NurbsCurve};

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn mixed_straight_leaves_fillet_internal_polyline_vertices() {
        let source = PolyCurve3::try_new(vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 0.), p(4., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Polyline(
                Polyline3::try_new(vec![p(4., 0.), p(4., 4.), p(8., 4.)], Tolerance::DEFAULT)
                    .unwrap(),
            ),
        ])
        .unwrap();
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.segments().len(), 5);
        assert!(!result.is_closed().unwrap());
    }

    #[test]
    fn curved_leaf_is_rejected() {
        let arc = crate::CircularArc3::try_from_three_points(
            p(0., 0.),
            p(1., 1.),
            p(2., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = PolyCurve3::try_new(vec![CurveSegment3::Arc(arc)]).unwrap();
        assert!(source.try_fillet_corners(0.5, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn discontinuous_linear_nurbs_leaf_does_not_bridge_its_jump() {
        let curve = NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(1., 0.), p(2., 0.), p(3., 0.)],
            vec![0., 0., 1., 1., 2., 2.],
        )
        .unwrap();
        let source = PolyCurve3::try_new(vec![CurveSegment3::NurbsCurve(curve)]).unwrap();
        assert!(source.try_fillet_corners(0.25, Tolerance::DEFAULT).is_err());
    }
}
