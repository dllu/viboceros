use super::super::{EdgeUse, Point3, Tolerance, TriangleMesh, topology_face_edge_indices};
use super::*;

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
