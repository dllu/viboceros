//! Representation, parameter domains, and exact segment assembly.
use super::*;

pub(super) fn is_linear(curve: &Curve3) -> bool {
    match curve {
        Curve3::Line(_) | Curve3::Polyline(_) => true,
        Curve3::NurbsCurve(curve) => {
            curve.degree() == 1
                && curve.knots()[1..curve.knots().len() - 1]
                    .windows(2)
                    .all(|p| p[0] < p[1])
        }
        _ => false,
    }
}

pub(super) fn assemble(
    curves: &[Curve3],
    chain: &[(usize, bool)],
    ends: &[Option<[usize; 2]>],
    endpoints: &[Endpoint],
    partners: &[Option<usize>],
    policy: AssemblyPolicy,
    validation: Tolerance,
) -> Result<Curve3, GeometryError> {
    let mut parts = Vec::with_capacity(chain.len());
    let mut linear_points = Vec::new();
    let mut linear_parameters: Vec<Real> = Vec::new();
    let seed = chain
        .iter()
        .map(|(index, _)| *index)
        .min()
        .expect("an assembly has sources");
    let mut seed_offset = 0.0;
    let mut seed_parameter = 0.0;
    for &(source, reversed) in chain {
        let source_ends = ends[source].expect("assembled curves are open");
        let mut targets = [None; 2];
        for side in 0..2 {
            let endpoint = source_ends[side];
            if let Some(partner) = partners[endpoint] {
                let first = endpoints[endpoint];
                let second = endpoints[partner];
                let first_arc = endpoint_is_arc(&curves[first.curve], first.start);
                let second_arc = endpoint_is_arc(&curves[second.curve], second.start);
                let point = if first_arc && second_arc {
                    first.point.midpoint(second.point)?
                } else if first_arc {
                    first.point
                } else if second_arc {
                    second.point
                } else {
                    first.point.midpoint(second.point)?
                };
                targets[side] = Some(point);
            }
        }
        if policy.all_linear {
            let polyline = linear_form(&curves[source], validation)?;
            let mut points = polyline.vertices().to_vec();
            if let Some(point) = targets[0] {
                points[0] = point;
            }
            if let Some(point) = targets[1] {
                *points.last_mut().expect("a polyline has vertices") = point;
            }
            if reversed {
                points.reverse();
            }
            let parameters = if reversed {
                polyline
                    .parameters()
                    .iter()
                    .rev()
                    .map(|t| -t)
                    .collect::<Vec<_>>()
            } else {
                polyline.parameters().to_vec()
            };
            if linear_parameters.is_empty() {
                linear_parameters.push(parameters[0]);
            }
            if source == seed {
                seed_offset = parameters[0] - linear_parameters.last().unwrap();
                seed_parameter = *linear_parameters.last().unwrap();
            }
            for pair in parameters.windows(2) {
                linear_parameters.push(linear_parameters.last().unwrap() + (pair[1] - pair[0]));
            }
            let skip = usize::from(!linear_points.is_empty());
            linear_points.extend_from_slice(&points[skip..]);
        } else {
            let mut part = match &curves[source] {
                Curve3::Arc(arc) => {
                    Curve3::Arc(arc.try_with_endpoints(targets[0], targets[1], validation)?)
                        .to_polycurve()?
                }
                curve => curve
                    .to_polycurve()?
                    .try_with_endpoints(targets[0], targets[1])?,
            };
            if reversed {
                part = part.reversed()?;
            }
            if source == seed {
                let start = parts
                    .first()
                    .map_or(*part.domain().start(), |curve: &PolyCurve3| {
                        *curve.domain().start()
                    });
                let preceding = parts
                    .iter()
                    .map(|curve| curve.domain().end() - curve.domain().start())
                    .sum::<Real>();
                seed_offset = *part.domain().start() - (start + preceding);
                seed_parameter = start + preceding;
            }
            parts.push(part);
        }
    }
    if policy.all_linear {
        let curve = if policy.linear_batch {
            Polyline3::try_new(linear_points, validation)?.try_chord_length_parameterized()?
        } else {
            Polyline3::try_with_parameters(
                linear_points,
                linear_parameters
                    .into_iter()
                    .map(|t| {
                        if policy.style == CurveJoinStyle::Seeded {
                            t + seed_offset
                        } else {
                            t
                        }
                    })
                    .collect(),
                validation,
            )?
        };
        let curve = Curve3::Polyline(curve);
        if policy.style == CurveJoinStyle::Batch
            && !policy.linear_batch
            && curve.as_ref().is_closed()?
        {
            curve.try_change_closed_seam(seed_parameter)
        } else {
            Ok(curve)
        }
    } else {
        let curve = PolyCurve3::concatenate(&parts)?;
        let curve = if policy.style == CurveJoinStyle::Seeded && seed_offset != 0.0 {
            PolyCurve3::try_with_segment_domains(
                curve.segments().to_vec(),
                curve.parameters().iter().map(|t| t + seed_offset).collect(),
            )?
        } else {
            curve
        };
        let curve = Curve3::PolyCurve(curve);
        let curve = if policy.style == CurveJoinStyle::Batch && curve.as_ref().is_closed()? {
            curve.try_change_closed_seam(seed_parameter)?
        } else {
            curve
        };
        let Curve3::PolyCurve(curve) = curve else {
            unreachable!()
        };
        let curve = merge_linear_runs(&curve, validation)?;
        if policy.style == CurveJoinStyle::Batch || !curve.is_closed()? {
            Ok(Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
                curve
                    .segments()
                    .iter()
                    .enumerate()
                    .map(|(i, s)| s.try_reparameterized(curve.segment_domain(i)?))
                    .collect::<Result<Vec<_>, _>>()?,
                curve.parameters().to_vec(),
            )?))
        } else {
            Ok(Curve3::PolyCurve(curve))
        }
    }
}

