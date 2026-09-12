use super::*;

#[test]
fn high_valence_fans_keep_all_faces_when_each_radial_edge_is_separated() {
    for count in [3, 17, 257] {
        let mut vertices = vec![Point3::try_new(0.0, 0.0, 0.0).unwrap()];
        for index in 0..count {
            let angle = std::f64::consts::TAU * index as f64 / count as f64;
            vertices.push(Point3::try_new(angle.cos(), angle.sin(), 0.0).unwrap());
        }
        let triangles = (0..count)
            .map(|index| [0, index as u32 + 1, ((index + 1) % count) as u32 + 1])
            .collect();
        let source = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
        let selected = (0..count).collect::<Vec<_>>();
        for (output, separated) in [
            source.unwelded_topology_edges(&selected).unwrap(),
            source.unwelded_vertices(0.0).unwrap(),
        ] {
            assert_eq!(separated, count);
            assert_eq!(output.vertices().len(), 3 * count);
            assert_eq!(output.triangles().len(), count);
            let used = output
                .triangles()
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>();
            assert_eq!(used.len(), 3 * count);
            for (original, rebuilt) in source.triangles().iter().zip(output.triangles()) {
                assert_eq!(
                    original.map(|raw| source.vertices()[raw as usize]),
                    rebuilt.map(|raw| output.vertices()[raw as usize])
                );
            }
            assert_eq!(source.area().unwrap(), output.area().unwrap());
        }
    }
}

#[test]
fn sparse_selected_edges_across_disconnected_panels_keep_local_connectivity() {
    for panel_count in [1, 17, 257, 1024] {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut selected = Vec::new();
        for panel in 0..panel_count {
            let base = vertices.len() as u32;
            let x = 10.0 * panel as f64;
            for coordinates in [
                [x, 0.0, 0.0],
                [x + 4.0, 0.0, 0.0],
                [x, 3.0, 0.0],
                [x, -3.0, 0.0],
                [x, 99.0, 99.0],
            ] {
                vertices
                    .push(Point3::try_new(coordinates[0], coordinates[1], coordinates[2]).unwrap());
            }
            triangles.extend([[base, base + 1, base + 2], [base + 1, base, base + 3]]);
            // Five ordered topology edges per disconnected panel. The unused
            // source vertex has no edge and must be removed from every panel.
            if panel % 2 == 0 {
                selected.push(5 * panel);
            }
        }
        let source = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
        let (output, count) = source.unwelded_topology_edges(&selected).unwrap();
        assert_eq!(count, selected.len());
        assert_eq!(
            output.vertices().len(),
            4 * panel_count + 2 * selected.len()
        );
        assert_eq!(output.triangles().len(), 2 * panel_count);
        for panel in 0..panel_count {
            let first = output.triangles()[2 * panel];
            let second = output.triangles()[2 * panel + 1];
            assert_eq!(first[0] == second[1], panel % 2 != 0);
            assert_eq!(first[1] == second[0], panel % 2 != 0);
        }
        for (original, rebuilt) in source.triangles().iter().zip(output.triangles()) {
            assert_eq!(
                original.map(|raw| source.vertices()[raw as usize]),
                rebuilt.map(|raw| output.vertices()[raw as usize])
            );
        }
        assert_eq!(source.area().unwrap(), output.area().unwrap());
    }
}

#[test]
fn non_manifold_endpoint_partitions_survive_face_permutation_and_winding() {
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let vertices = vec![
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(0.0, 1.0, 0.0),
        point(0.0, -1.0, 0.0),
        point(0.0, 0.0, 1.0),
        point(99.0, 99.0, 99.0),
    ];
    let partitions = [[0, 0, 0], [0, 0, 1], [0, 1, 0], [0, 1, 1], [0, 1, 2]];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for first in partitions {
        for second in partitions {
            let labels = [first, second];
            let fully_shared = labels.map(|partition| partition == [0, 0, 0]);
            let expected_count = usize::from(fully_shared.iter().any(|&shared| shared));
            let expected_vertices = 3 + labels
                .iter()
                .zip(fully_shared)
                .map(|(partition, shared)| {
                    if shared {
                        3
                    } else {
                        partition.iter().collect::<BTreeSet<_>>().len()
                    }
                })
                .sum::<usize>();
            for order in orders {
                for reversed in [false, true] {
                    let triangles = order
                        .map(|face| {
                            let mut triangle =
                                [2 * first[face], 2 * second[face] + 1, 6 + face as u32];
                            if (face == 1) != reversed {
                                triangle.swap(0, 1);
                            }
                            triangle
                        })
                        .to_vec();
                    let source =
                        TriangleMesh::try_new(vertices.clone(), triangles, Tolerance::DEFAULT)
                            .unwrap();
                    let (output, count) = source.unwelded_topology_edges(&[0, 0]).unwrap();
                    assert_eq!(count, expected_count);
                    assert_eq!(output.vertices().len(), expected_vertices);
                    let mut output_endpoints = [[0; 2]; 3];
                    for (output_face, (&original_face, triangle)) in
                        order.iter().zip(source.triangles()).enumerate()
                    {
                        let rebuilt = output.triangles()[output_face];
                        assert_eq!(
                            triangle.map(|raw| source.vertices()[raw as usize]),
                            rebuilt.map(|raw| output.vertices()[raw as usize])
                        );
                        for (corner, &raw) in triangle.iter().enumerate() {
                            if raw < 6 {
                                output_endpoints[original_face][raw as usize % 2] = rebuilt[corner];
                            }
                        }
                    }
                    for endpoint in 0..2 {
                        for left in 0..3 {
                            for right in 0..3 {
                                let expected_shared = if fully_shared[endpoint] {
                                    left == right
                                } else {
                                    labels[endpoint][left] == labels[endpoint][right]
                                };
                                assert_eq!(
                                    output_endpoints[left][endpoint]
                                        == output_endpoints[right][endpoint],
                                    expected_shared
                                );
                            }
                        }
                    }
                    assert_eq!(source.area().unwrap(), output.area().unwrap());
                }
            }
        }
    }
}
