use super::*;

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
