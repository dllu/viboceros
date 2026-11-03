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
