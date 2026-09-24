use super::*;
use crate::{CircularArc3, CurveRef, curve_offset::nurbs_offset_plane};

const SAMPLES_PER_SPAN: usize = 48;
const MAX_ROOT_SAMPLES: usize = 65_536;

struct NurbsArcSolution {
    curve: NurbsCurve,
    fillet: CircularArc3,
    arc: CircularArc3,
    removed_length: Real,
}

struct Search<'a> {
    curve: &'a NurbsCurve,
    arc: CircularArc3,
    radius: Real,
    tolerance: Tolerance,
    normal: Vector3,
}

pub(super) fn resolve_nurbs_arc_kinks(
    source: &PolyCurve3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<PolyCurve3>, GeometryError> {
    let closed = source.is_closed()?;
    let mut segments = source.segments().to_vec();
    let mut changed = false;
    let mut index = 0;
    while index + 1 < segments.len() {
        if let Some((before, fillet, after)) =
            fillet_pair(&segments[index], &segments[index + 1], radius, tolerance)?
        {
            segments.splice(
                index..=index + 1,
                [before, CurveSegment3::Arc(fillet), after],
            );
            changed = true;
            index += 2;
        } else {
            index += 1;
        }
    }
    if closed
        && let Some((before, fillet, after)) =
            fillet_pair(segments.last().unwrap(), &segments[0], radius, tolerance)?
    {
        segments[0] = after;
        *segments.last_mut().unwrap() = before;
        segments.insert(0, CurveSegment3::Arc(fillet));
        changed = true;
    }
    if changed {
        Ok(Some(PolyCurve3::try_new(segments)?))
    } else {
        Ok(None)
    }
}

fn fillet_pair(
    before: &CurveSegment3,
    after: &CurveSegment3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<(CurveSegment3, CircularArc3, CurveSegment3)>, GeometryError> {
    let (curve, arc, reversed) = match (before, after) {
        (CurveSegment3::NurbsCurve(curve), CurveSegment3::Arc(arc)) => (curve.clone(), *arc, false),
        (CurveSegment3::Arc(arc), CurveSegment3::NurbsCurve(curve)) => {
            (curve.reversed()?, arc.reversed(tolerance)?, true)
        }
        _ => return Ok(None),
    };
    if straight_leaf_vertices(if reversed { after } else { before })?.is_some()
        || !arc_line::sharp_joint(before, after, tolerance)?
    {
        return Ok(None);
    }
    let solution = solve_nurbs_then_arc(&curve, arc, radius, tolerance)?;
    if reversed {
        Ok(Some((
            CurveSegment3::Arc(solution.arc.reversed(tolerance)?),
            solution.fillet.reversed(tolerance)?,
            CurveSegment3::NurbsCurve(solution.curve.reversed()?),
        )))
    } else {
        Ok(Some((
            CurveSegment3::NurbsCurve(solution.curve),
            solution.fillet,
            CurveSegment3::Arc(solution.arc),
        )))
    }
}

fn solve_nurbs_then_arc(
    curve: &NurbsCurve,
    arc: CircularArc3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<NurbsArcSolution, GeometryError> {
    let normal = arc.normal()?;
    let curve_normal = nurbs_offset_plane(curve, normal, tolerance)?;
    if curve_normal
        .as_vector()
        .cross(normal.as_vector())?
        .length()?
        > tolerance.angular()
    {
        return Err(unsupported_curved_corner());
    }
    let spans = curve.spans().collect::<Vec<_>>();
    if spans.len().saturating_mul(SAMPLES_PER_SPAN) > MAX_ROOT_SAMPLES {
        return Err(GeometryError::InvalidPolyCurve {
            context: "too many NURBS fillet search intervals",
        });
    }
    let search = Search {
        curve,
        arc,
        radius,
        tolerance,
        normal: normal.as_vector(),
    };
    let mut best: Option<NurbsArcSolution> = None;
    for side in [-1.0, 1.0] {
        for signed_radius in [arc.radius() + radius, arc.radius() - radius] {
            if signed_radius == 0.0 {
                continue;
            }
            for &(span_start, span_end) in &spans {
                let mut previous_parameter = span_start;
                let mut previous_value = search.residual(span_start, side, signed_radius)?;
                for index in 1..=SAMPLES_PER_SPAN {
                    let fraction = index as Real / SAMPLES_PER_SPAN as Real;
                    let parameter = (span_end - span_start).mul_add(fraction, span_start);
                    let value = search.residual(parameter, side, signed_radius)?;
                    let root = if previous_value == 0.0 {
                        Some(previous_parameter)
                    } else if value == 0.0 {
                        Some(parameter)
                    } else if previous_value.is_sign_negative() != value.is_sign_negative() {
                        Some(search.bisect(
                            previous_parameter,
                            parameter,
                            previous_value,
                            side,
                            signed_radius,
                        )?)
                    } else {
                        None
                    };
                    if let Some(root) = root
                        && let Some(solution) = search.candidate_at(root, side, signed_radius)?
                        && best.as_ref().is_none_or(|previous| {
                            solution.removed_length < previous.removed_length
                        })
                    {
                        best = Some(solution);
                    }
                    previous_parameter = parameter;
                    previous_value = value;
                }
            }
        }
    }
    best.ok_or_else(unsupported_curved_corner)
}

impl Search<'_> {
    fn offset_sample(
        &self,
        parameter: Real,
        side: Real,
    ) -> Result<(Point3, Vector3, Point3), GeometryError> {
        let sample = CurveRef::NurbsCurve(self.curve).evaluate_with_tangent_on_side(
            parameter,
            if parameter == *self.curve.domain().start() {
                ParameterSide::Right
            } else {
                ParameterSide::Left
            },
        )?;
        let tangent = sample.tangent().as_vector();
        let left = self
            .normal
            .cross(tangent)?
            .normalized_nonzero()?
            .as_vector();
        let center = sample
            .point()
            .translated(left.scaled(side * self.radius)?)?;
        Ok((sample.point(), tangent, center))
    }

    fn residual(
        &self,
        parameter: Real,
        side: Real,
        signed_radius: Real,
    ) -> Result<Real, GeometryError> {
        let (_, _, center) = self.offset_sample(parameter, side)?;
        Ok(self.arc.center().distance_to(center)? - signed_radius.abs())
    }

    fn bisect(
        &self,
        mut low: Real,
        mut high: Real,
        mut low_value: Real,
        side: Real,
        signed_radius: Real,
    ) -> Result<Real, GeometryError> {
        let high_value = self.residual(high, side, signed_radius)?;
        let mut best = if low_value.abs() <= high_value.abs() {
            low
        } else {
            high
        };
        let mut best_error = low_value.abs().min(high_value.abs());
        for _ in 0..64 {
            let middle = low + (high - low) * 0.5;
            if middle == low || middle == high {
                break;
            }
            let value = self.residual(middle, side, signed_radius)?;
            if value.abs() < best_error {
                best = middle;
                best_error = value.abs();
            }
            if value == 0.0 {
                break;
            }
            if value.is_sign_negative() == low_value.is_sign_negative() {
                low = middle;
                low_value = value;
            } else {
                high = middle;
            }
        }
        Ok(best)
    }

    fn candidate_at(
        &self,
        parameter: Real,
        side: Real,
        signed_radius: Real,
    ) -> Result<Option<NurbsArcSolution>, GeometryError> {
        if !(parameter > *self.curve.domain().start() && parameter < *self.curve.domain().end()) {
            return Ok(None);
        }
        let (point, tangent, center) = self.offset_sample(parameter, side)?;
        let residual = self.arc.center().distance_to(center)? - signed_radius.abs();
        let scale = point
            .distance_to(self.arc.center())?
            .max(self.radius)
            .max(1.0);
        if residual.abs() > (self.tolerance.absolute() * 0.1).max(64.0 * Real::EPSILON * scale)
            || point.distance_to(self.curve.evaluate(*self.curve.domain().end())?)?
                <= self.tolerance.absolute()
        {
            return Ok(None);
        }
        let radial = self
            .arc
            .center()
            .vector_to(center)?
            .scaled(1.0 / signed_radius)?;
        let arc_contact = self
            .arc
            .center()
            .translated(radial.scaled(self.arc.radius())?)?;
        let arc_parameter =
            CurveRef::Arc(&self.arc).closest_parameter(arc_contact, self.tolerance)?;
        if !(arc_parameter > *self.arc.domain().start() && arc_parameter < *self.arc.domain().end())
        {
            return Ok(None);
        }
        let exact_contact = self.arc.evaluate(arc_parameter)?;
        if exact_contact.distance_to(arc_contact)? > self.tolerance.absolute() * 0.1
            || exact_contact.distance_to(self.arc.start()?)? <= self.tolerance.absolute()
        {
            return Ok(None);
        }
        let arc_tangent = CurveRef::Arc(&self.arc)
            .evaluate_with_tangent_on_side(arc_parameter, ParameterSide::Right)?
            .tangent()
            .as_vector();
        let Some(fillet) = arc_line::tangent_fillet_arc(
            center,
            point,
            exact_contact,
            tangent,
            arc_tangent,
            self.radius,
            self.tolerance,
        )?
        else {
            return Ok(None);
        };
        let removed_length = self
            .curve
            .try_trimmed(parameter..=*self.curve.domain().end())?
            .length(self.tolerance)?
            + self.arc.radius()
                * self.arc.sweep_radians()
                * ((arc_parameter - *self.arc.domain().start())
                    / (*self.arc.domain().end() - *self.arc.domain().start()));
        Ok(Some(NurbsArcSolution {
            curve: self
                .curve
                .try_trimmed(*self.curve.domain().start()..=parameter)?,
            fillet,
            arc: self
                .arc
                .try_trimmed(arc_parameter..=*self.arc.domain().end())?,
            removed_length,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }

    #[test]
    fn quadratic_nurbs_and_arc_keep_native_leaves_and_tangent_fillet() {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2., 0.), p(2., 2.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let arc = CircularArc3::try_from_three_points(
            p(2., 2.),
            p(2. + 2_f64.sqrt(), 4. - 2_f64.sqrt()),
            p(4., 4.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for segments in [
            vec![
                CurveSegment3::NurbsCurve(curve.clone()),
                CurveSegment3::Arc(arc),
            ],
            vec![
                CurveSegment3::Arc(arc.reversed(Tolerance::DEFAULT).unwrap()),
                CurveSegment3::NurbsCurve(curve.reversed().unwrap()),
            ],
        ] {
            let source = PolyCurve3::try_new(segments).unwrap();
            let result = source.try_fillet_corners(0.5, Tolerance::DEFAULT).unwrap();
            assert_eq!(result.segments().len(), 3);
            let CurveSegment3::Arc(fillet) = result.segments()[1] else {
                panic!("middle leaf is the fillet")
            };
            assert!((fillet.radius() - 0.5).abs() < 1e-8);
            for pair in result.segments().windows(2) {
                assert!(!arc_line::sharp_joint(&pair[0], &pair[1], Tolerance::DEFAULT).unwrap());
            }
        }
    }
}
