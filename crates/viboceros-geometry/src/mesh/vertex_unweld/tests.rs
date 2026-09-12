use super::*;
use crate::{MeshFace, Point3, Tolerance};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn change_count_matches_pairwise_sharing_for_all_four_face_assignments() {
    for code in 0..256 {
        let labels = std::array::from_fn::<_, 4, _>(|face| ((code >> (2 * face)) & 3) as u32);
        let shared =
            (0..4).any(|first| (first + 1..4).any(|second| labels[first] == labels[second]));
        for reversed in [false, true] {
            for flipped in [false, true] {
                // Five coincident raw vertices include an unused member and
                // every possible assignment of four faces to the other four.
                let mut vertices = vec![point(0.0, 0.0, 0.0); 5];
                let mut faces = Vec::new();
                for (face, &raw) in labels.iter().enumerate() {
                    let base = vertices.len() as u32;
                    vertices.extend([
                        point(face as f64 + 1.0, 1.0, 0.0),
                        point(face as f64 + 1.0, 2.0, 0.0),
                    ]);
                    faces.push(if flipped {
                        [raw, base + 1, base]
                    } else {
                        [raw, base, base + 1]
                    });
                }
                if reversed {
                    faces.reverse();
                }
                let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
                let (result, count) = mesh.unwelded_topology_vertices(&[0, 0]).unwrap();
                assert_eq!(count, usize::from(shared), "assignment={labels:?}");
                assert_eq!(result.vertices().len(), 12);
                assert_eq!(result.faces().len(), 4);
                assert_eq!(
                    result
                        .faces()
                        .iter()
                        .flat_map(|face| face.indices())
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len(),
                    12
                );
                for (before, after) in mesh.faces().iter().zip(result.faces()) {
                    for (&source, &target) in before.indices().iter().zip(after.indices()) {
                        assert_eq!(
                            mesh.vertices()[source as usize],
                            result.vertices()[target as usize]
                        );
                    }
                }
                assert_eq!(result.area(), mesh.area());
            }
        }
    }
}

#[test]
fn rejects_first_invalid_selection_in_caller_order() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (selection, vertex) in [
        (vec![usize::MAX, 3], usize::MAX),
        (vec![0, 0, 3, usize::MAX], 3),
        (vec![2, 1, usize::MAX, 3], usize::MAX),
    ] {
        assert_eq!(
            mesh.unwelded_topology_vertices(&selection),
            Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                vertex,
                vertex_count: 3
            })
        );
    }
}

#[test]
fn sparse_selections_leave_other_panels_geometry_and_sharing_unchanged() {
    for panels in [1, 3, 257, 1024] {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for panel in 0..panels {
            let x = panel as f64 * 3.0;
            let base = vertices.len() as u32;
            vertices.extend([
                point(x, 0.0, 0.0),
                point(x + 1.0, 0.0, 0.0),
                point(x, 1.0, 0.0),
                point(x, -1.0, 0.0),
            ]);
            faces.extend([[base, base + 1, base + 2], [base + 1, base, base + 3]]);
        }
        vertices.push(point(-99.0, -99.0, -99.0));
        let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
        // All subsets of the first, middle, and last panel; duplicates arise
        // naturally for a one-panel input and must not change counts/order.
        for mask in 0..8 {
            let selected = [0, panels / 2, panels - 1]
                .into_iter()
                .enumerate()
                .filter_map(|(bit, panel)| (mask & (1 << bit) != 0).then_some(panel))
                .collect::<BTreeSet<_>>();
            let mut selection = selected.iter().map(|panel| panel * 4).collect::<Vec<_>>();
            selection.extend(selection.clone());
            let (result, count) = mesh.unwelded_topology_vertices(&selection).unwrap();
            assert_eq!(count, selected.len());
            assert_eq!(
                result.vertices().len(),
                4 * panels + selected.len() + usize::from(selection.is_empty())
            );
            assert_eq!(result.area(), mesh.area());
            assert_eq!(result.faces().len(), mesh.faces().len());
            for (before, after) in mesh.faces().iter().zip(result.faces()) {
                let points = |mesh: &TriangleMesh, face: &MeshFace| {
                    face.indices()
                        .iter()
                        .map(|&raw| mesh.vertices()[raw as usize])
                        .collect::<Vec<_>>()
                };
                assert_eq!(points(&mesh, before), points(&result, after));
            }
            for panel in 0..panels {
                let first = result.faces()[2 * panel].indices();
                let second = result.faces()[2 * panel + 1].indices();
                assert_eq!(first[0] != second[1], selected.contains(&panel));
                assert_eq!(first[1], second[0]);
            }
            let mut reversed = selection;
            reversed.reverse();
            assert_eq!(
                mesh.unwelded_topology_vertices(&reversed).unwrap(),
                (result, count)
            );
        }
    }
}

