use super::*;

enum FilletPart {
    Straight(Vec<Point3>),
    Curved(CurveSegment3),
}

impl PolyCurve3 {
    /// Fillets straight-span corners while retaining smooth curved leaves.
    /// Kinks touching curved leaves and internal curved-leaf kinks require a
    /// general curve fillet and are rejected rather than changing their locus.
    pub fn try_fillet_corners(
        &self,
        radius: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius], "fillet radius")?;
        if radius <= tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "fillet radius",
            });
        }
        let mut parts = Vec::with_capacity(self.segments.len());
        for segment in &self.segments {
            if let Some(vertices) = straight_leaf_vertices(segment)? {
                parts.push(FilletPart::Straight(vertices));
            } else {
                if let CurveSegment3::NurbsCurve(curve) = segment {
                    check_curved_nurbs_is_smooth(curve, tolerance)?;
                }
                parts.push(FilletPart::Curved(segment.clone()));
            }
        }
        if parts
            .iter()
            .all(|part| matches!(part, FilletPart::Straight(_)))
        {
            let mut vertices = Vec::new();
            for part in parts {
                let FilletPart::Straight(points) = part else {
                    unreachable!()
                };
                append_straight_vertices(&mut vertices, &points)?;
            }
            if self.is_closed()? && vertices.first() != vertices.last() {
                return Err(unsupported_straight_polycurve());
            }
            return Polyline3::try_new(vertices, tolerance)?.try_fillet_corners(radius, tolerance);
        }
        let closed = self.is_closed()?;
        for pair in parts.windows(2) {
            if matches!(pair[0], FilletPart::Curved(_)) || matches!(pair[1], FilletPart::Curved(_))
            {
                check_smooth_joint(&pair[0], &pair[1], tolerance)?;
            }
        }
        if closed {
            let last = parts.last().unwrap();
            let first = &parts[0];
            if matches!(last, FilletPart::Straight(_)) && matches!(first, FilletPart::Straight(_)) {
                let (end, incoming) = part_end(last)?;
                let (start, outgoing) = part_start(first)?;
                if !curve_points_coincident(end, start) {
                    return Err(unsupported_straight_polycurve());
                }
                if tangent_angle(incoming, outgoing)? > tolerance.angular() {
                    return fillet_closed_mixed_sharp_seam(parts, radius, tolerance);
                }
            } else {
                check_smooth_joint(last, first, tolerance)?;
            }
        }
        let mut result = Vec::new();
        let mut run = Vec::new();
        for part in parts {
            match part {
                FilletPart::Straight(points) => append_straight_vertices(&mut run, &points)?,
                FilletPart::Curved(curve) => {
                    append_rounded_run(&mut result, std::mem::take(&mut run), radius, tolerance)?;
                    result.push(curve);
                }
            }
        }
        append_rounded_run(&mut result, run, radius, tolerance)?;
        Self::try_new(result)
    }
}

fn fillet_closed_mixed_sharp_seam(
    parts: Vec<FilletPart>,
    radius: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let first_curved = parts
        .iter()
        .position(|part| matches!(part, FilletPart::Curved(_)))
        .unwrap();
    let last_curved = parts
        .iter()
        .rposition(|part| matches!(part, FilletPart::Curved(_)))
        .unwrap();
    let mut joined = Vec::new();
    for part in &parts[last_curved + 1..] {
        let FilletPart::Straight(points) = part else {
            unreachable!()
        };
        append_straight_vertices(&mut joined, points)?;
    }
    let seam_corner = joined.len() - 1;
    for part in &parts[..first_curved] {
        let FilletPart::Straight(points) = part else {
            unreachable!()
        };
        append_straight_vertices(&mut joined, points)?;
    }
    let (seam_run, seam_piece) = Polyline3::try_new(joined, tolerance)?
        .try_fillet_corners_with_marked_corner(radius, tolerance, seam_corner)?;
    let seam_segments = seam_run.segments();
    let mut result = seam_segments[seam_piece..].to_vec();
    let mut run = Vec::new();
    for part in parts
        .into_iter()
        .skip(first_curved)
        .take(last_curved - first_curved + 1)
    {
        match part {
            FilletPart::Straight(points) => append_straight_vertices(&mut run, &points)?,
            FilletPart::Curved(curve) => {
                append_rounded_run(&mut result, std::mem::take(&mut run), radius, tolerance)?;
                result.push(curve);
            }
        }
    }
    append_rounded_run(&mut result, run, radius, tolerance)?;
    result.extend_from_slice(&seam_segments[..seam_piece]);
    PolyCurve3::try_new(result)
}

