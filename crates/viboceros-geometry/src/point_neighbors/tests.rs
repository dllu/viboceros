use super::*;
#[test]
fn point_queries_preserve_neighbours_at_binary64_boundaries() {
    for scale in [f64::from_bits(1), 1e-100, 1., 1e100, 1e300] {
        let mut points = (-12..=12)
            .flat_map(|i| {
                let x = i as f64 * scale;
                [x.next_down(), x, x.next_up()]
            })
            .filter(|x| x.is_finite())
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .collect::<Vec<_>>();
        points.push(Point3::try_new(-f64::MAX, 0., 0.).unwrap());
        points.push(Point3::try_new(f64::MAX, 0., 0.).unwrap());
        let tree = PointTree::new(points.clone());
        for &point in &points {
            for radius in [0., scale.next_down(), scale, scale.next_up(), f64::MAX] {
                let mut found = Vec::new();
                let mut checks = 0;
                tree.visit_near(
                    point,
                    radius,
                    &mut || {
                        checks += 1;
                        Ok::<_, ()>(())
                    },
                    &mut |i| {
                        if (points[i].x() - point.x()).abs() <= radius {
                            found.push(i)
                        }
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
                found.sort_unstable();
                assert_eq!(
                    found,
                    points
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| ((p.x() - point.x()).abs() <= radius).then_some(i))
                        .collect::<Vec<_>>()
                );
                assert!(checks <= tree.nodes.len());
            }
        }
    }
}
