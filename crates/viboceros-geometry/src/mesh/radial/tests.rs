use super::super::{EdgeUse, Point3, Tolerance, TriangleMesh, topology_face_edge_indices};
use super::*;
use std::collections::BTreeSet;

#[test]
fn face_walk_seen_flags_use_local_indices_for_sparse_and_fallback_faces() {
    let incidences = [
        incidence_with_faces([42, 900]),
        incidence_with_faces([42]),
        incidence_with_faces([900]),
    ];
    let edges = incidences
        .iter()
        .enumerate()
        .map(|(index, incidence)| ([0, index + 1], incidence))
        .collect::<Vec<_>>();
    let groups = vec![vec![0, 1], vec![2], vec![2]];
    assert_eq!(
        radial_vertex_face_walk(&groups, &edges, &[42, 900, usize::MAX]),
        vec![(42, Some(0)), (900, Some(2)), (usize::MAX, None)]
    );
    assert_eq!(radial_vertex_face_walk(&[], &[], &[]), Vec::new());
}

#[test]
fn cyclic_component_seen_flags_match_label_order_for_all_four_face_partitions() {
    let incidences = [[0, 3], [0, 1], [1, 2], [2, 3]].map(incidence_with_faces);
    let edges = incidences
        .iter()
        .enumerate()
        .map(|(index, incidence)| ([0, index + 1], incidence))
        .collect::<Vec<_>>();
    let faces = [0, 1, 2, 3];
    let locals = BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 3)]);
    for code in 0..256 {
        let labels = std::array::from_fn::<_, 4, _>(|index| (code >> (2 * index)) & 3);
        for latest in [false, true] {
            let representatives = std::array::from_fn::<_, 4, _>(|label| {
                let mut members = (0..4).filter(|&index| labels[index] == label);
                if latest {
                    members.next_back()
                } else {
                    members.next()
                }
            });
            for rotation in 0..4 {
                let group = (0..4)
                    .map(|index| (index + rotation) % 4)
                    .collect::<Vec<_>>();
                let mut runs = Vec::new();
                for &face in &group {
                    if runs.last().copied() != Some(labels[face]) {
                        runs.push(labels[face]);
                    }
                }
                if runs.len() > 1 && runs.first() == runs.last() {
                    runs.remove(0);
                }
                let mut expected = Vec::new();
                for label in runs {
                    let root = representatives[label].unwrap();
                    if !expected.contains(&root) {
                        expected.push(root);
                    }
                }
                let mut parents = labels.map(|label| representatives[label].unwrap());
                assert_eq!(
                    ordered_vertex_face_components(&[group], &edges, &locals, &faces, &mut parents),
                    expected
                );
            }
        }
    }
}

fn incidence_with_faces(faces: impl IntoIterator<Item = usize>) -> EdgeIncidence {
    let mut incidence = EdgeIncidence::default();
    for face in faces {
        incidence.add_use(EdgeUse {
            face,
            side: 0,
            forward: true,
            raw_vertices: [0, 1],
        });
    }
    incidence
}

#[test]
fn shared_face_merge_matches_all_eight_face_subset_pairs() {
    let incidences = (0..256)
        .map(|mask| incidence_with_faces((0..8).filter(|&face| mask & (1 << face) != 0)))
        .collect::<Vec<_>>();
    for (left_mask, left) in incidences.iter().enumerate() {
        for (right_mask, right) in incidences.iter().enumerate() {
            let expected = (0..8).find(|&face| left_mask & right_mask & (1 << face) != 0);
            assert_eq!(shared_edge_face(left, right), expected);
        }
    }
}

