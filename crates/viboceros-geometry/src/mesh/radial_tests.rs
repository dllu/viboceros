use super::*;

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
