use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn component_order_matches_graph_traversal_for_every_graph_up_to_six_faces() {
    for count in 0..=6 {
        let edges = (0..count)
            .flat_map(|a| (a + 1..count).map(move |b| (a, b)))
            .collect::<Vec<_>>();
        for mask in 0..1usize << edges.len() {
            let mut adjacent = vec![Vec::new(); count];
            for (bit, &(a, b)) in edges.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    adjacent[a].push(b);
                    adjacent[b].push(a);
                }
            }
            // Independent traversal, with no union-find or root lookup.
            let mut visited = vec![false; count];
            let mut expected = Vec::new();
            for start in 0..count {
                if visited[start] {
                    continue;
                }
                let mut stack = vec![start];
                let mut component = Vec::new();
                visited[start] = true;
                while let Some(face) = stack.pop() {
                    component.push(face);
                    for &neighbor in &adjacent[face] {
                        if !visited[neighbor] {
                            visited[neighbor] = true;
                            stack.push(neighbor);
                        }
                    }
                }
                component.sort_unstable();
                expected.push(component);
            }
            for reverse in [false, true] {
                let mut parents = (0..count).collect::<Vec<_>>();
                let mut ranks = vec![0; count];
                for step in 0..edges.len() {
                    let bit = if reverse {
                        edges.len() - 1 - step
                    } else {
                        step
                    };
                    if mask & (1 << bit) != 0 {
                        let (a, b) = edges[bit];
                        let (a, b) = if reverse { (b, a) } else { (a, b) };
                        union_faces(&mut parents, &mut ranks, a, b);
                    }
                }
                assert_eq!(component_faces(&mut parents), expected);
            }
        }
    }
}

#[test]
fn many_components_with_a_shared_vertex_reset_only_their_own_remap_entries() {
    let mut vertices = vec![p(0., 0., 0.), p(999., 999., 999.)];
    let mut faces = Vec::new();
    // The common vertex appears at a different local index in alternating
    // pieces, exposing stale scratch entries across disconnected components.
    for i in 0..2048u32 {
        vertices.extend([p(i as f64 + 1., 1., 0.), p(i as f64 + 1., 0., 1.)]);
        faces.push(if i % 2 == 0 {
            [0, 2 * i + 2, 2 * i + 3]
        } else {
            [2 * i + 2, 0, 2 * i + 3]
        });
    }
    let mesh = TriangleMesh::try_new(vertices, faces.clone(), Tolerance::DEFAULT).unwrap();
    for pieces in [mesh.disjoint_pieces(), mesh.explode_pieces()] {
        assert_eq!(pieces.len(), faces.len());
        for (piece, face) in pieces.iter().zip(&faces) {
            assert_eq!(piece.vertices(), &face.map(|i| mesh.vertices()[i as usize]));
            assert_eq!(piece.faces(), &[MeshFace::Triangle([0, 1, 2])]);
        }
    }
}

#[test]
fn component_remapping_preserves_quads_duplicate_raw_vertices_and_face_order() {
    let mesh = TriangleMesh::try_new_faces(
        vec![
            p(50., 50., 50.),
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(2., 2., 1.),
            p(0., 2., 0.),
            p(10., 0., 0.),
            p(12., 0., 0.),
            p(10., 2., 0.),
            p(2., 0., 0.),
            p(0., 2., 0.),
            p(2., -2., 0.),
        ],
        vec![
            MeshFace::Quad([1, 2, 3, 4]),
            MeshFace::Triangle([5, 6, 7]),
            MeshFace::Triangle([8, 1, 10]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let pieces = mesh.disjoint_pieces();
    assert_eq!(pieces.len(), 2);
    assert_eq!(
        pieces[0].vertices(),
        &[1, 2, 3, 4, 8, 10].map(|i| mesh.vertices()[i])
    );
    assert_eq!(
        pieces[0].faces(),
        &[MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 0, 5])]
    );
    assert_eq!(pieces[1].vertices(), &[5, 6, 7].map(|i| mesh.vertices()[i]));
    assert_eq!(pieces[1].faces(), &[MeshFace::Triangle([0, 1, 2])]);
    // Sharing one raw endpoint leaves this seam welded for Explode too.
    assert_eq!(mesh.explode_pieces(), pieces);
    for maximum in [0, 1] {
        assert_eq!(
            mesh.try_explode_pieces(maximum),
            Err(GeometryError::MeshComponentLimit { maximum })
        );
        assert_eq!(
            mesh.try_disjoint_pieces(maximum),
            Err(GeometryError::MeshComponentLimit { maximum })
        );
    }
    for maximum in [2, 3, usize::MAX] {
        assert_eq!(mesh.try_explode_pieces(maximum).unwrap(), pieces);
        assert_eq!(mesh.try_disjoint_pieces(maximum).unwrap(), pieces);
    }
}
