//! Segment correspondence and singular corner fans for ordinary polyline ribbons.
use super::corner::{RibbonCornerStyle, smooth_corner};
mod assembly;
mod correspondence;
use crate::curve_offset::{polyline_offset_corner_arc, polyline_offset_layout};
use crate::{
    Brep, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, Polyline3, Real, Tolerance,
    UnitVector3, WeightedPoint3, join_breps,
};
use assembly::{Patch, VertexKey};
use correspondence::SegmentClock;

const MAX_PATCHES: usize = 4096;

/// Build a planar polyline's ordinary offset ribbon, retaining each original
/// segment's correspondence. Convex gaps get a round or chamfer fan; concave
/// corners use the offset-line intersection. None leaves gaps open while
/// retaining shared source vertices. BothSides sews at original source edges.
///
/// U spans [0,abs(distance)]; segment V charts retain the source parameters and
/// fan V charts follow their offset curve. Signed distance chooses the first
/// side. Smooth corners use cubic tangent fans. Sharp seam clips can omit
/// extra fans when both incident strips extend at other corners. Returned
/// containers can include shells connected only at declared corner vertices.
/// This does not trim self-intersections or implement command/object policies.
pub fn try_polyline_ribbon(
    source: &Polyline3,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    corner: impl Into<RibbonCornerStyle>,
) -> Result<Vec<Brep>, GeometryError> {
    try_polyline_ribbon_with_ruled(
        source,
        distance,
        both_sides,
        normal,
        tolerance,
        corner,
        &mut NurbsSurface::try_ruled_from_curves,
    )
}

