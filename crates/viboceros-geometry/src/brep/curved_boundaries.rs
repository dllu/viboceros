//! Conservative certificates for curved UV boundary loops.

use super::*;
use crate::exact_scalar::{Rational, rational};
mod convexity;

type ExactPoint = [Rational; 2];
type Bounds = [Real; 4]; // min x, max x, min y, max y

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sense {
    Clockwise,
    Counterclockwise,
}

/// Sector separation and strict chord progress certify a simple closed loop.
/// This is independent of the stronger nesting certificate below.
pub(super) fn simple_loop_orientation(curve: &NurbsCurve2) -> Option<bool> {
    let trim = BrepTrim::try_new(
        [0, 0],
        Some(0),
        false,
        curve.clone(),
        BrepTrimType::Boundary,
        SurfaceIso::NotIso,
        [0.; 2],
    )
    .ok()?;
    certified_bezier_loop(&trim).map(|sense| sense == Sense::Counterclockwise)
}

impl BrepFace {
    /// Constructs polygonal boundaries with optional certified Bézier loops.
    ///
    /// A curved loop must be one closed rational NURBS curve with at least
    /// three Bézier spans. Its span endpoints form a strictly convex polygon.
    /// Each intermediate control lies strictly inside its angular sector
    /// about the endpoint centroid and advances along its endpoint chord.
    /// Same-sign weights keep each arc in that sector; strict chord progress
    /// prevents a span from retracing or crossing itself. A curved outer
    /// loop must bow outside its endpoint polygon; holes must lie
    /// strictly inside that polygon or a convex polygonal outer loop.
    /// Disjoint control bounds separate holes. Other curves are rejected.
    pub fn try_from_certified_boundaries(
        surface: NurbsSurface,
        reversed: bool,
        boundaries: Vec<Vec<BrepTrim>>,
    ) -> Result<Self, GeometryError> {
        let invalid = || GeometryError::InvalidPlanarFaceBoundary;
        let mut polygon_ids = Vec::new();
        let mut polygons = Vec::new();
        let mut curved = Vec::new();
        for (id, boundary) in boundaries.into_iter().enumerate() {
            if boundary.iter().all(|trim| {
                trim.curve.is_straight_segment()
                    || polygon_boundaries::has_control_polygon_image(&trim.curve)
            }) {
                polygon_ids.push(id);
                polygons.push(boundary);
            } else if boundary.len() == 1 {
                let sense = certified_bezier_loop(&boundary[0]).ok_or_else(invalid)?;
                curved.push((id, boundary, sense));
            } else {
                return Err(invalid());
            }
        }
        if curved
            .iter()
            .filter(|entry| entry.2 == Sense::Counterclockwise)
            .count()
            > 1
        {
            return Err(invalid());
        }
        let curved_outer = curved
            .iter()
            .position(|entry| entry.2 == Sense::Counterclockwise)
            .map(|index| curved.remove(index));
        let mut checked_polygons =
            Vec::with_capacity(polygons.len() + usize::from(curved_outer.is_some()));
        let mut checked_ids = Vec::with_capacity(checked_polygons.capacity());
        if let Some((_, outer, _)) = &curved_outer {
            if !outer_bows_outward(&outer[0]) {
                return Err(invalid());
            }
            checked_polygons.push(endpoint_polygon(&outer[0])?);
            checked_ids.push(None);
        }
        for (id, polygon) in polygon_ids.iter().copied().zip(&polygons) {
            checked_polygons.push(polygon.clone());
            checked_ids.push(Some(id));
        }
        let mut face =
            Self::try_from_polygon_boundaries(surface, reversed, checked_polygons.clone())?;
        if curved.is_empty() && curved_outer.is_none() {
            return Ok(face);
        }
        let outer = &face.loops[0].trims;
        let outer_points = polygon_boundaries::polygon_points(outer)?
            .into_iter()
            .map(exact_point)
            .collect::<Vec<_>>();
        let zero = rational(0.);
        if (0..outer_points.len()).any(|i| {
            cross(
                &outer_points[i],
                &outer_points[(i + 1) % outer_points.len()],
                &outer_points[(i + 2) % outer_points.len()],
            ) <= zero
        }) {
            return Err(invalid());
        }
        let mut occupied = face.loops[1..]
            .iter()
            .map(|loop_| {
                bounds(loop_.trims.iter().flat_map(|trim| {
                    trim.curve
                        .control_points()
                        .iter()
                        .map(|control| control.point())
                }))
            })
            .collect::<Vec<_>>();
        for (_, boundary, _) in &curved {
            let controls = boundary[0].curve.control_points();
            for control in controls {
                let point = exact_point(control.point());
                if (0..outer_points.len()).any(|i| {
                    cross(
                        &outer_points[i],
                        &outer_points[(i + 1) % outer_points.len()],
                        &point,
                    ) <= zero
                }) {
                    return Err(invalid());
                }
            }
            let box_ = bounds(controls.iter().map(|control| control.point()));
            if occupied.iter().any(|other| !disjoint(box_, *other)) {
                return Err(invalid());
            }
            occupied.push(box_);
        }

        // The polygon constructor moves only its outer loop. Restore the
        // original relative order of all inner loops, including curved holes.
        let outer_polygon = checked_polygons
            .iter()
            .position(|boundary| *boundary == face.loops[0].trims)
            .ok_or_else(invalid)?;
        if curved_outer.is_some() && outer_polygon != 0 {
            return Err(invalid());
        }
        let mut loops = std::mem::take(&mut face.loops);
        let virtual_outer = loops.remove(0);
        let outer_loop = if let Some((_, boundary, _)) = curved_outer {
            BrepLoop::try_new(BrepLoopType::Outer, boundary)?
        } else {
            virtual_outer
        };
        let mut polygon_inner = loops.into_iter();
        let mut ordered = Vec::with_capacity(checked_ids.len() + curved.len() - 1);
        for (index, id) in checked_ids.into_iter().enumerate() {
            if index != outer_polygon {
                ordered.push((
                    id.ok_or_else(invalid)?,
                    polygon_inner.next().ok_or_else(invalid)?,
                ));
            }
        }
        for (id, boundary, _) in curved {
            ordered.push((id, BrepLoop::try_new(BrepLoopType::Inner, boundary)?));
        }
        ordered.sort_by_key(|entry| entry.0);
        face.loops = std::iter::once(outer_loop)
            .chain(ordered.into_iter().map(|entry| entry.1))
            .collect();
        Self::try_new(face.surface, face.reversed, face.loops)
    }
}