#[test]
fn shared_face_merge_handles_long_disjoint_streams_duplicates_and_extreme_indices() {
    let even = incidence_with_faces((0..8192).step_by(2));
    let odd = incidence_with_faces((1..8192).step_by(2));
    assert_eq!(shared_edge_face(&even, &odd), None);
    let even_with_tail = incidence_with_faces((0..8192).step_by(2).chain([usize::MAX]));
    let odd_with_tail = incidence_with_faces((1..8192).step_by(2).chain([usize::MAX]));
    assert_eq!(
        shared_edge_face(&even_with_tail, &odd_with_tail),
        Some(usize::MAX)
    );
    let repeated = incidence_with_faces([0, 0, 2, 2, usize::MAX]);
    let other = incidence_with_faces([1, 2, 2, 3, usize::MAX]);
    assert_eq!(shared_edge_face(&repeated, &other), Some(2));
    assert_eq!(shared_edge_face(&other, &repeated), Some(2));
}

#[test]
fn high_valence_sorting_preserves_closed_fans_and_many_boundary_groups() {
    for count in [3, 17, 257, 1024] {
        for disconnected in [false, true] {
            let mut vertices = vec![Point3::try_new(0.0, 0.0, 0.0).unwrap()];
            let mut triangles = Vec::new();
            if disconnected {
                for index in 0..count {
                    let x = (index + 1) as f64;
                    vertices.push(Point3::try_new(x, 0.0, 0.0).unwrap());
                    vertices.push(Point3::try_new(x, 1.0, 0.0).unwrap());
                    triangles.push([0, 2 * index as u32 + 1, 2 * index as u32 + 2]);
                }
            } else {
                for index in 0..count {
                    let angle = std::f64::consts::TAU * index as f64 / count as f64;
                    vertices.push(Point3::try_new(angle.cos(), angle.sin(), 0.0).unwrap());
                    triangles.push([0, index as u32 + 1, ((index + 1) % count) as u32 + 1]);
                }
            }
            let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
            let data = mesh.topology_data();
            let edges = data
                .edges
                .iter()
                .map(|(&(a, b), incidence)| ([a, b], incidence))
                .collect::<Vec<_>>();
            let face_edges = topology_face_edge_indices(&mesh, &data);
            let incident = edges
                .iter()
                .enumerate()
                .filter_map(|(index, (endpoints, _))| endpoints.contains(&0).then_some(index))
                .collect::<Vec<_>>();
            let expected = if disconnected {
                (0..count)
                    .map(|index| vec![2 * index, 2 * index + 1])
                    .collect::<Vec<_>>()
            } else {
                vec![(0..count).collect::<Vec<_>>()]
            };
            assert_eq!(
                radially_sorted_vertex_edges(0, &incident, &edges, &face_edges),
                expected
            );
        }
    }
}

#[test]
fn cyclic_component_order_is_not_first_occurrence_face_order() {
    let incidences = [[0, 2], [0, 1], [1, 2]].map(|faces| {
        let mut incidence = EdgeIncidence::default();
        for face in faces {
            incidence.add_use(EdgeUse {
                face,
                side: 0,
                forward: true,
                raw_vertices: [0, 1],
            });
        }
        incidence
    });
    let edges = incidences
        .iter()
        .enumerate()
        .map(|(index, incidence)| ([0, index + 1], incidence))
        .collect::<Vec<_>>();
    let groups = vec![vec![0, 1, 2]];
    let faces = [0, 1, 2];
    let locals = BTreeMap::from([(0, 0), (1, 1), (2, 2)]);
    assert_eq!(
        radial_vertex_face_walk(&groups, &edges, &faces),
        vec![(0, Some(0)), (1, Some(1)), (2, Some(2))]
    );
    for (mut parents, expected) in [(vec![0, 1, 0], vec![1, 0]), (vec![2, 1, 2], vec![1, 2])] {
        assert_eq!(
            ordered_vertex_face_components(&groups, &edges, &locals, &faces, &mut parents),
            expected
        );
    }
}