/// Delegate the ruled tensors' degree/knot edits to a geometry backend.
#[allow(clippy::too_many_arguments)]
pub fn try_polyline_ribbon_with_ruled<E: From<GeometryError>>(
    source: &Polyline3,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    corner: impl Into<RibbonCornerStyle>,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Vec<Brep>, E> {
    let corner = corner.into();
    if !distance.is_finite() || distance == 0. {
        return Err(GeometryError::InvalidCurveOffsetDistance.into());
    }
    let sides = if both_sides { 2 } else { 1 };
    // A strip, two inner triangles and an outer fan per segment per side.
    if source.segment_count() > MAX_PATCHES / (4 * sides) {
        return Err(GeometryError::RibbonResourceLimit {
            maximum: MAX_PATCHES,
        }
        .into());
    }
    let mut patches = Vec::new();
    for (side, d) in [distance, -distance].into_iter().take(sides).enumerate() {
        let layout = polyline_offset_layout(source, d, normal, tolerance, corner.layout_style())?;
        let omit_seam_fans = corner == RibbonCornerStyle::Sharp
            && source.is_closed()
            && layout.outside_corners[1]
            && layout.outside_corners[layout.corners.len() - 1];
        let source_key = |i| Some(VertexKey::Source(i));
        let offset_key = |i: usize, end| {
            Some(VertexKey::Offset {
                side,
                corner: i,
                end: if layout.corners[i].0 == layout.corners[i].1 {
                    0
                } else {
                    end
                },
            })
        };
        for (i, segment) in source.segments().enumerate() {
            let next = (i + 1) % layout.corners.len();
            let domain = source.parameters()[i]..=source.parameters()[i + 1];
            let (start, end) = (layout.corners[i].1, layout.corners[next].0);
            let inside = [layout.inside_corners[i], layout.inside_corners[next]];
            let clipped = if corner == RibbonCornerStyle::Sharp {
                [i, next].map(|j| {
                    correspondence::sharp_clip_at(
                        j,
                        &layout.inside_corners,
                        &layout.outside_corners,
                        source.is_closed(),
                    )
                })
            } else {
                inside
            };
            let clock = if corner != RibbonCornerStyle::Sharp {
                SegmentClock::Projected
            } else if clipped.into_iter().any(|v| v) {
                SegmentClock::Clipped {
                    distance: d,
                    normal: layout.normal,
                }
            } else {
                SegmentClock::Original
            };
            let fractions = correspondence::fractions(
                segment,
                [start, end],
                inside,
                clipped,
                clock,
                tolerance,
            )?;
            if fractions[0] >= fractions[1] {
                return Err(GeometryError::Degenerate {
                    context: "overlapping inner ribbon corners",
                }
                .into());
            }
            let points = [
                segment.point_at(fractions[0])?,
                segment.point_at(fractions[1])?,
            ];
            let parameters = fractions.map(|f| {
                if f == 0. {
                    *domain.start()
                } else if f == 1. {
                    *domain.end()
                } else {
                    (domain.end() - domain.start()).mul_add(f, *domain.start())
                }
            });
            if fractions[0] > 0. && !(omit_seam_fans && i == 0) {
                let source = LineSegment::try_new(segment.start(), points[0], tolerance)?
                    .try_reparameterized(*domain.start()..=parameters[0])?
                    .to_nurbs()?;
                patches.push(patch(
                    &source,
                    &constant_with_basis(start, &source)?,
                    distance.abs(),
                    tolerance,
                    [source_key(i), offset_key(i, 1), offset_key(i, 1), None],
                    ruled,
                )?);
            }
            let original = LineSegment::try_new(points[0], points[1], tolerance)?
                .try_reparameterized(parameters[0]..=parameters[1])?
                .to_nurbs()?;
            let offset = LineSegment::try_new(start, end, tolerance)?
                .try_reparameterized(original.domain())?
                .to_nurbs()?;
            patches.push(patch(
                &original,
                &offset,
                distance.abs(),
                tolerance,
                [
                    if fractions[0] == 0. {
                        source_key(i)
                    } else {
                        None
                    },
                    offset_key(i, 1),
                    offset_key(next, 0),
                    if fractions[1] == 1. {
                        source_key(next)
                    } else {
                        None
                    },
                ],
                ruled,
            )?);
            if fractions[1] < 1. && !(omit_seam_fans && next == 0) {
                let source = LineSegment::try_new(points[1], segment.end(), tolerance)?
                    .try_reparameterized(parameters[1]..=*domain.end())?
                    .to_nurbs()?;
                patches.push(patch(
                    &source,
                    &constant_with_basis(end, &source)?,
                    distance.abs(),
                    tolerance,
                    [
                        None,
                        offset_key(next, 0),
                        offset_key(next, 0),
                        source_key(next),
                    ],
                    ruled,
                )?);
            }
            let (start, end) = layout.corners[next];
            if start == end || corner == RibbonCornerStyle::None {
                continue;
            }
            let center = source.vertices()[next];
            let offset = match corner {
                RibbonCornerStyle::Round => {
                    polyline_offset_corner_arc(center, start, end, layout.normal, tolerance)?
                        .to_nurbs()?
                }
                RibbonCornerStyle::Smooth => smooth_corner(
                    start,
                    end,
                    segment.direction(tolerance)?,
                    source.vertices()[next].direction_to(source.vertices()[next + 1])?,
                )?,
                RibbonCornerStyle::Chamfer => {
                    LineSegment::try_new(start, end, tolerance)?.to_nurbs()?
                }
                _ => unreachable!("sharp offset layouts have no corner gaps"),
            };
            // Keep the offset's rational clock across the fan. A unit-weight
            // constant row would change every interior radial isocurve.
            let pole = constant_with_basis(center, &offset)?;
            patches.push(patch(
                &pole,
                &offset,
                distance.abs(),
                tolerance,
                [
                    source_key(next),
                    offset_key(next, 0),
                    offset_key(next, 1),
                    source_key(next),
                ],
                ruled,
            )?);
        }
    }
    Ok(assembly::sew(patches, tolerance)?)
}

fn constant_with_basis(point: Point3, basis: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new_rational(
        basis.degree(),
        basis
            .control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(point, p.weight()))
            .collect::<Result<Vec<_>, _>>()?,
        basis.knots().to_vec(),
    )
}

fn patch<E: From<GeometryError>>(
    a: &NurbsCurve,
    b: &NurbsCurve,
    width: Real,
    tolerance: Tolerance,
    keys: [Option<VertexKey>; 4],
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Patch, E> {
    Ok(Patch {
        brep: Brep::try_surface_face(
            ruled(a, b)?.try_reparameterized(0. ..=width, a.domain())?,
            tolerance,
        )?,
        keys,
    })
}

#[cfg(test)]
mod tests;