/// Exact Bézier structure: clamped endpoints and degree-multiplicity joins.
fn bezier_layout(curve: &NurbsCurve2) -> Option<(usize, usize)> {
    let degree = curve.degree();
    let controls = curve.control_points();
    let knots = curve.knots();
    if degree < 2 || !(controls.len() - 1).is_multiple_of(degree) {
        return None;
    }
    let spans = (controls.len() - 1) / degree;
    if spans < 3 || knots[..=degree].iter().any(|&knot| knot != knots[0]) {
        return None;
    }
    let mut offset = degree + 1;
    let mut previous = knots[0];
    for _ in 1..spans {
        let current = knots[offset];
        if current <= previous
            || knots[offset..offset + degree]
                .iter()
                .any(|&knot| knot != current)
        {
            return None;
        }
        previous = current;
        offset += degree;
    }
    let end = knots[offset];
    (end > previous && knots[offset..].iter().all(|&knot| knot == end)).then_some((degree, spans))
}

fn certified_bezier_loop(trim: &BrepTrim) -> Option<Sense> {
    let curve = &trim.curve;
    let controls = curve.control_points();
    if trim.vertices[0] != trim.vertices[1] || controls[0].point() != controls.last()?.point() {
        return None;
    }
    let (degree, spans) = bezier_layout(curve)?;
    let sign = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|control| control.weight().is_sign_positive() != sign)
    {
        return None;
    }
    let corners = (0..spans)
        .map(|index| exact_point(controls[index * degree].point()))
        .collect::<Vec<_>>();
    let sense = convexity::strict_cycle(&corners)?;
    let center = std::array::from_fn(|axis| {
        corners.iter().map(|point| &point[axis]).sum::<Rational>() / rational(spans as Real)
    });
    let zero = rational(0.);
    let oriented = |value: Rational| match sense {
        Sense::Counterclockwise => value > zero,
        Sense::Clockwise => value < zero,
    };
    (0..spans)
        .all(|i| {
            let next = (i + 1) % spans;
            let mut previous_progress = zero.clone();
            let full_progress = progress(&corners[i], &corners[next], &corners[next]);
            (i * degree + 1..(i + 1) * degree).all(|index| {
                let control = exact_point(controls[index].point());
                let next_progress = progress(&corners[i], &corners[next], &control);
                let accepted = next_progress > previous_progress
                    && next_progress < full_progress
                    && oriented(cross(&center, &corners[i], &control))
                    && oriented(cross(&center, &control, &corners[next]));
                previous_progress = next_progress;
                accepted
            })
        })
        .then_some(sense)
}

