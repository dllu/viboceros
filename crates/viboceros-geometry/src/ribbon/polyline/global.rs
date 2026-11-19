//! Preserve source intervals when a closed inward Sharp offset splits.
use super::*;
use crate::curve_offset::OffsetPolylineLayout;
use crate::{Curve3, CurveOffsetCornerStyle, parameter::map_parameter};
mod rails;

pub(super) fn build<E: From<GeometryError>>(
    source: &Polyline3,
    layout: &OffsetPolylineLayout,
    distance: Real,
    tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Option<Brep>, E> {
    let Some(rails::Trimmed { spans, single_loop }) =
        rails::trim(source, layout, distance, tolerance)?
    else {
        return Ok(None);
    };
    if spans.is_empty() {
        return Err(GeometryError::Degenerate {
            context: "inward ribbon has no surviving offset rail",
        }
        .into());
    }

    // A surviving original junction retains the full source interval there.
    // At a new junction, use the clock of the strip before any corner trims.
    let raw = polyline_offset_layout_with_trim(
        source,
        distance,
        layout.normal,
        tolerance,
        CurveOffsetCornerStyle::Sharp,
        false,
    )?;
    let count = source.segment_count();
    let mut retained_ends = vec![[false; 2]; count];
    for span in &spans {
        retained_ends[span.index][0] |= span.range[0] == 0.;
        retained_ends[span.index][1] |= span.range[1] == 1.;
    }
    let mut result = Vec::with_capacity(spans.len());
    let mut sections = Vec::with_capacity(spans.len());
    let segments = source.segments().collect::<Vec<_>>();
    for span in spans {
        let i = span.index;
        let prev = (i + count - 1) % count;
        let next = (i + 1) % count;
        let original = segments[i];
        let [a, b] = raw.segment_points(i);
        let reference = LineSegment::try_new(a, b, tolerance)?;
        let join_start = span.range[0] == 0. && retained_ends[prev][1];
        let join_end = span.range[1] == 1. && retained_ends[next][0];
        let start = if join_start {
            0.
        } else {
            reference.closest_parameter(span.points[0], tolerance)?
        };
        let end = if join_end {
            1.
        } else {
            reference.closest_parameter(span.points[1], tolerance)?
        };
        let lo = map_parameter(start, 0. ..=1., original.domain())?;
        let hi = map_parameter(end, 0. ..=1., original.domain())?;
        sections.push(([lo, hi], span.points));
        let parameter = lo..=hi;
        let unoffset = LineSegment::try_new(
            original.point_at(start)?,
            original.point_at(end)?,
            tolerance,
        )?
        .try_reparameterized(parameter.clone())?
        .to_nurbs()?;
        let offset = LineSegment::try_new(span.points[0], span.points[1], tolerance)?
            .try_reparameterized(parameter)?
            .to_nurbs()?;
        result.push(patch(
            &unoffset,
            &offset,
            distance.abs(),
            tolerance,
            [None; 4],
            ruled,
        )?);
    }
    if single_loop {
        // A single offset rail retains source portions collapsed onto a rail
        // junction. With multiple loops the missing intervals stay open.
        let domain = source.domain();
        for (i, &(current, points)) in sections.iter().enumerate() {
            let j = (i + 1) % sections.len();
            let (next, next_points) = sections[j];
            if !points[1].is_near(next_points[0], tolerance) {
                return Err(GeometryError::Degenerate {
                    context: "trimmed ribbon sections are out of order",
                }
                .into());
            }
            let intervals = if j == 0 {
                [[current[1], *domain.end()], [*domain.start(), next[0]]]
            } else {
                [[current[1], next[0]], [0., 0.]]
            };
            for [start, end] in intervals {
                if start >= end {
                    continue;
                }
                let first = source.parameters().partition_point(|v| *v <= start) - 1;
                for segment in &segments[first..] {
                    let domain = segment.domain();
                    let lo = start.max(*domain.start());
                    let hi = end.min(*domain.end());
                    if lo >= hi {
                        break;
                    }
                    let source = LineSegment::try_new(
                        segment.evaluate(lo)?,
                        segment.evaluate(hi)?,
                        tolerance,
                    )?
                    .try_reparameterized(lo..=hi)?
                    .to_nurbs()?;
                    result.push(patch(
                        &source,
                        &constant_with_basis(points[1], &source)?,
                        distance.abs(),
                        tolerance,
                        [None; 4],
                        ruled,
                    )?);
                }
            }
        }
    }
    Ok(Some(assembly::sew_sections(
        result,
        source.domain(),
        tolerance,
    )?))
}
