use super::*;

#[test]
fn positive_thin_facets_are_not_discarded_by_the_polygon_area_epsilon() {
    let p = [[1., 1.], [1.1, 1.1], [1.05, 1.05 + 2. * Real::EPSILON]];
    let epsilon = 64. * Real::EPSILON;
    let area = doubled_area(p, epsilon).unwrap();
    assert!(area > 0. && area < epsilon);
    assert_eq!(doubled_area([p[0], p[2], p[1]], epsilon).unwrap(), -area);
    assert_eq!(doubled_area([p[0], p[1], p[1]], epsilon).unwrap(), 0.);
}

fn loops() -> Vec<Point2> {
    [
        [0., 0.],
        [10., 0.],
        [10., 10.],
        [0., 10.],
        [2., 2.],
        [2., 6.],
        [6., 6.],
        [6., 2.],
        [4., 4.],
        [4., 8.],
        [8., 8.],
        [8., 4.],
    ]
    .into_iter()
    .map(|p| Point2::try_new(p[0], p[1]).unwrap())
    .collect()
}

#[test]
fn transverse_constraints_add_vertices_and_mesh_the_union_of_holes() {
    for (scale, offset) in [(1e-100, 0.), (1., 0.), (1., 1e6), (1e100, -1e101)] {
        let mut points = loops()
            .into_iter()
            .map(|p| Point2::try_new(p.x() * scale + offset, p.y() * scale + offset).unwrap())
            .collect::<Vec<_>>();
        let original = points.clone();
        let triangles = triangulate(&mut points, &[4, 4, 4], &[]).unwrap().unwrap();
        assert!(points.len() > original.len());
        assert_eq!(points[..original.len()], original);
        let mut area = 0.;
        for triangle in triangles {
            let p = triangle.map(|i| {
                let p = points[i as usize];
                [(p.x() - offset) / scale, (p.y() - offset) / scale]
            });
            area += polygon_cross(p[0], p[1], p[2]) / 2.;
            let center = [
                (p[0][0] + p[1][0] + p[2][0]) / 3.,
                (p[0][1] + p[1][1] + p[2][1]) / 3.,
            ];
            assert!(!(center[0] > 2. && center[0] < 6. && center[1] > 2. && center[1] < 6.));
            assert!(!(center[0] > 4. && center[0] < 8. && center[1] > 4. && center[1] < 8.));
        }
        assert!((area - 72.).abs() < 1e-10, "{scale}: {area}");
    }
}

#[test]
fn unsupported_crossings_and_malformed_ranges_leave_parameters_untouched() {
    let original = loops();
    for lengths in [&[usize::MAX, 4, 4][..], &[4, 4, 0][..], &[4, 4, 5][..]] {
        let mut p = original.clone();
        assert!(triangulate(&mut p, lengths, &[]).unwrap().is_none());
        assert_eq!(p, original);
    }
    let mut p = original;
    for point in &mut p[8..] {
        *point = Point2::try_new(point.x() + 5., point.y()).unwrap();
    }
    let original = p.clone();
    assert!(triangulate(&mut p, &[4, 4, 4], &[]).unwrap().is_none());
    assert_eq!(p, original);
}
