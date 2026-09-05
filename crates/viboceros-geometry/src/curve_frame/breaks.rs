use super::*;

// Lower-multiplicity NURBS knots are structurally position-continuous. Their
// two evaluation paths can round to neighboring floats; that is not a jump.
pub(super) fn position_can_jump(curve: CurveRef<'_>, t: Real) -> Result<bool, GeometryError> {
    Ok(match curve {
        CurveRef::NurbsCurve(c) => {
            let knots = c.knots();
            knots.partition_point(|k| *k <= t) - knots.partition_point(|k| *k < t) > c.degree()
                && *c.domain().start() < t
                && t < *c.domain().end()
        }
        CurveRef::PolyCurve(c) => {
            let left = c.segment_index(t, ParameterSide::Left)?;
            let right = c.segment_index(t, ParameterSide::Right)?;
            left != right
                || position_can_jump(c.segments()[right].as_ref(), c.segment_parameter(right, t)?)?
        }
        _ => false,
    })
}

pub(super) fn parameters(
    curve: CurveRef<'_>,
    start: Real,
    end: Real,
    maximum: usize,
) -> Result<Vec<Real>, GeometryError> {
    let mut values = Vec::new();
    collect(curve, &mut values, maximum)?;
    values.retain(|t| start < *t && *t < end);
    values.sort_by(Real::total_cmp);
    values.dedup();
    Ok(values)
}

fn collect(curve: CurveRef<'_>, out: &mut Vec<Real>, maximum: usize) -> Result<(), GeometryError> {
    let mut span = |a: Real, b: Real, count: usize| -> Result<(), GeometryError> {
        if out.len().saturating_add(count + 1) > maximum {
            return Err(GeometryError::CurveFrameResourceLimit { maximum });
        }
        for i in 0..=count {
            let f = i as Real / count as Real;
            out.push(if i == 0 {
                a
            } else if i == count {
                b
            } else {
                a * (1.0 - f) + b * f
            });
        }
        Ok(())
    };
    match curve {
        CurveRef::NurbsCurve(c) => {
            for pair in c.knots()[c.degree()..=c.control_points().len()].windows(2) {
                if pair[0] < pair[1] {
                    span(pair[0], pair[1], c.degree().max(2))?;
                }
            }
        }
        CurveRef::Polyline(c) => {
            for pair in c.parameters().windows(2) {
                span(pair[0], pair[1], 1)?;
            }
        }
        CurveRef::PolyCurve(c) => {
            for (i, segment) in c.segments().iter().enumerate() {
                let mut child = Vec::new();
                collect(segment.as_ref(), &mut child, maximum)?;
                if out.len().saturating_add(child.len()) > maximum {
                    return Err(GeometryError::CurveFrameResourceLimit { maximum });
                }
                for t in child {
                    out.push(c.polycurve_parameter(i, t)?);
                }
            }
        }
        CurveRef::Ellipse(c) => collect(CurveRef::NurbsCurve(&c.to_nurbs()?), out, maximum)?,
        CurveRef::Circle(_) | CurveRef::Arc(_) => {
            span(*curve.domain().start(), *curve.domain().end(), 8)?
        }
        CurveRef::Line(_) => span(*curve.domain().start(), *curve.domain().end(), 1)?,
    }
    Ok(())
}
