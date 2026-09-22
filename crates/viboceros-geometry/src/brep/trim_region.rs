//! Classify connected CDT regions, not nearly collinear boundary triangles.
use super::*;

pub(super) fn interior_faces(
    triangulation: &ConstrainedDelaunayTriangulation<TrimTriangulationVertex>,
    normalized: &[[Real; 2]],
    loops: &[std::ops::Range<usize>],
    epsilon: Real,
) -> Vec<bool> {
    let count = triangulation.num_all_faces();
    let mut neighbors = vec![[None; 3]; count];
    let mut samples = vec![(0., [0.; 2]); count];
    for face in triangulation.inner_faces() {
        let index = face.index();
        neighbors[index] = face
            .adjacent_edges()
            .map(|edge| (!edge.is_constraint_edge()).then(|| edge.rev().face().index()));
        let points = face.vertices().map(|vertex| {
            let p = vertex.position();
            [p.x, p.y]
        });
        samples[index] = (
            polygon_cross(points[0], points[1], points[2]),
            [
                (points[0][0] + points[1][0] + points[2][0]) / 3.,
                (points[0][1] + points[1][1] + points[2][1]) / 3.,
            ],
        );
    }
    let outer = triangulation.outer_face().index();
    let mut visited = vec![false; count];
    visited[outer] = true;
    let mut inside = vec![false; count];
    for seed in 0..count {
        if visited[seed] {
            continue;
        }
        visited[seed] = true;
        let mut component = vec![seed];
        let mut cursor = 0;
        let mut reaches_outer = false;
        let mut best = samples[seed];
        while cursor < component.len() {
            let index = component[cursor];
            cursor += 1;
            if samples[index].0 > best.0 {
                best = samples[index];
            }
            for next in neighbors[index].into_iter().flatten() {
                reaches_outer |= next == outer;
                if !visited[next] {
                    visited[next] = true;
                    component.push(next);
                }
            }
        }
        // Constraints split exterior and holes from material. Classify each
        // component from its largest triangle, whose centroid is away from
        // numerical slivers on a sampled straight trim. Zero-area regions are
        // still rejected by the caller's area and boundary-conformity audits.
        let material = !reaches_outer
            && best.0 > epsilon
            && point_in_trim_polygon(best.1, &normalized[loops[0].clone()], epsilon)
            && !loops[1..]
                .iter()
                .any(|range| point_in_trim_polygon(best.1, &normalized[range.clone()], epsilon));
        for index in component {
            inside[index] = material;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: Real, y: Real) -> Point2 {
        Point2::try_new(x, y).unwrap()
    }

    #[test]
    fn nearly_straight_boundary_does_not_turn_exterior_slivers_into_material() {
        // The middle sample is a roundoff-sized inward dent. A tolerant
        // per-triangle centroid test considers the exterior sliver on-edge.
        for dent in [1e-15, 1e-13, 1e-10] {
            let points = vec![p(0., 0.), p(1., dent), p(2., 0.), p(2., 2.), p(0., 2.)];
            let triangles = triangulate_trim_region(&points, &[points.len()])
                .unwrap()
                .unwrap();
            assert_eq!(triangles.len(), 3);
            let mut area = 0.;
            for triangle in triangles {
                assert_ne!(
                    triangle.into_iter().collect::<BTreeSet<_>>(),
                    BTreeSet::from([0, 1, 2])
                );
                let [a, b, c] = triangle.map(|i| {
                    let p = points[i as usize];
                    [p.x(), p.y()]
                });
                let twice_area = polygon_cross(a, b, c);
                assert!(twice_area > 0.);
                area += twice_area / 2.;
            }
            assert!((area - (4. - dent)).abs() < 1e-14);
        }
    }

    #[test]
    fn hole_boundary_slivers_stay_inside_the_hole() {
        let points = vec![
            p(-1., -1.),
            p(3., -1.),
            p(3., 3.),
            p(-1., 3.),
            p(0., 0.),
            p(0., 2.),
            p(2., 2.),
            p(2., 0.),
            p(1., 1e-15),
        ];
        let triangles = triangulate_trim_region(&points, &[4, 5]).unwrap().unwrap();
        let mut area = 0.;
        for triangle in triangles {
            let [a, b, c] = triangle.map(|i| {
                let p = points[i as usize];
                [p.x(), p.y()]
            });
            area += polygon_cross(a, b, c) / 2.;
            let center = [(a[0] + b[0] + c[0]) / 3., (a[1] + b[1] + c[1]) / 3.];
            assert!(!(center[0] > 0. && center[0] < 2. && center[1] > 1e-14 && center[1] < 2.));
        }
        assert!((area - 12.).abs() < 1e-13);
    }
}
