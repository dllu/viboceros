//! Native span construction and initial length integration.

use super::numerical_distance_tolerance;
use crate::{
    CurveRef, GeometryError, Real, Tolerance, integration::integrate_adaptive, require_finite,
};
use std::f64::consts::FRAC_PI_2;

#[derive(Clone, Copy, Debug)]
pub(super) enum LinearSpan {
    Line,
    Polyline(usize),
    CompositeLine(usize),
    CompositePolyline(usize, usize),
}

pub(super) type RawSpan = (Real, Real, Real, bool, Option<LinearSpan>);

pub(super) fn raw_spans(
    curve: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Vec<RawSpan>, GeometryError> {
    Ok(match curve {
        CurveRef::Line(line) => vec![(
            *line.domain().start(),
            *line.domain().end(),
            line.length()?,
            false,
            Some(LinearSpan::Line),
        )],
        CurveRef::Circle(circle) => {
            let quadrant_length = circle.length()? * 0.25;
            (0..4)
                .map(|quadrant| {
                    Ok((
                        curve.parameter_at(quadrant as Real * 0.25)?,
                        curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        quadrant_length,
                        false,
                        None,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Arc(arc) => vec![(
            *arc.domain().start(),
            *arc.domain().end(),
            arc.length()?,
            false,
            None,
        )],
        CurveRef::Ellipse(ellipse) => {
            let quadrant_length = integrate_speed(0.0, FRAC_PI_2, tolerance, |angle| {
                let (sine, cosine) = angle.sin_cos();
                let speed = (ellipse.radius_x() * sine).hypot(ellipse.radius_y() * cosine);
                require_finite([speed], "ellipse speed")?;
                Ok(speed)
            })?;
            (0..4)
                .map(|quadrant| {
                    Ok((
                        curve.parameter_at(quadrant as Real * 0.25)?,
                        curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        quadrant_length,
                        true,
                        None,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Polyline(polyline) => polyline
            .segments()
            .enumerate()
            .map(|(index, segment)| {
                Ok((
                    polyline.parameters()[index],
                    polyline.parameters()[index + 1],
                    segment.length()?,
                    false,
                    Some(LinearSpan::Polyline(index)),
                ))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::NurbsCurve(curve) => curve
            .spans()
            .map(|(start, end)| {
                let length = integrate_speed(start, end, tolerance, |parameter| {
                    curve.derivative_at(parameter)?.length()
                })?;
                Ok((start, end, length, true, None))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::PolyCurve(curve) => {
            let mut spans = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                for (start, end, length, variable_speed, linear) in
                    raw_spans(segment.as_ref(), tolerance)?
                {
                    spans.push((
                        curve.polycurve_parameter(index, start)?,
                        curve.polycurve_parameter(index, end)?,
                        length,
                        variable_speed,
                        linear.map(|linear| match linear {
                            LinearSpan::Line => LinearSpan::CompositeLine(index),
                            LinearSpan::Polyline(edge) => {
                                LinearSpan::CompositePolyline(index, edge)
                            }
                            _ => unreachable!("polycurve leaves are not nested composites"),
                        }),
                    ));
                }
            }
            spans
        }
    })
}

fn integrate_speed(
    start: Real,
    end: Real,
    tolerance: Tolerance,
    mut speed: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let coarse = integrate_adaptive(
        start,
        end,
        tolerance.absolute(),
        tolerance.relative(),
        &mut speed,
    )?;
    let tighter = numerical_distance_tolerance(coarse, tolerance);
    if tighter < tolerance.absolute() {
        integrate_adaptive(start, end, tighter, tolerance.relative(), speed)
    } else {
        Ok(coarse)
    }
}
