use super::*;
use monstertruck::step::load::Table;
use viboceros_geometry::MeshFace;

fn point(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn check_boundary_topology(mesh: &TriangleMesh, expected_edges: usize) {
    let shell = mesh_to_shell(mesh).unwrap();
    assert_eq!(shell.edges.len(), expected_edges);
    assert_eq!(shell.faces.len(), mesh.triangles().len());
    for (face, triangle) in shell.faces.iter().zip(mesh.triangles()) {
        assert!(face.orientation);
        assert_eq!(face.boundaries.len(), 1);
        assert_eq!(face.boundaries[0].len(), 3);
        for (side, edge_use) in face.boundaries[0].iter().enumerate() {
            let (first, second) = shell.edges[edge_use.index].vertices;
            let directed = if edge_use.orientation {
                (first, second)
            } else {
                (second, first)
            };
            assert_eq!(
                directed,
                (triangle[side] as usize, triangle[(side + 1) % 3] as usize)
            );
            for raw in [first, second] {
                let actual = shell.vertices[raw].0;
                let expected = mesh.vertices()[raw];
                assert_eq!(
                    [actual.x, actual.y, actual.z],
                    [expected.x(), expected.y(), expected.z()]
                );
            }
        }
    }
    let mut bytes = Vec::new();
    write_step(&mut bytes, std::slice::from_ref(mesh)).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let table = Table::from_step(&text).unwrap();
    assert_eq!(table.entity_report.total(), 0);
    assert_eq!(table.shell.len(), if expected_edges == 5 { 1 } else { 2 });
    assert_eq!(text.matches("EDGE_CURVE(").count(), expected_edges);
    assert_eq!(
        text.matches("ADVANCED_FACE(").count(),
        mesh.triangles().len()
    );
}

#[test]
fn disconnected_panels_partition_and_remap_in_first_face_order() {
    for count in [2, 17, 257] {
        let mut vertices = Vec::new();
        for panel in 0..count {
            let x = panel as f64 * 10.0;
            vertices.extend([
                point(x, 0.0),
                point(x + 2.0, 0.0),
                point(x + 2.0, 1.0),
                point(x, 1.0),
            ]);
        }
        let mut triangles = Vec::new();
        for side in 0..2 {
            for panel in 0..count {
                let base = (4 * panel) as u32;
                triangles.push(if side == 0 {
                    [base, base + 1, base + 2]
                } else {
                    [base, base + 2, base + 3]
                });
            }
        }
        let mesh = TriangleMesh::try_new(vertices, triangles, Tolerance::DEFAULT).unwrap();
        let pieces = components::partition(mesh_to_shell(&mesh).unwrap());
        assert_eq!(pieces.len(), count);
        for (panel, piece) in pieces.iter().enumerate() {
            assert_eq!(
                (piece.vertices.len(), piece.edges.len(), piece.faces.len()),
                (4, 5, 2)
            );
            for (side, face) in piece.faces.iter().enumerate() {
                for (corner, edge_use) in face.boundaries[0].iter().enumerate() {
                    let edge = &piece.edges[edge_use.index];
                    let raw = if edge_use.orientation {
                        edge.vertices.0
                    } else {
                        edge.vertices.1
                    };
                    let actual = piece.vertices[raw].0;
                    let expected =
                        mesh.vertices()[mesh.triangles()[side * count + panel][corner] as usize];
                    assert_eq!(
                        [actual.x, actual.y, actual.z],
                        [expected.x(), expected.y(), expected.z()]
                    );
                }
            }
        }
        if count == 2 {
            let mut bytes = Vec::new();
            write_step(&mut bytes, &[mesh]).unwrap();
            let imported =
                crate::read_step(std::io::Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
            assert_eq!(imported.objects.len(), 2);
            for object in imported.objects {
                assert_eq!(object.mesh.triangles().len(), 2);
                assert_eq!(object.mesh.area().unwrap(), 2.0);
            }
        }
    }
}

#[test]
fn exported_topology_preserves_raw_seams_and_directed_face_boundaries() {
    for split_a in [false, true] {
        for split_b in [false, true] {
            for flipped in [false, true] {
                for reversed in [false, true] {
                    let mut faces = vec![
                        [0, 1, 2],
                        [if split_b { 4 } else { 1 }, if split_a { 3 } else { 0 }, 5],
                    ];
                    if flipped {
                        for face in &mut faces {
                            face.swap(1, 2);
                        }
                    }
                    if reversed {
                        faces.reverse();
                    }
                    let mesh = TriangleMesh::try_new(
                        vec![
                            point(0.0, 0.0),
                            point(2.0, 0.0),
                            point(0.0, 1.0),
                            point(0.0, 0.0),
                            point(2.0, 0.0),
                            point(0.0, -1.0),
                        ],
                        faces,
                        Tolerance::DEFAULT,
                    )
                    .unwrap();
                    check_boundary_topology(&mesh, if split_a || split_b { 6 } else { 5 });
                }
            }
        }
    }
}

#[test]
fn exported_quad_triangulation_shares_only_its_internal_diagonal() {
    for face in [MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([0, 3, 2, 1])] {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(2.0, 1.0),
                point(0.0, 1.0),
            ],
            vec![face],
            Tolerance::DEFAULT,
        )
        .unwrap();
        check_boundary_topology(&mesh, 5);
        let shell = mesh_to_shell(&mesh).unwrap();
        let shared = shell.faces[0].boundaries[0]
            .iter()
            .filter(|first| {
                shell.faces[1].boundaries[0]
                    .iter()
                    .any(|second| first.index == second.index)
            })
            .collect::<Vec<_>>();
        assert_eq!(shared.len(), 1);
        let other = shell.faces[1].boundaries[0]
            .iter()
            .find(|edge| edge.index == shared[0].index)
            .unwrap();
        assert_ne!(shared[0].orientation, other.orientation);
    }
}
