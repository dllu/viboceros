//! Curve representation simplification through the public OpenNURBS predicates.
use crate::{
    Circle3, CircularArc3, Curve3, CurveSegment3, GeometryError, LineSegment, NurbsCurve, Point3,
    PolyCurve3, Polyline3, Real, Tolerance, Vector3,
};
use std::ops::RangeInclusive;
use viboceros_opennurbs::AnalyticCurve;
const MAX_CONTROLS: usize = 1_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct CurveSimplification {
    pub curve: Curve3,
    /// Structural reconstruction, distinct from constant-weight normalization.
    pub rebuilt_representation: bool,
}
#[derive(Clone)]
struct Part {
    segment: CurveSegment3,
    domain: RangeInclusive<Real>,
}

pub fn simplify_curve(
    curve: &Curve3,
    tolerance: Tolerance,
) -> Result<CurveSimplification, GeometryError> {
    simplify_curve_with_split(curve, tolerance, &mut |c, t| c.try_split(t))
}
/// Native builds supply the existing usd-brep splitter; recognition and command
/// policy are shared, without maintaining a second knot-editing implementation.
pub fn simplify_curve_with_split<E: From<GeometryError>>(
    curve: &Curve3,
    tolerance: Tolerance,
    split: &mut impl FnMut(&NurbsCurve, Real) -> Result<(NurbsCurve, NurbsCurve), E>,
) -> Result<CurveSimplification, E> {
    if matches!(curve, Curve3::Line(_) | Curve3::Circle(_) | Curve3::Arc(_)) {
        return Ok(CurveSimplification {
            curve: curve.clone(),
            rebuilt_representation: false,
        });
    }
    let domain = curve.as_ref().domain();
    let mut parts = Vec::new();
    let mut work = 0;
    decompose(
        curve,
        domain.clone(),
        tolerance,
        split,
        &mut parts,
        &mut work,
    )?;
    let mut merged: Vec<Part> = Vec::with_capacity(parts.len());
    for part in parts {
        if let Some(previous) = merged.last_mut()
            && let Some(segment) = merge_pair(
                &previous.segment,
                &part.segment,
                *previous.domain.start()..=*part.domain.end(),
                tolerance,
            )?
        {
            previous.segment = segment;
            previous.domain = *previous.domain.start()..=*part.domain.end();
            continue;
        }
        merged.push(part);
    }
    let mut parts = Vec::new();
    let mut i = 0;
    while i < merged.len() {
        let start = i;
        i += 1;
        if linear(&merged[start].segment) {
            while i < merged.len() && linear(&merged[i].segment) {
                i += 1;
            }
        }
        if i - start > 1 {
            let first = &merged[start];
            let mut points = Vec::new();
            let mut parameters = Vec::new();
            for part in &merged[start..i] {
                let (vertices, local) = match &part.segment {
                    CurveSegment3::Line(c) => (
                        vec![c.start(), c.end()],
                        vec![*c.domain().start(), *c.domain().end()],
                    ),
                    CurveSegment3::Polyline(c) => (c.vertices().to_vec(), c.parameters().to_vec()),
                    _ => unreachable!(),
                };
                let skip = usize::from(!points.is_empty());
                points.extend(vertices.into_iter().skip(skip));
                for parameter in local.into_iter().skip(skip) {
                    parameters.push(crate::parameter::map_parameter(
                        parameter,
                        part.segment.domain(),
                        part.domain.clone(),
                    )?);
                }
            }
            let polyline = Polyline3::try_with_parameters(
                points,
                parameters,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
            parts.push(Part {
                segment: simplify_polyline(&polyline, tolerance)?,
                domain: *first.domain.start()..=*merged[i - 1].domain.end(),
            });
        } else {
            parts.push(merged[start].clone());
        }
    }
    let mut result = if parts.len() == 1 {
        parts
            .pop()
            .unwrap()
            .segment
            .try_reparameterized(domain)?
            .into_curve()
    } else {
        let mut parameters = vec![*domain.start()];
        parameters.extend(parts.iter().map(|p| *p.domain.end()));
        Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
            parts.into_iter().map(|p| p.segment).collect(),
            parameters,
        )?)
    };
    if result != *curve
        && let Curve3::PolyCurve(c) = &result
    {
        result = Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
            c.segments()
                .iter()
                .enumerate()
                .map(|(i, s)| s.try_reparameterized(c.segment_domain(i)?))
                .collect::<Result<Vec<_>, GeometryError>>()?,
            c.parameters().to_vec(),
        )?);
    }
    let rebuilt_representation = result != *curve
        && !matches!(
            (curve, &result),
            (Curve3::NurbsCurve(_), Curve3::NurbsCurve(_))
        );
    Ok(CurveSimplification {
        curve: result,
        rebuilt_representation,
    })
}
fn push<E: From<GeometryError>>(parts: &mut Vec<Part>, part: Part) -> Result<(), E> {
    if parts.len() >= crate::polycurve::MAX_POLYCURVE_SEGMENTS {
        return Err(GeometryError::InvalidPolyCurve {
            context: "curve simplification segment limit",
        }
        .into());
    }
    parts.push(part);
    Ok(())
}
fn decompose<E: From<GeometryError>>(
    curve: &Curve3,
    outer: RangeInclusive<Real>,
    tolerance: Tolerance,
    split: &mut impl FnMut(&NurbsCurve, Real) -> Result<(NurbsCurve, NurbsCurve), E>,
    parts: &mut Vec<Part>,
    work: &mut usize,
) -> Result<(), E> {
    let map = |t| crate::parameter::map_parameter(t, curve.as_ref().domain(), outer.clone());
    match curve {
        Curve3::PolyCurve(c) => {
            for (i, segment) in c.segments().iter().enumerate() {
                let d = c.segment_domain(i)?;
                decompose(
                    &segment.clone().into_curve(),
                    map(*d.start())?..=map(*d.end())?,
                    tolerance,
                    split,
                    parts,
                    work,
                )?;
            }
        }
        Curve3::Polyline(c) => {
            *work =
                work.checked_add(c.vertices().len())
                    .ok_or(GeometryError::InvalidPolyCurve {
                        context: "curve simplification work limit",
                    })?;
            if *work > MAX_CONTROLS {
                return Err(GeometryError::InvalidPolyCurve {
                    context: "curve simplification work limit",
                }
                .into());
            }
            push(
                parts,
                Part {
                    segment: simplify_polyline(c, tolerance)?,
                    domain: outer,
                },
            )?;
        }
        Curve3::NurbsCurve(c) => {
            *work = work.checked_add(c.control_points().len()).ok_or(
                GeometryError::InvalidPolyCurve {
                    context: "curve simplification work limit",
                },
            )?;
            if *work > MAX_CONTROLS {
                return Err(GeometryError::InvalidPolyCurve {
                    context: "curve simplification work limit",
                }
                .into());
            }
            if let Some(segment) = recognize(c, tolerance)? {
                push(
                    parts,
                    Part {
                        segment,
                        domain: outer,
                    },
                )?;
                return Ok(());
            }
            if c.degree() == 1
                && c.knots()[1..=c.control_points().len()]
                    .windows(2)
                    .all(|p| p[0] < p[1])
                && c.control_points()
                    .windows(2)
                    .all(|p| p[0].point() != p[1].point())
            {
                let polyline = Polyline3::try_with_parameters(
                    c.control_points().iter().map(|p| p.point()).collect(),
                    c.knots()[1..=c.control_points().len()].to_vec(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                push(
                    parts,
                    Part {
                        segment: simplify_polyline(&polyline, tolerance)?,
                        domain: outer,
                    },
                )?;
                return Ok(());
            }
            let range = c.domain();
            let mut breaks = Vec::new();
            let mut i = c.degree() + 1;
            while i < c.control_points().len() {
                let t = c.knots()[i];
                let start = i;
                while i < c.knots().len() && c.knots()[i] == t {
                    i += 1;
                }
                if i - start >= c.degree() && t > *range.start() && t < *range.end() {
                    breaks.push(t);
                }
            }
            if breaks.is_empty() {
                push(
                    parts,
                    Part {
                        segment: CurveSegment3::NurbsCurve(normalized_weights(c.clone())?),
                        domain: outer,
                    },
                )?;
                return Ok(());
            }
            let mut remainder = c.clone();
            for t in breaks {
                *work = work.checked_add(remainder.control_points().len()).ok_or(
                    GeometryError::InvalidPolyCurve {
                        context: "curve simplification work limit",
                    },
                )?;
                if *work > MAX_CONTROLS {
                    return Err(GeometryError::InvalidPolyCurve {
                        context: "curve simplification work limit",
                    }
                    .into());
                }
                let (left, right) = split(&remainder, t)?;
                let d = left.domain();
                let left = normalized_weights(left)?;
                let segment =
                    recognize(&left, tolerance)?.unwrap_or(CurveSegment3::NurbsCurve(left));
                push(
                    parts,
                    Part {
                        segment,
                        domain: map(*d.start())?..=map(*d.end())?,
                    },
                )?;
                remainder = right;
            }
            let d = remainder.domain();
            let remainder = normalized_weights(remainder)?;
            let segment =
                recognize(&remainder, tolerance)?.unwrap_or(CurveSegment3::NurbsCurve(remainder));
            push(
                parts,
                Part {
                    segment,
                    domain: map(*d.start())?..=map(*d.end())?,
                },
            )?;
        }
        Curve3::Ellipse(_) => decompose(
            &Curve3::NurbsCurve(curve.as_ref().to_nurbs()?),
            outer,
            tolerance,
            split,
            parts,
            work,
        )?,
        _ => push(
            parts,
            Part {
                segment: CurveSegment3::try_from_curve(curve)?,
                domain: outer,
            },
        )?,
    }
    Ok(())
}
fn normalized_weights(curve: NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    if curve
        .control_points()
        .iter()
        .all(|p| p.weight() == curve.control_points()[0].weight())
    {
        return NurbsCurve::try_new(
            curve.degree(),
            curve.control_points().iter().map(|p| p.point()).collect(),
            curve.knots().to_vec(),
        );
    }
    Ok(curve)
}
fn recognize(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<CurveSegment3>, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|p| {
            let [x, y, z] = p.point().to_array();
            [x, y, z, p.weight()]
        })
        .collect::<Vec<_>>();
    let result = viboceros_opennurbs::curve_analytic_form(
        curve.degree(),
        &controls,
        curve.knots(),
        tolerance.absolute(),
    )?;
    Ok(match result {
        None => None,
        Some(AnalyticCurve::Line { .. }) => Some(CurveSegment3::Line(
            LineSegment::try_new(
                curve.evaluate(*curve.domain().start())?,
                curve.evaluate(*curve.domain().end())?,
                Tolerance::NUMERICAL_VALIDATION,
            )?
            .try_reparameterized(curve.domain())?,
        )),
        Some(AnalyticCurve::Arc {
            center,
            x,
            normal,
            radius,
            angles,
        }) => {
            let circle = Circle3::try_from_frame(
                Point3::try_from(center)?,
                radius,
                Vector3::try_from(x)?.normalized_nonzero()?,
                Vector3::try_from(normal)?.normalized_nonzero()?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
            Some(CurveSegment3::Arc(
                CircularArc3::try_from_circle_angles(circle, angles[0]..=angles[1])?
                    .try_reparameterized(curve.domain())?,
            ))
        }
    })
}
fn merge_pair(
    a: &CurveSegment3,
    b: &CurveSegment3,
    domain: RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<Option<CurveSegment3>, GeometryError> {
    if !matches!(
        (a, b),
        (CurveSegment3::Line(_), CurveSegment3::Line(_))
            | (CurveSegment3::Arc(_), CurveSegment3::Arc(_))
    ) {
        return Ok(None);
    }
    let composite = PolyCurve3::try_new(vec![a.clone(), b.clone()])?;
    let mut result = recognize(&composite.to_nurbs()?, tolerance)?;
    if let Some(segment) = &mut result {
        *segment = segment.try_reparameterized(domain)?;
    }
    Ok(result)
}

fn linear(segment: &CurveSegment3) -> bool {
    matches!(segment, CurveSegment3::Line(_) | CurveSegment3::Polyline(_))
}
fn simplify_polyline(
    source: &Polyline3,
    tolerance: Tolerance,
) -> Result<CurveSegment3, GeometryError> {
    let points = source
        .vertices()
        .iter()
        .map(|p| p.to_array())
        .collect::<Vec<_>>();
    let retained = viboceros_opennurbs::polyline_simplify_indices(&points, tolerance.absolute())?;
    if retained.len() == 2 {
        return Ok(CurveSegment3::Line(
            LineSegment::try_new(
                source.vertices()[0],
                *source.vertices().last().unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )?
            .try_reparameterized(source.domain())?,
        ));
    }
    Ok(CurveSegment3::Polyline(Polyline3::try_with_parameters(
        retained.iter().map(|&i| source.vertices()[i]).collect(),
        retained.iter().map(|&i| source.parameters()[i]).collect(),
        Tolerance::NUMERICAL_VALIDATION,
    )?))
}

#[cfg(test)]
mod tests;
