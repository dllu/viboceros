//! Bounded angular refinement of positive-weight Bézier control polygons.
use super::*;
fn validate_source(curve: CurveRef<'_>) -> Result<(), GeometryError> {
    match curve {
        CurveRef::NurbsCurve(c) => {
            let sign = c.control_points()[0].weight().is_sign_positive();
            if c.control_points()
                .iter()
                .any(|p| p.weight().is_sign_positive() != sign)
            {
                return Err(GeometryError::UnsupportedCurveApproximation {
                    context: "angular approximation needs common-sign weights",
                });
            }
            for (t, _) in c.spans().skip(1) {
                if c.evaluate_on_side(t, ParameterSide::Left)?
                    != c.evaluate_on_side(t, ParameterSide::Right)?
                {
                    return Err(GeometryError::UnsupportedCurveApproximation {
                        context: "angular approximation crosses a discontinuity",
                    });
                }
            }
        }
        CurveRef::PolyCurve(c) => {
            for segment in c.segments() {
                validate_source(segment.as_ref())?;
            }
        }
        _ => (),
    }
    Ok(())
}
impl CurveRef<'_> {
    /// Keeps existing polyline vertices or refines common-sign rational Bézier
    /// spans until their control edges lie inside the requested angular cone.
    /// This controls direction variation; it is not a positional error bound.
    pub fn try_polyline_by_angle(
        self,
        angle: Real,
        maximum: usize,
    ) -> Result<Polyline3, GeometryError> {
        if !angle.is_finite()
            || angle <= 0.
            || angle >= std::f64::consts::PI
            || !(2..=MAX_CURVE_DIVISION_POINTS).contains(&maximum)
        {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "invalid angular approximation settings",
            });
        }
        validate_source(self)?;
        if let Some(points) = self.polyline_vertices()? {
            if points.len() > maximum {
                return Err(GeometryError::CurveApproximationLimit);
            }
            return Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION);
        }
        let curve = self.to_nurbs()?;
        let sign = curve.control_points()[0].weight().is_sign_positive();
        if curve
            .control_points()
            .iter()
            .any(|p| p.weight().is_sign_positive() != sign)
        {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "angular approximation needs common-sign weights",
            });
        }
        let mut points = Vec::new();
        let mut work = 0usize;
        let mut previous_end = None;
        for span in curve.try_bezier_spans()? {
            let domain = span.domain();
            let first = curve.evaluate_on_side(*domain.start(), ParameterSide::Right)?;
            if previous_end.is_some_and(|p| p != first) {
                return Err(GeometryError::UnsupportedCurveApproximation {
                    context: "angular approximation crosses a discontinuity",
                });
            }
            previous_end = Some(curve.evaluate_on_side(*domain.end(), ParameterSide::Left)?);
            let mut pending = vec![(span, 0)];
            while let Some((span, depth)) = pending.pop() {
                work = work
                    .checked_add(span.control_points().len())
                    .filter(|n| *n <= 4_000_000)
                    .ok_or(GeometryError::CurveApproximationLimit)?;
                let controls = span.control_points();
                let first = controls[0].point();
                let last = controls.last().unwrap().point();
                let chord = first.vector_to(last)?.normalized_nonzero();
                let flat = if let Ok(chord) = chord {
                    let mut flat = true;
                    for edge in controls.windows(2) {
                        if edge[0].point() == edge[1].point() {
                            continue;
                        }
                        let direction = edge[0]
                            .point()
                            .vector_to(edge[1].point())?
                            .normalized_nonzero()?;
                        if direction.as_vector().dot(chord.as_vector())? < (angle * 0.5).cos() {
                            flat = false;
                            break;
                        }
                    }
                    flat
                } else {
                    false
                };
                if flat {
                    // Round shared stations through one source evaluator;
                    // independently rounded split control nets can differ in
                    // their last bits without a discontinuity in the source.
                    let domain = span.domain();
                    let first = curve.evaluate(*domain.start())?;
                    let last = curve.evaluate(*domain.end())?;
                    if points.is_empty() {
                        points.push(first);
                    } else if points.last() != Some(&first) {
                        return Err(GeometryError::UnsupportedCurveApproximation {
                            context: "angular approximation crosses a discontinuity",
                        });
                    }
                    if points.len() == maximum {
                        return Err(GeometryError::CurveApproximationLimit);
                    }
                    points.push(last);
                } else {
                    let domain = span.domain();
                    let middle = domain.start().midpoint(*domain.end());
                    if depth >= 32 || middle <= *domain.start() || middle >= *domain.end() {
                        return Err(GeometryError::CurveApproximationLimit);
                    }
                    let (left, right) = span.try_split(middle)?;
                    pending.push((right, depth + 1));
                    pending.push((left, depth + 1));
                }
            }
        }
        if self.is_closed()? && points.len() > 2 {
            *points.last_mut().unwrap() = points[0];
        }
        Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    #[test]
    fn polyline_recognition_does_not_bridge_rational_poles_or_knot_gaps() {
        let pole = NurbsCurve::try_new_rational(
            1,
            vec![
                crate::WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(1., 0.), -1.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert!(
            CurveRef::NurbsCurve(&pole)
                .try_polyline_by_angle(0.2, 100)
                .is_err()
        );
        let gap = NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(1., 0.), p(2., 0.), p(3., 0.)],
            vec![0., 0., 1., 1., 2., 2.],
        )
        .unwrap();
        assert!(
            CurveRef::NurbsCurve(&gap)
                .try_polyline_by_angle(0.2, 100)
                .is_err()
        );
        let polyline =
            Polyline3::try_new(vec![p(0., 0.), p(1., 0.), p(1., 1.)], Tolerance::DEFAULT).unwrap();
        assert_eq!(
            CurveRef::Polyline(&polyline)
                .try_polyline_by_angle(0.2, 100)
                .unwrap(),
            polyline
        );
        assert!(matches!(
            CurveRef::Polyline(&polyline).try_polyline_by_angle(0.2, 2),
            Err(GeometryError::CurveApproximationLimit)
        ));
    }
}
