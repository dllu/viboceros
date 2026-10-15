use super::*;
fn data(vertices: Vec<[f64; 3]>, faces: Vec<[u32; 4]>) -> MeshData {
    MeshData {
        vertices,
        faces,
        ..Default::default()
    }
}
#[test]
fn cleanup_repairs_quads_culls_lines_and_orphans_without_welding_colors() {
    let source = data(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 0.],
            [2., 0., 0.],
            [3., 0., 0.],
            [4., 0., 0.],
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [99., 99., 99.],
        ],
        vec![[0, 1, 2, 3], [4, 5, 6, 6], [7, 8, 9, 9]],
    );
    let source = MeshData {
        colors: Some(
            (0..source.vertices.len())
                .map(|i| [i as u8, 2, 3, 4])
                .collect(),
        ),
        ..source
    };
    let before = source.clone();
    let result = cull_degenerate_mesh_faces(&source).unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (1, 1, 5)
    );
    assert_eq!(result.mesh.faces.len(), 2);
    assert_eq!(result.mesh.vertices.len(), 6);
    assert!(result.mesh.faces.iter().all(|f| f[2] == f[3]));
    assert_eq!(
        result
            .mesh
            .colors
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c[0])
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 7, 8, 9]
    );
    assert_eq!(source, before);
    let second = cull_degenerate_mesh_faces(&result.mesh).unwrap();
    assert_eq!(
        (
            second.removed_faces,
            second.repaired_faces,
            second.removed_vertices
        ),
        (0, 0, 0)
    );
    assert_eq!(second.mesh, result.mesh);
}
#[test]
fn all_degenerate_and_invalid_raw_indices_become_empty_without_out_of_bounds_reads() {
    for face in [
        [0, 1, 2, 2],
        [0, 0, 1, 1],
        [0, 1, 99, 99],
        [0, 1, u32::MAX, u32::MAX],
    ] {
        let source = data(vec![[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]], vec![face]);
        let result = cull_degenerate_mesh_faces(&source).unwrap();
        assert_eq!(
            (
                result.removed_faces,
                result.repaired_faces,
                result.removed_vertices
            ),
            (1, 0, 3)
        );
        assert!(result.mesh.faces.is_empty());
        assert!(result.mesh.vertices.is_empty());
    }
}
#[test]
fn tiny_remote_and_unreferenced_vertex_cases_follow_native_double_precision_policy() {
    let source = data(
        vec![
            [1e6, 0., 0.],
            [1e6 + 1e-4, 0., 0.],
            [1e6, 1e-4, 0.],
            [9., 9., 9.],
        ],
        vec![[0, 1, 2, 2]],
    );
    let result = cull_degenerate_mesh_faces(&source).unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (0, 0, 1)
    );
    assert_eq!(result.mesh.vertices, &source.vertices[..3]);
    for (scale, removed) in [(1e-17, 0), (1e-19, 1)] {
        let result = cull_degenerate_mesh_faces(&data(
            vec![[0., 0., 0.], [scale, 0., 0.], [0., scale, 0.]],
            vec![[0, 1, 2, 2]],
        ))
        .unwrap();
        assert_eq!(result.removed_faces, removed);
    }
}
#[test]
fn repaired_ngon_corner_used_elsewhere_does_not_leave_invalid_overlay() {
    let mut source = data(
        vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 0., 0.],
            [-1., 0., 0.],
            [-1., 1., 0.],
        ],
        vec![[0, 1, 2, 3], [3, 4, 5, 5]],
    );
    source.ngons = vec![Ngon {
        vertices: vec![0, 1, 2, 3],
        faces: vec![0],
    }];
    let result = cull_degenerate_mesh_faces(&source).unwrap();
    assert_eq!((result.removed_faces, result.repaired_faces), (0, 1));
    assert_eq!(result.mesh.ngons.len(), 1);
    assert_eq!(result.mesh.ngons[0].vertices, vec![0, 1, 2]);
}

#[test]
fn repaired_ngon_that_loses_edge_connectivity_can_dissolve_without_welding() {
    let mut source = data(
        (0..2)
            .flat_map(|y| (0..4).map(move |x| [x as f64, y as f64, 0.]))
            .collect(),
        vec![[0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6]],
    );
    source.ngons = vec![Ngon {
        vertices: vec![0, 1, 2, 3, 7, 6, 5, 4],
        faces: vec![0, 1, 2],
    }];
    source.vertices[1] = source.vertices[0];
    let result = cull_degenerate_mesh_faces(&source).unwrap();
    assert_eq!(
        (
            result.removed_faces,
            result.repaired_faces,
            result.removed_vertices
        ),
        (0, 1, 0)
    );
    assert_eq!(result.mesh.vertices.len(), 8);
    // The remaining left and middle facets have coincident unwelded edges.
    assert_eq!(result.mesh.vertices[0], result.mesh.vertices[1]);
}