#[test]
fn unwelds_selected_topology_vertices_in_rhino_order_and_compacts() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(mesh.topology_vertex_points(), mesh.vertices());

    let (unwelded, vertex_count) = mesh.unwelded_topology_vertices(&[0]).unwrap();
    assert_eq!(vertex_count, 1);
    assert_eq!(
        unwelded.vertices(),
        &[
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]
    );
    assert_eq!(unwelded.triangles(), &[[4, 0, 1], [0, 3, 2]]);
    assert_eq!(
        mesh.unwelded_topology_vertices(&[0, 0]).unwrap().0,
        unwelded
    );

    let (empty, vertex_count) = mesh.unwelded_topology_vertices(&[]).unwrap();
    assert_eq!((empty, vertex_count), (mesh.clone(), 0));
    let (naked, vertex_count) = mesh.unwelded_topology_vertices(&[2]).unwrap();
    assert_eq!(vertex_count, 0);
    assert_eq!(naked.vertices(), &mesh.vertices()[..4]);
    assert_eq!(naked.triangles(), mesh.triangles());
    assert_eq!(
        mesh.unwelded_topology_vertices(&[5]),
        Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
            vertex: 5,
            vertex_count: 5,
        })
    );

    let already_unwelded = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [1, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        already_unwelded.unwelded_topology_vertices(&[0]).unwrap(),
        (unwelded, 0)
    );
}

#[test]
fn selected_topology_vertex_separates_closed_and_non_manifold_fans() {
    let fan = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(-1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
        ],
        vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (unwelded, vertex_count) = fan.unwelded_topology_vertices(&[0]).unwrap();
    assert_eq!(vertex_count, 1);
    assert_eq!(unwelded.vertices().len(), 8);
    assert_eq!(
        unwelded.triangles(),
        &[[6, 1, 0], [5, 2, 1], [4, 3, 2], [7, 0, 3]]
    );

    let non_manifold = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ],
        vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (unwelded, vertex_count) = non_manifold.unwelded_topology_vertices(&[0]).unwrap();
    assert_eq!(vertex_count, 1);
    assert_eq!(
        unwelded.vertices(),
        &[
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]
    );
    assert_eq!(unwelded.triangles(), &[[5, 0, 1], [0, 4, 2], [6, 0, 3]]);
}

#[test]
fn selected_topology_vertex_handles_triangle_and_quad_cube_corners() {
    let vertices = vec![
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(1.0, 1.0, 0.0),
        point(0.0, 1.0, 0.0),
        point(0.0, 0.0, 1.0),
        point(1.0, 0.0, 1.0),
        point(1.0, 1.0, 1.0),
        point(0.0, 1.0, 1.0),
    ];
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let mesh = TriangleMesh::try_new(vertices.clone(), triangles, Tolerance::DEFAULT).unwrap();
    let (corner, vertex_count) = mesh.unwelded_topology_vertices(&[0]).unwrap();
    assert_eq!(vertex_count, 1);
    assert_eq!(corner.vertices().len(), 12);
    assert_eq!(
        corner.triangles(),
        &[
            [10, 1, 0],
            [9, 2, 1],
            [3, 4, 5],
            [3, 5, 6],
            [11, 0, 4],
            [7, 4, 3],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 8, 3],
            [2, 3, 6],
        ]
    );

    let all_vertices = (0..mesh.topology().topological_vertex_count()).collect::<Vec<_>>();
    let (all, vertex_count) = mesh.unwelded_topology_vertices(&all_vertices).unwrap();
    assert_eq!(vertex_count, 8);
    assert_eq!(all.vertices().len(), 36);
    assert_eq!(
        all.faces()
            .iter()
            .flat_map(|face| face.indices())
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        36
    );

    let quad_mesh = TriangleMesh::try_new_faces(
        vertices,
        vec![
            MeshFace::Quad([0, 3, 2, 1]),
            MeshFace::Quad([4, 5, 6, 7]),
            MeshFace::Quad([0, 1, 5, 4]),
            MeshFace::Quad([1, 2, 6, 5]),
            MeshFace::Quad([2, 3, 7, 6]),
            MeshFace::Quad([3, 0, 4, 7]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (quad_corner, vertex_count) = quad_mesh.unwelded_topology_vertices(&[0]).unwrap();
    assert_eq!(vertex_count, 1);
    assert_eq!(quad_corner.vertices().len(), 10);
    assert!(quad_corner.faces().iter().all(|face| face.is_quad()));
}