/// Merge adjacent exact line/polyline leaves without changing parent speeds.
pub(super) fn merge_linear_runs(
    curve: &PolyCurve3,
    validation: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    use crate::CurveSegment3;
    let linear =
        |s: &CurveSegment3| matches!(s, CurveSegment3::Line(_) | CurveSegment3::Polyline(_));
    let mut segments = Vec::new();
    let mut parameters = vec![curve.parameters()[0]];
    let mut i = 0;
    while i < curve.segments().len() {
        let start = i;
        i += 1;
        if linear(&curve.segments()[start]) {
            while i < curve.segments().len() && linear(&curve.segments()[i]) {
                i += 1;
            }
        }
        if i == start + 1 {
            segments.push(curve.segments()[start].clone());
        } else {
            let origin = *curve.segments()[start].as_ref().domain().start();
            let parent_start = curve.parameters()[start];
            let mut points = Vec::new();
            let mut times = Vec::new();
            for j in start..i {
                let part = linear_form(&curve.segments()[j].as_ref().to_owned(), validation)?;
                let skip = usize::from(j != start);
                for (&point, &t) in part.vertices().iter().zip(part.parameters()).skip(skip) {
                    points.push(point);
                    times.push(origin + (curve.polycurve_parameter(j, t)? - parent_start));
                }
            }
            segments.push(CurveSegment3::Polyline(Polyline3::try_with_parameters(
                points, times, validation,
            )?));
        }
        parameters.push(curve.parameters()[i]);
    }
    PolyCurve3::try_with_segment_domains(segments, parameters)
}

fn endpoint_is_arc(curve: &Curve3, start: bool) -> bool {
    match curve {
        Curve3::Arc(_) => true,
        Curve3::PolyCurve(curve) => matches!(
            if start {
                curve.segments().first()
            } else {
                curve.segments().last()
            },
            Some(crate::CurveSegment3::Arc(_))
        ),
        _ => false,
    }
}

pub(super) fn linear_form(
    curve: &Curve3,
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    match curve {
        Curve3::Line(line) => Polyline3::try_with_parameters(
            vec![line.start(), line.end()],
            vec![*line.domain().start(), *line.domain().end()],
            tolerance,
        ),
        Curve3::Polyline(line) => Ok(line.clone()),
        Curve3::NurbsCurve(curve) => Polyline3::try_with_parameters(
            curve
                .control_points()
                .iter()
                .map(|point| point.point())
                .collect(),
            curve.knots()[1..curve.knots().len() - 1].to_vec(),
            tolerance,
        ),
        _ => unreachable!("linear form is only used for line/polyline inputs"),
    }
}
