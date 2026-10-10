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
    CompositePolyline { segment: usize, edge: usize },
}

pub(super) struct RawSpan {
    pub(super) start: Real,
    pub(super) end: Real,
    pub(super) length: Real,
    pub(super) variable_speed: bool,
    pub(super) linear: Option<LinearSpan>,
}

pub(super) fn raw_spans(
    curve: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Vec<RawSpan>, GeometryError> {
    Ok(match curve {
        CurveRef::Line(line) => vec![RawSpan {
            start: *line.domain().start(),
            end: *line.domain().end(),
            length: line.length()?,
            variable_speed: false,
            linear: Some(LinearSpan::Line),
        }],
        CurveRef::Circle(circle) => {
            let quadrant_length = circle.length()? * 0.25;
            (0..4)
                .map(|quadrant| {
                    Ok(RawSpan {
                        start: curve.parameter_at(quadrant as Real * 0.25)?,
                        end: curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        length: quadrant_length,
                        variable_speed: false,
                        linear: None,
                    })
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Arc(arc) => vec![RawSpan {
            start: *arc.domain().start(),
            end: *arc.domain().end(),
            length: arc.length()?,
            variable_speed: false,
            linear: None,
        }],
        CurveRef::Ellipse(ellipse) => {
            let quadrant_length = integrate_speed(0.0, FRAC_PI_2, tolerance, |angle| {
                let (sine, cosine) = angle.sin_cos();
                let speed = (ellipse.radius_x() * sine).hypot(ellipse.radius_y() * cosine);
                require_finite([speed], "ellipse speed")?;
                Ok(speed)
            })?;
            (0..4)
                .map(|quadrant| {
                    Ok(RawSpan {
                        start: curve.parameter_at(quadrant as Real * 0.25)?,
                        end: curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        length: quadrant_length,
                        variable_speed: true,
                        linear: None,
                    })
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Polyline(polyline) => polyline
            .segments()
            .enumerate()
            .map(|(index, segment)| {
                Ok(RawSpan {
                    start: polyline.parameters()[index],
                    end: polyline.parameters()[index + 1],
                    length: segment.length()?,
                    variable_speed: false,
                    linear: Some(LinearSpan::Polyline(index)),
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::NurbsCurve(curve) => curve
            .spans()
            .map(|(start, end)| {
                let length = integrate_speed(start, end, tolerance, |parameter| {
                    curve.derivative_at(parameter)?.length()
                })?;
                Ok(RawSpan {
                    start,
                    end,
                    length,
                    variable_speed: true,
                    linear: None,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::PolyCurve(curve) => {
            let mut spans = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                for span in raw_spans(segment.as_ref(), tolerance)? {
                    spans.push(RawSpan {
                        start: curve.polycurve_parameter(index, span.start)?,
                        end: curve.polycurve_parameter(index, span.end)?,
                        linear: span.linear.map(|linear| match linear {
                            LinearSpan::Line => LinearSpan::CompositeLine(index),
                            LinearSpan::Polyline(edge) => LinearSpan::CompositePolyline {
                                segment: index,
                                edge,
                            },
                            _ => unreachable!("polycurve leaves are not nested composites"),
                        }),
                        ..span
                    });
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