fn straight_leaf_vertices(segment: &CurveSegment3) -> Result<Option<Vec<Point3>>, GeometryError> {
    Ok(match segment {
        CurveSegment3::Line(line) => Some(vec![line.start(), line.end()]),
        CurveSegment3::Polyline(polyline) => Some(polyline.vertices().to_vec()),
        CurveSegment3::NurbsCurve(curve) => {
            let mut points = Vec::new();
            for (start, end) in curve.spans() {
                let span = curve.try_trimmed(start..=end)?;
                if !span.is_linear_at_zero_tolerance()? {
                    return Ok(None);
                }
                append_straight_vertices(
                    &mut points,
                    &[
                        span.evaluate(*span.domain().start())?,
                        span.evaluate(*span.domain().end())?,
                    ],
                )?;
            }
            Some(points)
        }
        CurveSegment3::Arc(_) => None,
    })
}

fn check_curved_nurbs_is_smooth(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    for (knot, _) in curve.interior_knot_groups() {
        let source = crate::CurveRef::NurbsCurve(curve);
        let left = source.evaluate_with_tangent_on_side(knot, ParameterSide::Left)?;
        let right = source.evaluate_with_tangent_on_side(knot, ParameterSide::Right)?;
        if !curve_points_coincident(left.point(), right.point())
            || tangent_angle(left.tangent().as_vector(), right.tangent().as_vector())?
                > tolerance.angular()
        {
            return Err(unsupported_curved_corner());
        }
    }
    Ok(())
}

fn check_smooth_joint(
    before: &FilletPart,
    after: &FilletPart,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let (before_point, before_tangent) = part_end(before)?;
    let (after_point, after_tangent) = part_start(after)?;
    if !curve_points_coincident(before_point, after_point)
        || tangent_angle(before_tangent, after_tangent)? > tolerance.angular()
    {
        return Err(unsupported_curved_corner());
    }
    Ok(())
}

fn part_start(part: &FilletPart) -> Result<(Point3, Vector3), GeometryError> {
    match part {
        FilletPart::Straight(points) => Ok((points[0], points[0].vector_to(points[1])?)),
        FilletPart::Curved(curve) => {
            let sample = curve
                .as_ref()
                .evaluate_with_tangent_on_side(*curve.domain().start(), ParameterSide::Right)?;
            Ok((sample.point(), sample.tangent().as_vector()))
        }
    }
}

fn part_end(part: &FilletPart) -> Result<(Point3, Vector3), GeometryError> {
    match part {
        FilletPart::Straight(points) => {
            let end = points.len() - 1;
            Ok((points[end], points[end - 1].vector_to(points[end])?))
        }
        FilletPart::Curved(curve) => {
            let sample = curve
                .as_ref()
                .evaluate_with_tangent_on_side(*curve.domain().end(), ParameterSide::Left)?;
            Ok((sample.point(), sample.tangent().as_vector()))
        }
    }
}

fn tangent_angle(first: Vector3, second: Vector3) -> Result<Real, GeometryError> {
    let first = first.normalized_nonzero()?.as_vector();
    let second = second.normalized_nonzero()?.as_vector();
    Ok(first.cross(second)?.length()?.atan2(first.dot(second)?))
}

