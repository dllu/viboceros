//! Segment correspondence and singular corner fans for ordinary polyline ribbons.
use crate::curve_offset::{polyline_offset_corner_arc, polyline_offset_layout};
use crate::{
    Brep, CurveOffsetCornerStyle, GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3,
    Polyline3, Real, Tolerance, UnitVector3, WeightedPoint3, join_breps,
};

const MAX_PATCHES: usize = 4096;

/// Build a planar polyline's ordinary offset ribbon, retaining each original
/// segment's correspondence. Convex gaps get a round or chamfer fan; concave
/// corners use the offset-line intersection. None leaves gaps open and can
/// return several connected shells. BothSides sews at original source edges.
///
/// U spans [0,abs(distance)]; segment V charts retain the source parameters and
/// fan V charts follow their offset curve. Signed distance chooses the first
/// side. This does not trim self-intersections, support Smooth corners, or
/// implement the Ribbon command's getters and object policies.
pub fn try_polyline_ribbon(
    source: &Polyline3,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    corner: CurveOffsetCornerStyle,
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
    corner: CurveOffsetCornerStyle,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Vec<Brep>, E> {
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
    for d in [distance, -distance].into_iter().take(sides) {
        let layout = polyline_offset_layout(source, d, normal, tolerance, corner)?;
        for (i, segment) in source.segments().enumerate() {
            let next = (i + 1) % layout.corners.len();
            let domain = source.parameters()[i]..=source.parameters()[i + 1];
            let (start, end) = (layout.corners[i].1, layout.corners[next].0);
            let fractions = if corner == CurveOffsetCornerStyle::Sharp {
                [0., 1.]
            } else {
                [
                    if layout.inside_corners[i] {
                        segment.closest_parameter(start, tolerance)?
                    } else {
                        0.
                    },
                    if layout.inside_corners[next] {
                        segment.closest_parameter(end, tolerance)?
                    } else {
                        1.
                    },
                ]
            };
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
            if fractions[0] > 0. {
                let source = LineSegment::try_new(segment.start(), points[0], tolerance)?
                    .try_reparameterized(*domain.start()..=parameters[0])?
                    .to_nurbs()?;
                patches.push(patch(
                    &source,
                    &constant_with_basis(start, &source)?,
                    distance.abs(),
                    tolerance,
                    ruled,
                )?);
            }
            let original = LineSegment::try_new(points[0], points[1], tolerance)?
                .try_reparameterized(parameters[0]..=parameters[1])?
                .to_nurbs()?;
            let offset = LineSegment::try_new(start, end, tolerance)?
                .try_reparameterized(original.domain())?
                .to_nurbs()?;
            patches.push(patch(&original, &offset, distance.abs(), tolerance, ruled)?);
            if fractions[1] < 1. {
                let source = LineSegment::try_new(points[1], segment.end(), tolerance)?
                    .try_reparameterized(parameters[1]..=*domain.end())?
                    .to_nurbs()?;
                patches.push(patch(
                    &source,
                    &constant_with_basis(end, &source)?,
                    distance.abs(),
                    tolerance,
                    ruled,
                )?);
            }
            let (start, end) = layout.corners[next];
            if start == end || corner == CurveOffsetCornerStyle::None {
                continue;
            }
            let center = source.vertices()[next];
            let offset = match corner {
                CurveOffsetCornerStyle::Round => {
                    polyline_offset_corner_arc(center, start, end, layout.normal, tolerance)?
                        .to_nurbs()?
                }
                CurveOffsetCornerStyle::Chamfer => {
                    LineSegment::try_new(start, end, tolerance)?.to_nurbs()?
                }
                _ => unreachable!("sharp offset layouts have no corner gaps"),
            };
            // Keep the offset's rational clock across the fan. A unit-weight
            // constant row would change every interior radial isocurve.
            let pole = constant_with_basis(center, &offset)?;
            patches.push(patch(&pole, &offset, distance.abs(), tolerance, ruled)?);
        }
    }
    Ok(join_breps(
        &patches.iter().collect::<Vec<_>>(),
        tolerance.absolute(),
        tolerance,
    )?
    .into_iter()
    .map(|joined| joined.brep)
    .collect())
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
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    Ok(Brep::try_surface_face(
        ruled(a, b)?.try_reparameterized(0. ..=width, a.domain())?,
        tolerance,
    )?)
}

#[cfg(test)]
mod tests;