#[test]
fn angle_unweld_uses_radial_adjacency_instead_of_all_smooth_face_pairs() {
    for (order, expected_groups) in [
        ([0, 1, 2, 3], [[0, 1, 2, 2], [0, 1, 2, 2]]),
        ([0, 2, 1, 3], [[0, 0, 1, 2], [0, 1, 2, 3]]),
    ] {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ]
        .map(|[x, y, z]| Point3::try_new(x, y, z).unwrap())
        .to_vec();
        let faces = [[0, 1, 2], [1, 0, 3], [0, 1, 4], [1, 0, 5]];
        let source = TriangleMesh::try_new(
            vertices,
            order.map(|index| faces[index]).to_vec(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (output, count) = source
            .unwelded_vertices(std::f64::consts::FRAC_PI_4)
            .unwrap();
        assert_eq!(count, 1);
        let mut raw = [[0; 2]; 4];
        for (face, &original_index) in order.iter().enumerate() {
            let original = source.triangles()[face];
            let rebuilt = output.triangles()[face];
            assert_eq!(
                original.map(|index| source.vertices()[index as usize]),
                rebuilt.map(|index| output.vertices()[index as usize])
            );
            for (corner, &index) in original.iter().enumerate() {
                if index < 2 {
                    raw[original_index][index as usize] = rebuilt[corner];
                }
            }
        }
        for (endpoint, groups) in expected_groups.iter().enumerate() {
            for left in 0..4 {
                for right in 0..4 {
                    assert_eq!(
                        raw[left][endpoint] == raw[right][endpoint],
                        groups[left] == groups[right]
                    );
                }
            }
        }
        let expected_vertices = 4 + expected_groups
            .iter()
            .map(|groups| groups.iter().collect::<BTreeSet<_>>().len())
            .sum::<usize>();
        assert_eq!(output.vertices().len(), expected_vertices);
        assert_eq!(output.area().unwrap(), source.area().unwrap());
    }
}

#[test]
fn non_manifold_radial_order_matches_public_rhino_topology_measurements() {
    // Exact ConnectedEdges results after SortEdges from the twelve-case
    // tools/rhino_oracle/observations/mesh_radial_topology.json record.
    let cases = [
        ([0, 1, 2], [[2, 0, 1, 3], [4, 0, 5, 6]]),
        ([0, 2, 1], [[3, 0, 1, 2], [4, 0, 6, 5]]),
        ([1, 0, 2], [[2, 0, 1, 3], [4, 0, 5, 6]]),
        ([1, 2, 0], [[2, 0, 1, 3], [4, 0, 5, 6]]),
        ([2, 0, 1], [[3, 0, 1, 2], [4, 0, 6, 5]]),
        ([2, 1, 0], [[3, 0, 1, 2], [4, 0, 6, 5]]),
    ];
    for partial in [false, true] {
        for (order, endpoints) in cases {
            let mut vertices = vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
                Point3::try_new(0.0, -1.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 1.0).unwrap(),
            ];
            if partial {
                vertices.push(vertices[1]);
            }
            let faces = [[0, 1, 2], [1, 0, 3], [0, if partial { 5 } else { 1 }, 4]];
            let mesh = TriangleMesh::try_new(
                vertices,
                order.map(|face| faces[face]).to_vec(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let data = mesh.topology_data();
            let edges = data
                .edges
                .iter()
                .map(|(&(a, b), incidence)| ([a, b], incidence))
                .collect::<Vec<_>>();
            let face_edges = topology_face_edge_indices(&mesh, &data);
            let expected = [
                endpoints[0].to_vec(),
                endpoints[1].to_vec(),
                vec![1, 4],
                vec![5, 2],
                vec![3, 6],
            ];
            assert_eq!(data.topological_vertex_count, expected.len());
            for (vertex, expected_edges) in expected.iter().enumerate() {
                let incident = edges
                    .iter()
                    .enumerate()
                    .filter_map(|(index, (endpoints, _))| {
                        endpoints.contains(&vertex).then_some(index)
                    })
                    .collect::<Vec<_>>();
                let actual = radially_sorted_vertex_edges(vertex, &incident, &edges, &face_edges)
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(
                    &actual, expected_edges,
                    "partial={partial}, order={order:?}, vertex={vertex}"
                );
            }
        }
    }
}