fn append_rounded_run(
    result: &mut Vec<CurveSegment3>,
    vertices: Vec<Point3>,
    radius: Real,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    if vertices.is_empty() {
        return Ok(());
    }
    result.extend(
        Polyline3::try_new(vertices, tolerance)?
            .try_fillet_corners(radius, tolerance)?
            .segments()
            .iter()
            .cloned(),
    );
    Ok(())
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

fn unsupported_curved_corner() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "FilletCorners cannot round a kink involving a curved leaf",
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
    fn smooth_curved_leaf_is_preserved() {
        let arc = crate::CircularArc3::try_from_three_points(
            p(0., 0.),
            p(1., 1.),
            p(2., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = PolyCurve3::try_new(vec![CurveSegment3::Arc(arc)]).unwrap();
        assert_eq!(
            source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap(),
            source
        );
    }

    #[test]
    fn smooth_arc_remains_exact_while_a_later_line_corner_is_rounded() {
        let midpoint = 2_f64.sqrt();
        let arc = crate::CircularArc3::try_from_three_points(
            p(2., 0.),
            p(2. + midpoint, 2. - midpoint),
            p(4., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = PolyCurve3::try_new(vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 0.), p(2., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Arc(arc),
            CurveSegment3::Line(
                LineSegment::try_new(p(4., 2.), p(4., 6.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Line(
                LineSegment::try_new(p(4., 6.), p(8., 6.), Tolerance::DEFAULT).unwrap(),
            ),
        ])
        .unwrap();
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.segments().len(), 5);
        assert_eq!(result.segments()[1], CurveSegment3::Arc(arc));
        let CurveSegment3::Arc(fillet) = result.segments()[3] else {
            panic!("straight corner has a fillet arc")
        };
        assert!((fillet.radius() - 0.5).abs() < 1e-12);

        let curved_kink = PolyCurve3::try_new(vec![
            CurveSegment3::Arc(arc),
            CurveSegment3::Line(
                LineSegment::try_new(p(4., 2.), p(8., 2.), Tolerance::DEFAULT).unwrap(),
            ),
        ])
        .unwrap();
        assert!(
            curved_kink
                .try_fillet_corners(0.5, Tolerance::DEFAULT)
                .is_err()
        );
    }

    #[test]
    fn closed_smooth_arc_keeps_its_seam_and_rounds_straight_run() {
        let diagonal = 2_f64.sqrt();
        let arc = crate::CircularArc3::try_from_three_points(
            p(2., 0.),
            p(2. + diagonal, 2. - diagonal),
            p(4., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut segments = vec![CurveSegment3::Arc(arc)];
        for (start, end) in [
            (p(4., 2.), p(4., 6.)),
            (p(4., 6.), p(0., 6.)),
            (p(0., 6.), p(0., 0.)),
            (p(0., 0.), p(2., 0.)),
        ] {
            segments.push(CurveSegment3::Line(
                LineSegment::try_new(start, end, Tolerance::DEFAULT).unwrap(),
            ));
        }
        let source = PolyCurve3::try_new(segments).unwrap();
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(result.is_closed().unwrap());
        assert_eq!(result.segments().len(), 8);
        assert_eq!(result.segments()[0], CurveSegment3::Arc(arc));
    }

    #[test]
    fn closed_mixed_sharp_straight_seam_starts_at_its_incoming_fillet() {
        let diagonal = 2_f64.sqrt();
        let arc = crate::CircularArc3::try_from_three_points(
            p(2., 0.),
            p(2. + diagonal, 2. - diagonal),
            p(4., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut segments = vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 0.), p(2., 0.), Tolerance::DEFAULT).unwrap(),
            ),
            CurveSegment3::Arc(arc),
        ];
        for (start, end) in [
            (p(4., 2.), p(4., 6.)),
            (p(4., 6.), p(0., 6.)),
            (p(0., 6.), p(0., 0.)),
        ] {
            segments.push(CurveSegment3::Line(
                LineSegment::try_new(start, end, Tolerance::DEFAULT).unwrap(),
            ));
        }
        let source = PolyCurve3::try_new(segments).unwrap();
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(result.is_closed().unwrap());
        assert_eq!(result.segments().len(), 8);
        let CurveSegment3::Arc(seam_arc) = result.segments()[0] else {
            panic!("closed result starts with its seam fillet")
        };
        assert!(seam_arc.start().unwrap().distance_to(p(0., 0.5)).unwrap() < 1e-14);
        assert_eq!(result.segments()[2], CurveSegment3::Arc(arc));
    }

    #[test]
    fn closed_mixed_smooth_straight_seam_retains_source_point() {
        let diagonal = 2_f64.sqrt();
        let arc = crate::CircularArc3::try_from_three_points(
            p(2., 0.),
            p(2. + diagonal, 2. - diagonal),
            p(4., 2.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = PolyCurve3::try_new(vec![
            CurveSegment3::Polyline(
                Polyline3::try_new(vec![p(0., 1.), p(0., 0.), p(2., 0.)], Tolerance::DEFAULT)
                    .unwrap(),
            ),
            CurveSegment3::Arc(arc),
            CurveSegment3::Polyline(
                Polyline3::try_new(
                    vec![p(4., 2.), p(4., 6.), p(0., 6.), p(0., 1.)],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ),
        ])
        .unwrap();
        let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
        assert!(result.is_closed().unwrap());
        assert_eq!(
            result.evaluate(*result.domain().start()).unwrap(),
            p(0., 1.)
        );
        assert_eq!(result.segments()[3], CurveSegment3::Arc(arc));
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
