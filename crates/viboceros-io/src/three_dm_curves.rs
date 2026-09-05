//! Prepare unsupported full-order NURBS joins for the stricter 3DM curve model.

use crate::{ThreeDmError, ThreeDmGeometry};
use std::borrow::Cow;
use viboceros_geometry::{CurveSegment3, MAX_POLYCURVE_SEGMENTS, NurbsCurve, PolyCurve3};

#[cfg(test)]
mod tests;

struct Piece {
    curve: CurveSegment3,
    start: f64,
    end: f64,
    // Original polycurve junctions already satisfy its coincidence predicate.
    source_junction: bool,
}

pub(crate) fn prepare(
    geometry: &ThreeDmGeometry,
) -> Result<Vec<Cow<'_, ThreeDmGeometry>>, ThreeDmError> {
    let pieces = match geometry {
        ThreeDmGeometry::NurbsCurve(curve) if needs_decomposition(curve) => curve
            .try_split_at_full_order_knots()?
            .into_iter()
            .map(|curve| Piece {
                start: *curve.domain().start(),
                end: *curve.domain().end(),
                curve: curve.into(),
                source_junction: false,
            })
            .collect(),
        ThreeDmGeometry::PolyCurve(curve)
            if curve.segments().iter().any(
                |segment| matches!(segment, CurveSegment3::NurbsCurve(c) if needs_decomposition(c)),
            ) =>
        {
            let mut pieces = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                if let CurveSegment3::NurbsCurve(c) = segment
                    && needs_decomposition(c)
                {
                    for (part, c) in c.try_split_at_full_order_knots()?.into_iter().enumerate() {
                        pieces.push(Piece {
                            start: curve.polycurve_parameter(index, *c.domain().start())?,
                            end: curve.polycurve_parameter(index, *c.domain().end())?,
                            curve: c.into(),
                            source_junction: part == 0,
                        });
                    }
                } else {
                    pieces.push(Piece {
                        curve: segment.clone(),
                        start: curve.parameters()[index],
                        end: curve.parameters()[index + 1],
                        source_junction: true,
                    });
                }
            }
            pieces
        }
        _ => return Ok(vec![Cow::Borrowed(geometry)]),
    };
    group_pieces(pieces).map(|pieces| pieces.into_iter().map(Cow::Owned).collect())
}

fn needs_decomposition(curve: &NurbsCurve) -> bool {
    curve.full_order_knots().next().is_some()
}

fn group_pieces(pieces: Vec<Piece>) -> Result<Vec<ThreeDmGeometry>, ThreeDmError> {
    let mut result = Vec::new();
    let mut group: Vec<CurveSegment3> = Vec::new();
    let mut parameters: Vec<f64> = Vec::new();
    for piece in pieces {
        if piece.start >= piece.end {
            return Err(ThreeDmError::InvalidModel(
                "decomposed curve parameter span collapsed".into(),
            ));
        }
        let closed = piece.curve.is_closed()?;
        if let Some(previous) = group.last() {
            let touching = piece.source_junction
                || previous.evaluate(*previous.domain().end())?
                    == piece.curve.evaluate(*piece.curve.domain().start())?;
            if !touching
                || closed
                || previous.is_closed()?
                || group.len() == MAX_POLYCURVE_SEGMENTS
                || !(piece.end - parameters[0]).is_finite()
            {
                finish_group(&mut result, &mut group, &mut parameters)?;
            }
        }
        if group.is_empty() {
            parameters.push(piece.start);
        }
        parameters.push(piece.end);
        group.push(piece.curve);
    }
    finish_group(&mut result, &mut group, &mut parameters)?;
    Ok(result)
}

fn finish_group(
    result: &mut Vec<ThreeDmGeometry>,
    group: &mut Vec<CurveSegment3>,
    parameters: &mut Vec<f64>,
) -> Result<(), ThreeDmError> {
    if group.is_empty() {
        return Ok(());
    }
    let mut segments = std::mem::take(group);
    let parameters = std::mem::take(parameters);
    if segments.len() == 1 {
        let segment = segments.pop().expect("one segment exists");
        // A piece lifted out of a polycurve must retain its outer interval.
        let domain = parameters[0]..=parameters[1];
        let segment = if segment.domain() == domain {
            segment
        } else {
            segment.try_reparameterized(domain)?
        };
        result.push(match segment {
            CurveSegment3::Line(c) => ThreeDmGeometry::Line(c),
            CurveSegment3::Arc(c) => ThreeDmGeometry::Arc(c),
            CurveSegment3::Polyline(c) => ThreeDmGeometry::Polyline(c),
            CurveSegment3::NurbsCurve(c) => ThreeDmGeometry::NurbsCurve(c),
        });
    } else {
        result.push(ThreeDmGeometry::PolyCurve(
            PolyCurve3::try_with_segment_domains(segments, parameters)?,
        ));
    }
    Ok(())
}
