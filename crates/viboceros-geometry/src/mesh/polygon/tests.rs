use super::*;

#[test]
fn nearby_holes_retain_the_strip_between_their_boundaries() {
    let gap = 2.0_f64.powi(-43);
    let points = [
        [0., 0.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [0.1, 0.2],
        [0.5, 0.2],
        [0.5, 0.8],
        [0.1, 0.8],
        [0.5 + gap, 0.2],
        [0.9, 0.2],
        [0.9, 0.8],
        [0.5 + gap, 0.8],
    ];
    for winding in 0..8 {
        for origin in 0..4 {
            let mut points = points;
            for (index, boundary) in points.chunks_exact_mut(4).enumerate() {
                if winding & (1 << index) != 0 {
                    boundary.reverse();
                }
                boundary.rotate_left(origin);
            }
            let triangles = triangulate_projected_mesh_region(&points, &[0..4, 4..8, 8..12])
                .unwrap()
                .expect("two distinct holes leave a valid narrow strip");
            assert_eq!(triangles.len(), 14);
            audit_region(&points, &[0..4, 4..8, 8..12], &triangles, 0.52 + 0.6 * gap);
            let strip = triangles
                .iter()
                .filter(|triangle| {
                    triangle.iter().all(|&i| {
                        points[i as usize][0] >= 0.5 && points[i as usize][0] <= 0.5 + gap
                    })
                })
                .count();
            assert_eq!(strip, 2);
        }
    }
}

fn audit_region(
    points: &[[Real; 2]],
    loops: &[Range<usize>],
    triangles: &[[u32; 3]],
    expected_area: Real,
) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut edges = BTreeMap::<[u32; 2], (usize, i32)>::new();
    let mut area = 0.;
    for &[a, b, c] in triangles {
        let [p, q, r] = [a, b, c].map(|i| points[i as usize]);
        let doubled = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        assert!(doubled > 0.);
        area += 0.5 * doubled;
        for [u, v] in [[a, b], [b, c], [c, a]] {
            let entry = edges.entry([u.min(v), u.max(v)]).or_default();
            entry.0 += 1;
            entry.1 += if u < v { 1 } else { -1 };
        }
    }
    assert!((area - expected_area).abs() < 1e-11);
    let expected = loops
        .iter()
        .flat_map(|range| {
            range.clone().map(|i| {
                let j = if i + 1 == range.end {
                    range.start
                } else {
                    i + 1
                };
                [i.min(j) as u32, i.max(j) as u32]
            })
        })
        .collect::<BTreeSet<_>>();
    let mut boundary = BTreeSet::new();
    for (edge, (uses, orientation)) in edges {
        if uses == 1 {
            boundary.insert(edge);
        } else {
            assert_eq!((uses, orientation), (2, 0));
        }
    }
    assert_eq!(boundary, expected);
}

#[test]
fn many_holes_preserve_every_boundary_and_oriented_interior_edge() {
    for size in [8, 16, 32] {
        let mut points = vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        let mut loops = Vec::with_capacity(size * size + 1);
        loops.push(0..4);
        for y in 0..size {
            for x in 0..size {
                let [x0, x1] = [0.3, 0.7].map(|offset| (x as Real + offset) / size as Real);
                let [y0, y1] = [0.3, 0.7].map(|offset| (y as Real + offset) / size as Real);
                let start = points.len();
                points.extend([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
                loops.push(start..points.len());
            }
        }
        let start = std::time::Instant::now();
        let triangles = triangulate_projected_mesh_region(&points, &loops)
            .unwrap()
            .unwrap();
        println!(
            "POLYGON_HOLES size={size} holes={} vertices={} triangles={} microseconds={}",
            size * size,
            points.len(),
            triangles.len(),
            start.elapsed().as_micros()
        );
        assert_eq!(triangles.len(), 6 * size * size + 2);
        audit_region(&points, &loops, &triangles, 0.84);
    }
}

#[test]
fn an_inner_boundary_outside_the_outer_loop_is_rejected() {
    let points = [
        [0., 0.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [2., 0.],
        [2.1, 0.],
        [2.1, 0.1],
        [2., 0.1],
    ];
    assert!(
        triangulate_projected_mesh_region(&points, &[0..4, 4..8])
            .unwrap()
            .is_none()
    );
}

#[test]
fn constrained_mesh_region_triangulates_an_annulus_without_covering_its_hole() {
    let points = [
        [0., 0.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [0.25, 0.25],
        [0.75, 0.25],
        [0.75, 0.75],
        [0.25, 0.75],
    ];
    let triangles = triangulate_projected_mesh_region(&points, &[0..4, 4..8])
        .unwrap()
        .unwrap();
    assert_eq!(triangles.len(), 8);
    let area = triangles
        .iter()
        .map(|triangle| {
            mesh_hole_cross(
                points[triangle[0] as usize],
                points[triangle[1] as usize],
                points[triangle[2] as usize],
            ) * 0.5
        })
        .sum::<Real>();
    assert!((area - 0.75).abs() < 1e-14);
}

#[test]
fn constrained_mesh_region_handles_disjoint_holes_with_overlapping_bounds() {
    let points = [
        [0., 0.],
        [6., 0.],
        [6., 6.],
        [0., 6.],
        [1., 1.],
        [5., 1.],
        [1., 5.],
        [5., 5.],
        [5., 2.],
        [2., 5.],
    ];
    let triangles = triangulate_projected_mesh_region(&points, &[0..4, 4..7, 7..10])
        .unwrap()
        .unwrap();
    assert_eq!(triangles.len(), 12);
    let area = triangles
        .iter()
        .map(|triangle| {
            mesh_hole_cross(
                points[triangle[0] as usize],
                points[triangle[1] as usize],
                points[triangle[2] as usize],
            ) * 0.5
        })
        .sum::<Real>();
    assert!((area - 23.5).abs() < 1e-12);
}

#[test]
fn constrained_mesh_region_rejects_nested_inner_holes() {
    let points = [
        [0., 0.],
        [10., 0.],
        [10., 10.],
        [0., 10.],
        [2., 2.],
        [8., 2.],
        [8., 8.],
        [2., 8.],
        [4., 4.],
        [6., 4.],
        [6., 6.],
        [4., 6.],
    ];
    assert!(
        triangulate_projected_mesh_region(&points, &[0..4, 4..8, 8..12])
            .unwrap()
            .is_none()
    );
}

#[test]
fn constrained_mesh_region_rejects_crossing_inner_holes() {
    let points = [
        [0., 0.],
        [10., 0.],
        [10., 10.],
        [0., 10.],
        [2., 2.],
        [6., 2.],
        [6., 6.],
        [2., 6.],
        [4., 4.],
        [8., 4.],
        [8., 8.],
        [4., 8.],
    ];
    assert!(
        triangulate_projected_mesh_region(&points, &[0..4, 4..8, 8..12])
            .unwrap()
            .is_none()
    );
}

#[test]
fn constrained_mesh_region_rejects_a_hole_touching_the_outer_boundary() {
    let points = [
        [0., 0.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [0., 0.],
        [0.4, 0.2],
        [0.2, 0.4],
    ];
    assert!(
        triangulate_projected_mesh_region(&points, &[0..4, 4..7])
            .unwrap()
            .is_none()
    );
}
