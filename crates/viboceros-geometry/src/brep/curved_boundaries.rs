//! Conservative certificates for curved UV boundary loops.

use super::*;
use crate::exact_scalar::{Rational, rational};

type ExactPoint = [Rational; 2];
type Bounds = [Real; 4]; // min x, max x, min y, max y

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sense {
    Clockwise,
    Counterclockwise,
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
    let center = std::array::from_fn(|axis| {
        corners.iter().map(|point| &point[axis]).sum::<Rational>() / rational(spans as Real)
    });
    let zero = rational(0.);
    let first = cross(&corners[0], &corners[1], &corners[2]);
    if first == zero {
        return None;
    }
    let sense = if first > zero {
        Sense::Counterclockwise
    } else {
        Sense::Clockwise
    };
    let oriented = |value: Rational| match sense {
        Sense::Counterclockwise => value > zero,
        Sense::Clockwise => value < zero,
    };
    (0..spans)
        .all(|i| {
            let next = (i + 1) % spans;
            // Every other endpoint must lie strictly on the interior side of
            // every directed edge. Local turns alone accept star polygons.
            let convex = (0..spans).all(|j| {
                j == i || j == next || oriented(cross(&corners[i], &corners[next], &corners[j]))
            });
            let mut previous_progress = zero.clone();
            let full_progress = progress(&corners[i], &corners[next], &corners[next]);
            convex
                && (i * degree + 1..(i + 1) * degree).all(|index| {
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
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point2 {
        Point2::try_new(x, y).unwrap()
    }

    fn polygon(points: &[[Real; 2]]) -> Vec<BrepTrim> {
        (0..points.len())
            .map(|i| {
                let next = (i + 1) % points.len();
                BrepTrim::try_new(
                    [i, next],
                    Some(i),
                    false,
                    NurbsCurve2::try_line(
                        point(points[i][0], points[i][1]),
                        point(points[next][0], points[next][1]),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )
                .unwrap()
            })
            .collect()
    }

    fn outer() -> Vec<BrepTrim> {
        polygon(&[[0., 0.], [10., 0.], [10., 10.], [0., 10.]])
    }

    fn hole(cx: Real, cy: Real, radius: Real) -> Vec<BrepTrim> {
        let controls = [
            (1., 0.),
            (1., -1.),
            (0., -1.),
            (-1., -1.),
            (-1., 0.),
            (-1., 1.),
            (0., 1.),
            (1., 1.),
            (1., 0.),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            WeightedPoint2::try_new(
                point(cx + radius * x, cy + radius * y),
                if i % 2 == 0 { 1. } else { 0.75 },
            )
            .unwrap()
        })
        .collect();
        vec![
            BrepTrim::try_new(
                [100, 100],
                Some(100),
                false,
                NurbsCurve2::try_new_rational(
                    2,
                    controls,
                    vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.],
                )
                .unwrap(),
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [0.; 2],
            )
            .unwrap(),
        ]
    }

    fn curved_outer() -> Vec<BrepTrim> {
        let mut boundary = hole(5., 5., 4.);
        boundary[0].curve = boundary[0].curve.reversed().unwrap();
        boundary
    }

    fn closed_bezier(degree: usize, points: &[[Real; 2]], knots: Vec<Real>) -> Vec<BrepTrim> {
        vec![
            BrepTrim::try_new(
                [100, 100],
                Some(100),
                false,
                NurbsCurve2::try_new(
                    degree,
                    points.iter().map(|p| point(p[0], p[1])).collect(),
                    knots,
                )
                .unwrap(),
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [0.; 2],
            )
            .unwrap(),
        ]
    }

    fn cubic_outer() -> Vec<BrepTrim> {
        closed_bezier(
            3,
            &[
                [8., 5.],
                [8., 7.],
                [6., 9.],
                [3., 8.],
                [1., 7.],
                [1., 3.],
                [3., 2.],
                [6., 1.],
                [8., 3.],
                [8., 5.],
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 2., 2., 2., 3., 3., 3., 3.],
        )
    }

    fn six_span_outer() -> Vec<BrepTrim> {
        closed_bezier(
            2,
            &[
                [8., 5.],
                [8., 7.],
                [7., 8.],
                [5., 9.],
                [3., 8.],
                [2., 7.],
                [2., 5.],
                [2., 3.],
                [3., 2.],
                [5., 1.],
                [7., 2.],
                [8., 3.],
                [8., 5.],
            ],
            vec![
                0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 6., 6., 6.,
            ],
        )
    }

    fn face(boundaries: Vec<Vec<BrepTrim>>) -> Result<BrepFace, GeometryError> {
        let surface = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(10., 0., 0.).unwrap(),
                Point3::try_new(0., 10., 0.).unwrap(),
                Point3::try_new(10., 10., 0.).unwrap(),
            ],
            vec![0., 0., 10., 10.],
            vec![0., 0., 10., 10.],
        )
        .unwrap();
        BrepFace::try_from_certified_boundaries(surface, false, boundaries)
    }

    #[test]
    fn accepts_closed_quadratic_holes_without_changing_inner_order() {
        let first = hole(3., 3., 1.);
        let second = hole(7., 7., 1.);
        let polygon_hole = polygon(&[[6., 2.], [6., 3.], [7., 3.], [7., 2.]]);
        let built = face(vec![
            first.clone(),
            outer(),
            polygon_hole.clone(),
            second.clone(),
        ])
        .unwrap();
        assert_eq!(built.loops.len(), 4);
        assert_eq!(built.loops[0].trims, outer());
        assert_eq!(built.loops[1].trims, first);
        assert_eq!(built.loops[2].trims, polygon_hole);
        assert_eq!(built.loops[3].trims, second);
    }

    #[test]
    fn rejects_uncertified_or_intersecting_curved_holes() {
        assert!(face(vec![outer(), hole(9.5, 5., 1.)]).is_err());
        assert!(face(vec![outer(), hole(3., 3., 1.), hole(4., 3., 1.)]).is_err());
        let mut crossing = hole(5., 5., 1.);
        let mut controls = crossing[0].curve.control_points().to_vec();
        controls[1] = WeightedPoint2::try_new(point(4., 6.), 0.75).unwrap();
        crossing[0].curve =
            NurbsCurve2::try_new_rational(2, controls, crossing[0].curve.knots().to_vec()).unwrap();
        assert!(face(vec![outer(), crossing]).is_err());
        let concave = polygon(&[[0., 0.], [10., 0.], [10., 10.], [5., 5.], [0., 10.]]);
        assert!(face(vec![concave, hole(5., 3., 1.)]).is_err());
    }

    #[test]
    fn curved_holes_use_every_segment_of_a_polyline_outer() {
        let mut outer = outer();
        outer[0].curve = NurbsCurve2::try_new(
            1,
            vec![point(0., 0.), point(5., -1.), point(10., 0.)],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        assert!(face(vec![outer.clone(), hole(5., 3., 0.4)]).is_ok());
        outer[0].curve = NurbsCurve2::try_new(
            1,
            vec![point(0., 0.), point(5., 6.), point(10., 0.)],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        assert!(face(vec![outer, hole(5., 3., 0.4)]).is_err());
    }

    #[test]
    fn accepts_curved_outer_with_separate_polygon_and_curved_holes() {
        let outer = curved_outer();
        let polygon_hole = polygon(&[[6., 4.5], [6., 5.5], [7., 5.5], [7., 4.5]]);
        let curved_hole = hole(4., 5., 0.4);
        let built = face(vec![
            polygon_hole.clone(),
            outer.clone(),
            curved_hole.clone(),
        ])
        .unwrap();
        assert_eq!(built.loops.len(), 3);
        assert_eq!(built.loops[0].trims, outer);
        assert_eq!(built.loops[1].trims, polygon_hole);
        assert_eq!(built.loops[2].trims, curved_hole);
    }

    #[test]
    fn rejects_curved_outer_without_inscribed_polygon_certificate() {
        let outer = curved_outer();
        assert!(face(vec![outer.clone(), outer.clone()]).is_err());
        let outside_virtual = polygon(&[[8., 6.], [8., 7.], [9., 7.], [9., 6.]]);
        assert!(face(vec![outer.clone(), outside_virtual]).is_err());
        let mut inward = outer;
        let mut controls = inward[0].curve.control_points().to_vec();
        controls[1] = WeightedPoint2::try_new(point(6., 6.), 0.75).unwrap();
        inward[0].curve =
            NurbsCurve2::try_new_rational(2, controls, inward[0].curve.knots().to_vec()).unwrap();
        assert!(face(vec![inward]).is_err());
    }

    #[test]
    fn accepts_cubic_and_six_span_outer_loops() {
        let inner = polygon(&[[4.5, 4.5], [4.5, 5.5], [5.5, 5.5], [5.5, 4.5]]);
        for outer in [cubic_outer(), six_span_outer()] {
            let built = face(vec![inner.clone(), outer.clone()]).unwrap();
            assert_eq!(built.loops[0].trims, outer);
            assert_eq!(built.loops[1].trims, inner);
        }
    }

    #[test]
    fn rejects_star_endpoint_polygon_despite_consistent_local_turns() {
        let corners = [[5., 0.], [8., 10.], [0., 4.], [10., 4.], [2., 10.]];
        let mut points = Vec::new();
        for index in 0..corners.len() {
            let a = corners[index];
            let b = corners[(index + 1) % corners.len()];
            points.push(a);
            points.push([(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.]);
        }
        points.push(corners[0]);
        let star = closed_bezier(
            2,
            &points,
            vec![0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 5.],
        );
        assert!(certified_bezier_loop(&star[0]).is_none());
    }

    #[test]
    fn rejects_cubic_span_with_reversed_chord_progress() {
        let mut boundary = cubic_outer();
        let mut controls = boundary[0].curve.control_points().to_vec();
        controls.swap(1, 2);
        boundary[0].curve =
            NurbsCurve2::try_new_rational(3, controls, boundary[0].curve.knots().to_vec()).unwrap();
        assert!(certified_bezier_loop(&boundary[0]).is_none());
    }
}
