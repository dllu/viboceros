use super::*;

impl BrepFace {
    /// Constructs a face from unordered, simple polygon boundaries in surface UV.
    ///
    /// Each trim must be a single degree-one span with same-sign weights.
    /// Exactly one boundary must
    /// wind counterclockwise; all others must be clockwise, strictly inside it,
    /// mutually disjoint and unnested. Touching and numerically unresolved
    /// boundaries are rejected. Source curves, edge references and winding are
    /// preserved; only the outer loop is moved to the front. Model-space and
    /// shared-topology validation still requires `Brep::try_new`.
    pub fn try_from_polygon_boundaries(
        surface: NurbsSurface,
        reversed: bool,
        boundaries: Vec<Vec<BrepTrim>>,
    ) -> Result<Self, GeometryError> {
        let invalid = || GeometryError::InvalidPlanarFaceBoundary;
        let mut points = Vec::new();
        let mut ranges = Vec::new();
        for boundary in &boundaries {
            if boundary.len() < 3 {
                return Err(invalid());
            }
            let start = points.len();
            for trim in boundary {
                if trim.curve.degree() != 1 || trim.curve.control_points().len() != 2 {
                    return Err(invalid());
                }
                let controls = trim.curve.control_points();
                if controls[0].weight().is_sign_positive()
                    != controls[1].weight().is_sign_positive()
                {
                    return Err(invalid());
                }
                points.push(trim.curve.start_point()?);
            }
            ranges.push(start..points.len());
        }
        let normalization =
            TrimParameterNormalization::try_from_points(&points)?.ok_or_else(invalid)?;
        let points = points
            .into_iter()
            .map(|p| normalization.normalize(p))
            .collect::<Result<Vec<_>, _>>()?;
        let epsilon = 64.0 * Real::EPSILON;
        let mut outer = None;
        for (loop_index, (boundary, range)) in boundaries.iter().zip(&ranges).enumerate() {
            let polygon = &points[range.clone()];
            let loop_scale = polygon
                .iter()
                .flat_map(|point| {
                    [
                        (point[0] - polygon[0][0]).abs(),
                        (point[1] - polygon[0][1]).abs(),
                    ]
                })
                .fold(0.0, Real::max);
            if loop_scale <= epsilon {
                return Err(invalid());
            }
            let local = |point: [Real; 2]| {
                [
                    (point[0] - polygon[0][0]) / loop_scale,
                    (point[1] - polygon[0][1]) / loop_scale,
                ]
            };
            let mut area = 0.0;
            let mut correction = 0.0;
            for (i, trim) in boundary.iter().enumerate() {
                let j = (i + 1) % polygon.len();
                let end = normalization.normalize(trim.curve.end_point()?)?;
                if trim.vertices[1] != boundary[j].vertices[0]
                    || (end[0] - polygon[j][0]).abs() > epsilon
                    || (end[1] - polygon[j][1]).abs() > epsilon
                    || (polygon[i][0] - polygon[j][0]).hypot(polygon[i][1] - polygon[j][1])
                        <= epsilon
                {
                    return Err(invalid());
                }
                neumaier_add(
                    &mut area,
                    &mut correction,
                    polygon_cross([0.0; 2], local(polygon[i]), local(polygon[j])),
                );
                // Adjacent sides may continue straight, but must not backtrack.
                let k = (j + 1) % polygon.len();
                if on_segment(polygon[k], polygon[i], polygon[j], epsilon)
                    || on_segment(polygon[i], polygon[j], polygon[k], epsilon)
                {
                    return Err(invalid());
                }
            }
            let area = area + correction;
            if area.abs() <= epsilon * polygon.len() as Real {
                return Err(invalid());
            }
            if area > 0.0 && outer.replace(loop_index).is_some() {
                return Err(invalid());
            }
        }
        let outer = outer.ok_or_else(invalid)?;
        // Nonadjacent sides, including sides in different loops, must not touch.
        // Sweep in X to avoid testing pairs with disjoint X intervals.
        let mut sides = ranges
            .iter()
            .enumerate()
            .flat_map(|(loop_id, range)| {
                range.clone().map(move |start| {
                    let end = if start + 1 == range.end {
                        range.start
                    } else {
                        start + 1
                    };
                    (loop_id, start, end)
                })
            })
            .collect::<Vec<_>>();
        sides.sort_by(|a, b| {
            points[a.1][0]
                .min(points[a.2][0])
                .total_cmp(&points[b.1][0].min(points[b.2][0]))
        });
        for (index, &(a_loop, a, b)) in sides.iter().enumerate() {
            let max_x = points[a][0].max(points[b][0]) + epsilon;
            for &(b_loop, c, d) in &sides[index + 1..] {
                if points[c][0].min(points[d][0]) > max_x {
                    break;
                }
                if a_loop == b_loop && (b == c || d == a) {
                    continue;
                }
                if sides_touch(points[a], points[b], points[c], points[d], epsilon) {
                    return Err(invalid());
                }
            }
        }
        for (i, range) in ranges.iter().enumerate() {
            if i == outer {
                continue;
            }
            if !inside_polygon(points[range.start], &points[ranges[outer].clone()], epsilon) {
                return Err(invalid());
            }
            for (j, other) in ranges.iter().enumerate() {
                if j != i
                    && j != outer
                    && inside_polygon(points[range.start], &points[other.clone()], epsilon)
                {
                    return Err(invalid());
                }
            }
        }
        let mut loops = boundaries
            .into_iter()
            .enumerate()
            .map(|(index, trims)| {
                BrepLoop::try_new(
                    if index == outer {
                        BrepLoopType::Outer
                    } else {
                        BrepLoopType::Inner
                    },
                    trims,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let outer = loops.remove(outer);
        loops.insert(0, outer);
        Self::try_new(surface, reversed, loops)
    }
}

fn sides_touch(a: [Real; 2], b: [Real; 2], c: [Real; 2], d: [Real; 2], epsilon: Real) -> bool {
    if (0..2).any(|axis| {
        a[axis].max(b[axis]) + epsilon < c[axis].min(d[axis])
            || c[axis].max(d[axis]) + epsilon < a[axis].min(b[axis])
    }) {
        return false;
    }
    let crosses = [
        line_distance(c, a, b),
        line_distance(d, a, b),
        line_distance(a, c, d),
        line_distance(b, c, d),
    ];
    on_segment(c, a, b, epsilon)
        || on_segment(d, a, b, epsilon)
        || on_segment(a, c, d, epsilon)
        || on_segment(b, c, d, epsilon)
        || ((crosses[0] > 0.0) != (crosses[1] > 0.0) && (crosses[2] > 0.0) != (crosses[3] > 0.0))
}

// Coordinates are globally normalized and zero-length sides were rejected.
// A cross product has area units: divide the side direction by its length so
// the comparison uses the same distance epsilon for short and long sides.
fn line_distance(point: [Real; 2], start: [Real; 2], end: [Real; 2]) -> Real {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    (dx / length).mul_add(point[1] - start[1], -(dy / length) * (point[0] - start[0]))
}

fn on_segment(point: [Real; 2], start: [Real; 2], end: [Real; 2], epsilon: Real) -> bool {
    (0..2).all(|axis| {
        point[axis] >= start[axis].min(end[axis]) - epsilon
            && point[axis] <= start[axis].max(end[axis]) + epsilon
    }) && line_distance(point, start, end).abs() <= epsilon
}

fn inside_polygon(point: [Real; 2], polygon: &[[Real; 2]], epsilon: Real) -> bool {
    let mut winding = 0_i64;
    for i in 0..polygon.len() {
        let start = polygon[i];
        let end = polygon[(i + 1) % polygon.len()];
        if on_segment(point, start, end, epsilon) {
            return true;
        }
        let distance = line_distance(point, start, end);
        if start[1] <= point[1] {
            if end[1] > point[1] && distance > epsilon {
                winding += 1;
            }
        } else if end[1] <= point[1] && distance < -epsilon {
            winding -= 1;
        }
    }
    winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary(points: &[[Real; 2]]) -> Vec<BrepTrim> {
        (0..points.len())
            .map(|i| {
                let j = (i + 1) % points.len();
                BrepTrim::try_new(
                    [i, j],
                    Some(i),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_new(points[i][0], points[i][1]).unwrap(),
                        Point2::try_new(points[j][0], points[j][1]).unwrap(),
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
        BrepFace::try_from_polygon_boundaries(surface, true, boundaries)
    }

    fn outer() -> Vec<BrepTrim> {
        boundary(&[[0., 0.], [10., 0.], [10., 10.], [0., 10.]])
    }

    #[test]
    fn orders_outer_without_modifying_source_trims() {
        let a = boundary(&[[1., 1.], [1., 2.], [2., 2.], [2., 1.]]);
        let b = boundary(&[[5., 5.], [5., 7.], [7., 7.], [7., 5.]]);
        for boundaries in [
            vec![outer(), a.clone(), b.clone()],
            vec![a.clone(), outer(), b.clone()],
            vec![a.clone(), b.clone(), outer()],
        ] {
            let result = face(boundaries).unwrap();
            assert!(result.is_reversed());
            assert_eq!(result.loops[0].trims, outer());
            assert_eq!(result.loops[1].trims, a);
            assert_eq!(result.loops[2].trims, b);
        }
    }

    #[test]
    fn rejects_invalid_regions() {
        let hole = boundary(&[[1., 1.], [1., 5.], [5., 5.], [5., 1.]]);
        for (name, boundaries) in [
            ("empty", vec![]),
            ("no outer", vec![hole.clone()]),
            ("two outer loops", vec![outer(), outer()]),
            (
                "outside",
                vec![
                    outer(),
                    boundary(&[[11., 1.], [11., 2.], [12., 2.], [12., 1.]]),
                ],
            ),
            (
                "crossing outer",
                vec![
                    outer(),
                    boundary(&[[9., 1.], [9., 2.], [11., 2.], [11., 1.]]),
                ],
            ),
            (
                "touching outer",
                vec![outer(), boundary(&[[0., 1.], [0., 2.], [2., 2.], [2., 1.]])],
            ),
            (
                "overlapping holes",
                vec![
                    outer(),
                    hole.clone(),
                    boundary(&[[4., 4.], [4., 6.], [6., 6.], [6., 4.]]),
                ],
            ),
            (
                "nested holes",
                vec![
                    outer(),
                    hole.clone(),
                    boundary(&[[2., 2.], [2., 3.], [3., 3.], [3., 2.]]),
                ],
            ),
            (
                "touching holes",
                vec![
                    outer(),
                    hole,
                    boundary(&[[5., 5.], [5., 6.], [6., 6.], [6., 5.]]),
                ],
            ),
            (
                "self crossing nonzero area",
                vec![boundary(&[[0., 0.], [8., 0.], [0., 6.], [6., 8.]])],
            ),
            (
                "backtrack",
                vec![boundary(&[
                    [0., 0.],
                    [8., 0.],
                    [4., 0.],
                    [8., 8.],
                    [0., 8.],
                ])],
            ),
        ] {
            assert!(face(boundaries).is_err(), "{name}");
        }
    }

    #[test]
    fn accepts_concave_boundary_and_collinear_subdivision() {
        let polygon = boundary(&[
            [0., 0.],
            [5., 0.],
            [10., 0.],
            [10., 10.],
            [6., 10.],
            [6., 4.],
            [4., 4.],
            [4., 10.],
            [0., 10.],
        ]);
        let hole = boundary(&[[1., 1.], [1., 2.], [2., 2.], [2., 1.]]);
        assert!(face(vec![polygon.clone(), hole]).is_ok());
        let notch = boundary(&[[4.5, 5.], [4.5, 6.], [5.5, 6.], [5.5, 5.]]);
        assert!(face(vec![polygon, notch]).is_err());
    }

    #[test]
    fn rejects_uv_and_topological_gaps() {
        let mut broken = outer();
        broken[0].vertices[1] = 99;
        assert!(face(vec![broken]).is_err());
        let mut broken = outer();
        broken[0].curve = NurbsCurve2::try_line(
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(9., 0.).unwrap(),
        )
        .unwrap();
        assert!(face(vec![broken]).is_err());
    }

    #[test]
    fn rejects_curved_multispan_and_pole_trims() {
        let points = [[0., 0.], [5., 1.], [10., 0.]].map(|p| Point2::try_new(p[0], p[1]).unwrap());
        for curve in [
            NurbsCurve2::try_new(2, points.to_vec(), vec![0., 0., 0., 1., 1., 1.]).unwrap(),
            NurbsCurve2::try_new(1, points.to_vec(), vec![0., 0., 0.5, 1., 1.]).unwrap(),
            NurbsCurve2::try_new_rational(
                1,
                vec![
                    WeightedPoint2::try_new(points[0], 1.).unwrap(),
                    WeightedPoint2::try_new(points[2], -1.).unwrap(),
                ],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ] {
            let mut broken = outer();
            broken[0].curve = curve;
            assert!(face(vec![broken]).is_err());
        }
    }

    #[test]
    fn polygon_validation_is_scale_and_translation_independent() {
        for scale in [1e-150, 1., 1e150] {
            for offset in [0., 1e6] {
                let points = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]]
                    .map(|p| p.map(|v| (v + offset) * scale));
                assert!(face(vec![boundary(&points)]).is_ok(), "{scale} {offset}");
            }
        }
    }

    #[test]
    fn rectangle_hole_grid_matches_independent_interval_checks() {
        let fixed = boundary(&[[3., 3.], [3., 5.], [5., 5.], [5., 3.]]);
        for x in -2..12 {
            for y in -2..12 {
                for size in [1, 2, 4] {
                    let inside = x > 0 && y > 0 && x + size < 10 && y + size < 10;
                    let disjoint = x + size < 3 || y + size < 3 || x > 5 || y > 5;
                    let moving = boundary(&[
                        [x as Real, y as Real],
                        [x as Real, (y + size) as Real],
                        [(x + size) as Real, (y + size) as Real],
                        [(x + size) as Real, y as Real],
                    ]);
                    for boundaries in [
                        vec![outer(), fixed.clone(), moving.clone()],
                        vec![moving, fixed.clone(), outer()],
                    ] {
                        assert_eq!(
                            face(boundaries).is_ok(),
                            inside && disjoint,
                            "x={x}, y={y}, size={size}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn small_holes_are_validated_at_their_own_area_scale() {
        for size in [1e-4, 1e-6, 1e-8, 1e-10] {
            let square = |x: Real, y: Real, width: Real| {
                boundary(&[
                    [x, y],
                    [x, y + width],
                    [x + width, y + width],
                    [x + width, y],
                ])
            };
            let hole = square(4., 4., size);
            for boundaries in [vec![outer(), hole.clone()], vec![hole.clone(), outer()]] {
                assert!(face(boundaries).is_ok(), "size {size}");
            }
            let nested = square(4. + size * 0.25, 4. + size * 0.25, size * 0.5);
            assert!(
                face(vec![outer(), hole.clone(), nested]).is_err(),
                "nested {size}"
            );
            let disjoint = square(4. + size * 2., 4., size);
            assert!(
                face(vec![outer(), hole, disjoint]).is_ok(),
                "disjoint {size}"
            );
        }
    }
}
