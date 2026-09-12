use super::*;
use monstertruck::topology::compress::{CompressedEdge, CompressedEdgeIndex, CompressedFace};

#[test]
fn partition_matches_independent_reachability_for_every_graph_up_to_five_faces() {
    for count in 0..=5 {
        let pairs = (0..count)
            .flat_map(|a| (a + 1..count).map(move |b| (a, b)))
            .collect::<Vec<_>>();
        for mask in 0..(1_usize << pairs.len()) {
            // Abstract incidence fixtures isolate the partitioner's contract
            // from surface geometry. Each curve stores its original edge ID.
            let mut shell = CompressedShell {
                vertices: vec![10, 20],
                edges: Vec::new(),
                faces: (0..count)
                    .map(|face| CompressedFace {
                        boundaries: vec![Vec::new()],
                        orientation: true,
                        surface: face,
                    })
                    .collect::<Vec<_>>(),
                vertex_stable_ids: None,
                edge_stable_ids: None,
                face_stable_ids: None,
            };
            let mut adjacency = vec![vec![false; count]; count];
            for (bit, &(a, b)) in pairs.iter().enumerate() {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                adjacency[a][b] = true;
                adjacency[b][a] = true;
                let index = shell.edges.len();
                shell.edges.push(CompressedEdge {
                    vertices: (0, 1),
                    curve: index,
                });
                shell.faces[a].boundaries[0].push(CompressedEdgeIndex {
                    index,
                    orientation: true,
                });
                shell.faces[b].boundaries[0].push(CompressedEdgeIndex {
                    index,
                    orientation: false,
                });
            }
            let source_boundaries = shell
                .faces
                .iter()
                .map(|face| {
                    face.boundaries[0]
                        .iter()
                        .map(|edge| (edge.index, edge.orientation))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut expected = Vec::new();
            let mut visited = vec![false; count];
            for seed in 0..count {
                if visited[seed] {
                    continue;
                }
                let mut pending = vec![seed];
                visited[seed] = true;
                let mut group = Vec::new();
                while let Some(face) = pending.pop() {
                    group.push(face);
                    for next in 0..count {
                        if adjacency[face][next] && !visited[next] {
                            visited[next] = true;
                            pending.push(next);
                        }
                    }
                }
                group.sort_unstable();
                expected.push(group);
            }
            let pieces = partition(shell);
            assert_eq!(
                pieces
                    .iter()
                    .map(|piece| piece
                        .faces
                        .iter()
                        .map(|face| face.surface)
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                expected
            );
            for piece in pieces {
                for edge in &piece.edges {
                    assert_eq!(
                        (
                            piece.vertices[edge.vertices.0],
                            piece.vertices[edge.vertices.1]
                        ),
                        (10, 20)
                    );
                }
                for face in piece.faces {
                    let actual = face.boundaries[0]
                        .iter()
                        .map(|edge| (piece.edges[edge.index].curve, edge.orientation))
                        .collect::<Vec<_>>();
                    assert_eq!(actual, source_boundaries[face.surface]);
                }
            }
        }
    }
}
