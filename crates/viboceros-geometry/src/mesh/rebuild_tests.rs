use super::*;

#[test]
fn corner_rebuilding_preserves_geometry_and_requested_sharing_partitions() {
    let points = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 2.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
    ];
    let mut vertices = points.repeat(2);
    vertices.push(Point3::try_new(99.0, 99.0, 99.0).unwrap());
    let source = TriangleMesh::try_new_faces(
        vertices,
        vec![
            MeshFace::Quad([0, 1, 2, 3]),
            MeshFace::Triangle([4, 6, 5]),
            MeshFace::Quad([7, 6, 5, 4]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let data = source.topology_data();
    for mask in 0..16 {
        for separate_faces in [false, true] {
            for reverse_order in [false, true] {
                let affected = |raw: u32| mask & (1 << (raw % 4)) != 0;
                let mut components = vec![Vec::new(); data.topological_vertex_count];
                for raw in 0..4 {
                    if !affected(raw) {
                        continue;
                    }
                    let topology = data.topological_vertices[raw as usize];
                    let incident = source
                        .faces()
                        .iter()
                        .enumerate()
                        .filter_map(|(index, face)| {
                            face.indices()
                                .iter()
                                .any(|&other| other % 4 == raw)
                                .then_some(index)
                        })
                        .collect::<Vec<_>>();
                    components[topology] = if separate_faces {
                        incident.into_iter().map(|face| vec![face]).collect()
                    } else {
                        vec![incident]
                    };
                }
                let mut order = (0..data.topological_vertex_count).collect::<Vec<_>>();
                if reverse_order {
                    order.reverse();
                }
                let rebuilt = source
                    .rebuilt_from_face_components(&data, &components, &order)
                    .unwrap();
                let mut corners = Vec::new();
                for (face_index, (original, output)) in
                    source.faces().iter().zip(rebuilt.faces()).enumerate()
                {
                    assert_eq!(original.is_quad(), output.is_quad());
                    for (&raw, &target) in original.indices().iter().zip(output.indices()) {
                        assert_eq!(
                            source.vertices()[raw as usize],
                            rebuilt.vertices()[target as usize]
                        );
                        corners.push((face_index, raw, target));
                    }
                }
                // Compare all sharing relationships without using topology
                // roots, corner slots, or the rebuilding routine as an oracle.
                for &(first_face, first_raw, first_target) in &corners {
                    for &(second_face, second_raw, second_target) in &corners {
                        let should_share = if affected(first_raw) {
                            first_raw % 4 == second_raw % 4
                                && (!separate_faces || first_face == second_face)
                        } else {
                            first_raw == second_raw
                        };
                        assert_eq!(first_target == second_target, should_share);
                    }
                }
                let used = corners
                    .iter()
                    .map(|&(_, _, target)| target)
                    .collect::<BTreeSet<_>>();
                assert_eq!(used.len(), rebuilt.vertices().len());
                assert_eq!(rebuilt.area().unwrap(), source.area().unwrap());
            }
        }
    }
}
