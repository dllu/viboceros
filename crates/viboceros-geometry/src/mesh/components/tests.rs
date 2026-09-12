use super::*;

#[test]
fn ordered_grouping_matches_label_partitions_and_all_root_orders() {
    let faces = [91, 5, 72, 18];
    let permutations = (0..256)
        .map(|code| std::array::from_fn::<_, 4, _>(|index| (code >> (2 * index)) & 3))
        .filter(|order| order.iter().copied().collect::<BTreeSet<_>>().len() == 4)
        .collect::<Vec<_>>();
    assert_eq!(permutations.len(), 24);
    for code in 0..256 {
        let labels = std::array::from_fn::<_, 4, _>(|index| (code >> (2 * index)) & 3);
        for reverse in [false, true] {
            let mut parents = (0..4).collect::<Vec<_>>();
            for label in 0..4 {
                let mut members = (0..4)
                    .filter(|&index| labels[index] == label)
                    .collect::<Vec<_>>();
                if reverse {
                    members.reverse();
                }
                for pair in members.windows(2) {
                    parents[pair[0]] = pair[1];
                }
            }
            for permutation in &permutations {
                let order = permutation
                    .iter()
                    .copied()
                    .filter(|&index| parents[index] == index)
                    .collect::<Vec<_>>();
                let expected = order
                    .iter()
                    .map(|&root| {
                        faces
                            .iter()
                            .enumerate()
                            .filter_map(|(local, &face)| {
                                (labels[local] == labels[root]).then_some(face)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    faces_in_component_order(&faces, &mut parents.clone(), &order),
                    expected
                );
            }
        }
    }
    assert_eq!(
        faces_in_component_order(&[], &mut [], &[]),
        Vec::<Vec<usize>>::new()
    );
}

#[test]
fn streamed_use_pairs_keep_order_and_all_metadata_even_with_repeated_faces() {
    for count in 0..=6 {
        let uses = (0..count)
            .map(|index| EdgeUse {
                face: index % 2,
                side: index % 4,
                forward: index % 2 == 0,
                raw_vertices: [index as u32 * 2, index as u32 * 2 + 1],
            })
            .collect::<Vec<_>>();
        let mut incidence = EdgeIncidence::default();
        for &edge_use in &uses {
            incidence.add_use(edge_use);
        }
        let mut expected = Vec::new();
        for first in 0..count {
            for second in first + 1..count {
                expected.push((uses[first], uses[second]));
            }
        }
        assert_eq!(incidence.use_pairs().collect::<Vec<_>>(), expected);
    }
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn streamed_face_angles_match_indexed_pairs_and_strict_filter_boundaries() {
    let coordinates: [[f64; 3]; 5] = [
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [-1., 0., 0.],
        [0., -1., 0.],
    ];
    let normals =
        coordinates.map(|[x, y, z]| UnitVector3::try_new(x, y, z, Tolerance::DEFAULT).unwrap());
    let thresholds = [
        0.,
        std::f64::consts::FRAC_PI_4,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    ];
    for count in 0..=5 {
        for offset in 0..5 {
            let faces = (0..count)
                .map(|index| (index * 2 + offset) % 5)
                .collect::<Vec<_>>();
            let mut expected_pairs = Vec::new();
            let mut maximum: f64 = 0.;
            for left in 0..count {
                for right in left + 1..count {
                    let a = coordinates[faces[left]];
                    let b = coordinates[faces[right]];
                    let angle = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).acos();
                    maximum = maximum.max(angle);
                    expected_pairs.push((faces[left], faces[right]));
                }
            }
            for unwelded in [false, true] {
                let mut incidence = EdgeIncidence::default();
                for (index, &face) in faces.iter().enumerate() {
                    let vertex = if unwelded { index as u32 * 2 } else { 0 };
                    incidence.add_use(EdgeUse {
                        face,
                        side: 0,
                        forward: index % 2 == 0,
                        raw_vertices: [vertex, vertex + 1],
                    });
                }
                assert_eq!(incidence.face_pairs().collect::<Vec<_>>(), expected_pairs);
                for lower in thresholds {
                    assert_eq!(
                        mesh_edge_is_logical_boundary(&incidence, &normals, lower),
                        count <= 1 || unwelded || maximum >= lower
                    );
                    for upper in thresholds {
                        assert_eq!(
                            mesh_edge_matches_filter(
                                &incidence,
                                MeshEdgeFilter::FaceAngle {
                                    greater_than_radians: lower,
                                    less_than_radians: upper,
                                },
                                Some(&normals)
                            ),
                            count >= 2 && maximum > lower && maximum < upper
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unwelded_predicate_matches_independent_pairwise_endpoint_distinctness() {
    for count in 0..=4u32 {
        for mut code in 0..16usize.pow(count) {
            let uses = (0..count as usize)
                .map(|face| {
                    let raw_vertices = [(code % 4) as u32, ((code / 4) % 4) as u32];
                    code /= 16;
                    EdgeUse {
                        face,
                        side: face % 3,
                        forward: face % 2 == 0,
                        raw_vertices,
                    }
                })
                .collect::<Vec<_>>();
            let expected = uses.iter().enumerate().all(|(index, first)| {
                uses[index + 1..].iter().all(|second| {
                    first.raw_vertices[0] != second.raw_vertices[0]
                        && first.raw_vertices[1] != second.raw_vertices[1]
                })
            });
            assert_eq!(edge_uses_are_unwelded(uses.iter().copied()), expected);
        }
    }
}

#[test]
fn component_order_and_limits_match_graph_traversal_for_every_graph_up_to_six_faces() {
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
                for maximum in (0..=count).chain([usize::MAX]) {
                    let result = component_faces(&mut parents, maximum);
                    if expected.len() <= maximum {
                        assert_eq!(result.unwrap(), expected);
                    } else {
                        assert_eq!(result, Err(GeometryError::MeshComponentLimit { maximum }));
                    }
                }
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