fn outer_bows_outward(trim: &BrepTrim) -> bool {
    let controls = trim.curve.control_points();
    let Some((degree, spans)) = bezier_layout(&trim.curve) else {
        return false;
    };
    let zero = rational(0.);
    (0..spans).all(|i| {
        let start = exact_point(controls[i * degree].point());
        let end = exact_point(controls[(i + 1) * degree].point());
        (i * degree + 1..(i + 1) * degree)
            .all(|index| cross(&start, &end, &exact_point(controls[index].point())) < zero)
    })
}

fn endpoint_polygon(trim: &BrepTrim) -> Result<Vec<BrepTrim>, GeometryError> {
    let controls = trim.curve.control_points();
    let (degree, spans) =
        bezier_layout(&trim.curve).ok_or(GeometryError::InvalidPlanarFaceBoundary)?;
    (0..spans)
        .map(|i| {
            BrepTrim::try_new(
                [i, (i + 1) % spans],
                Some(i),
                false,
                NurbsCurve2::try_line(
                    controls[degree * i].point(),
                    controls[degree * ((i + 1) % spans)].point(),
                )?,
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [0.; 2],
            )
        })
        .collect()
}

fn exact_point(point: Point2) -> ExactPoint {
    [rational(point.x()), rational(point.y())]
}

fn cross(a: &ExactPoint, b: &ExactPoint, c: &ExactPoint) -> Rational {
    (&b[0] - &a[0]) * (&c[1] - &a[1]) - (&b[1] - &a[1]) * (&c[0] - &a[0])
}

fn progress(a: &ExactPoint, b: &ExactPoint, point: &ExactPoint) -> Rational {
    (&point[0] - &a[0]) * (&b[0] - &a[0]) + (&point[1] - &a[1]) * (&b[1] - &a[1])
}

fn bounds(points: impl IntoIterator<Item = Point2>) -> Bounds {
    let mut result = [
        Real::INFINITY,
        Real::NEG_INFINITY,
        Real::INFINITY,
        Real::NEG_INFINITY,
    ];
    for point in points {
        result[0] = result[0].min(point.x());
        result[1] = result[1].max(point.x());
        result[2] = result[2].min(point.y());
        result[3] = result[3].max(point.y());
    }
    result
}

fn disjoint(a: Bounds, b: Bounds) -> bool {
    a[1] < b[0] || b[1] < a[0] || a[3] < b[2] || b[3] < a[2]
}

#[cfg(test)]
mod tests;
