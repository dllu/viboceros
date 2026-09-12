use super::*;

#[test]
fn welding_compaction_matches_independent_partition_reference() {
    let points = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 1.0, 0.0).unwrap(),
    ];
    // The third copy is unused, but its vertices may become representatives
    // of used vertices. Unmerged unused components must still disappear.
    let mesh = TriangleMesh::try_new(
        points.repeat(3),
        vec![[0, 1, 2], [5, 4, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    // Every partition of three copies, independently at each location.
    let partitions = [[0, 1, 2], [0, 0, 1], [0, 1, 0], [0, 1, 1], [0, 0, 0]];
    for partition_code in 0..125 {
        for survivor_bits in 0..8 {
            let mut parents = (0..9).collect::<Vec<_>>();
            let mut expected_roots = [0; 9];
            let mut remaining = partition_code;
            for location in 0..3 {
                let labels = partitions[remaining % 5];
                remaining /= 5;
                for copy in 0..3 {
                    let mut members = (0..3)
                        .filter(|&other| labels[other] == labels[copy])
                        .map(|other| other * 3 + location)
                        .collect::<Vec<_>>();
                    if survivor_bits & (1 << location) == 0 {
                        members.reverse();
                    }
                    let raw = copy * 3 + location;
                    let position = members.iter().position(|&member| member == raw).unwrap();
                    // Use chains, not just stars, to exercise root resolution
                    // before the parent storage is repurposed for remapping.
                    parents[raw] = members[(position + 1).min(members.len() - 1)];
                    expected_roots[raw] = *members.last().unwrap();
                }
            }
            let mut representatives = expected_roots[..6].to_vec();
            representatives.sort_unstable();
            representatives.dedup();
            let expected_vertices = representatives
                .iter()
                .map(|&raw| mesh.vertices()[raw])
                .collect::<Vec<_>>();
            let expected_triangles = mesh
                .triangles()
                .iter()
                .map(|triangle| {
                    triangle.map(|raw| {
                        representatives
                            .iter()
                            .position(|&root| root == expected_roots[raw as usize])
                            .unwrap() as u32
                    })
                })
                .collect::<Vec<_>>();
            let (compacted, removed) = mesh.compacted_with_vertex_parents(&mut parents);
            assert_eq!(compacted.vertices(), expected_vertices);
            assert_eq!(compacted.triangles(), expected_triangles);
            assert_eq!(removed, 9 - representatives.len());
            assert_eq!(compacted.area().unwrap(), mesh.area().unwrap());
        }
    }
}

#[test]
fn welding_compaction_preserves_quad_kind_and_unchanged_meshes() {
    let points = [
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 1.0, 0.0).unwrap(),
        Point3::try_new(0.0, 1.0, 0.0).unwrap(),
    ];
    let mesh = TriangleMesh::try_new_faces(
        points.repeat(2),
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([6, 5, 4])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (compacted, removed) = mesh.compacted_with_vertex_parents(&mut [4, 5, 6, 7, 4, 5, 6, 7]);
    assert_eq!(removed, 4);
    assert_eq!(compacted.vertices(), points);
    assert_eq!(
        compacted.faces(),
        &[MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([2, 1, 0])]
    );
    assert_eq!(
        compacted.compacted_with_vertex_parents(&mut [0, 1, 2, 3]),
        (compacted.clone(), 0)
    );
}
