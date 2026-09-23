//! Conservative certificates for curved holes inside polygonal UV boundaries.

use super::*;
use crate::exact_scalar::{Rational, rational};

type ExactPoint = [Rational; 2];
type Bounds = [Real; 4]; // min x, max x, min y, max y

impl BrepFace {
    /// Constructs polygonal boundaries with optional certified quadratic holes.
    ///
    /// A curved hole must be one closed rational quadratic curve with four
    /// Bézier spans. Its four span endpoints form a strictly convex clockwise
    /// quadrilateral. Each intermediate control lies strictly inside the
    /// corresponding angular sector about the endpoint centroid. Same-sign
    /// weights keep each arc within that sector, proving a simple closed loop.
    /// Curved holes must have disjoint control bounds and lie strictly inside a
    /// convex polygonal outer boundary. Other curves are rejected.
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
            if boundary.iter().all(|trim| trim.curve.is_straight_segment()) {
                polygon_ids.push(id);
                polygons.push(boundary);
            } else if boundary.len() == 1 && certified_quadratic_hole(&boundary[0]) {
                curved.push((id, boundary));
            } else {
                return Err(invalid());
            }
        }
        let mut face = Self::try_from_polygon_boundaries(surface, reversed, polygons.clone())?;
        if curved.is_empty() {
            return Ok(face);
        }
        let outer = &face.loops[0].trims;
        let outer_points = outer
            .iter()
            .map(|trim| trim.curve.start_point().map(exact_point))
            .collect::<Result<Vec<_>, _>>()?;
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
        for (_, boundary) in &curved {
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
        let outer_polygon = polygons
            .iter()
            .position(|boundary| *boundary == face.loops[0].trims)
            .ok_or_else(invalid)?;
        let mut loops = std::mem::take(&mut face.loops);
        let outer_loop = loops.remove(0);
        let mut polygon_inner = loops.into_iter();
        let mut ordered = Vec::with_capacity(polygon_ids.len() + curved.len() - 1);
        for (index, id) in polygon_ids.into_iter().enumerate() {
            if index != outer_polygon {
                ordered.push((id, polygon_inner.next().ok_or_else(invalid)?));
            }
        }
        for (id, boundary) in curved {
            ordered.push((id, BrepLoop::try_new(BrepLoopType::Inner, boundary)?));
        }
        ordered.sort_by_key(|entry| entry.0);
        face.loops = std::iter::once(outer_loop)
            .chain(ordered.into_iter().map(|entry| entry.1))
            .collect();
        Self::try_new(face.surface, face.reversed, face.loops)
    }
}

fn certified_quadratic_hole(trim: &BrepTrim) -> bool {
    let curve = &trim.curve;
    let controls = curve.control_points();
    let knots = curve.knots();
    if trim.vertices[0] != trim.vertices[1]
        || curve.degree() != 2
        || controls.len() != 9
        || !(knots[0] == knots[1]
            && knots[1] == knots[2]
            && knots[2] < knots[3]
            && knots[3] == knots[4]
            && knots[4] < knots[5]
            && knots[5] == knots[6]
            && knots[6] < knots[7]
            && knots[7] == knots[8]
            && knots[8] < knots[9]
            && knots[9] == knots[10]
            && knots[10] == knots[11])
        || controls[0].point() != controls[8].point()
    {
        return false;
    }
    let sign = controls[0].weight().is_sign_positive();
    if controls
        .iter()
        .any(|control| control.weight().is_sign_positive() != sign)
    {
        return false;
    }
    let corners = [0, 2, 4, 6].map(|index| exact_point(controls[index].point()));
    let center = std::array::from_fn(|axis| {
        corners.iter().map(|point| &point[axis]).sum::<Rational>() / rational(4.)
    });
    let zero = rational(0.);
    (0..4).all(|i| {
        let next = (i + 1) % 4;
        let after = (i + 2) % 4;
        let middle = exact_point(controls[2 * i + 1].point());
        cross(&corners[i], &corners[next], &corners[after]) < zero
            && cross(&center, &corners[i], &middle) < zero
            && cross(&center, &middle, &corners[next]) < zero
    })
}

fn exact_point(point: Point2) -> ExactPoint {
    [rational(point.x()), rational(point.y())]
}

fn cross(a: &ExactPoint, b: &ExactPoint, c: &ExactPoint) -> Rational {
    (&b[0] - &a[0]) * (&c[1] - &a[1]) - (&b[1] - &a[1]) * (&c[0] - &a[0])
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
}
