use super::*;

fn sheet(x: [Real; 2], z: Real, vector: [Real; 3]) -> Brep {
    let points = [0., 1.]
        .into_iter()
        .flat_map(|v| {
            x.map(|x| Point3::try_new(x + v * vector[0], v * vector[1], z + v * vector[2]).unwrap())
        })
        .collect();
    Brep::try_surface_face(
        NurbsSurface::try_clamped_uniform(1, 1, 2, 2, points).unwrap(),
        Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap(),
    )
    .unwrap()
}

fn tolerance() -> Tolerance {
    Tolerance::try_new(0.001, 1e-12, 1e-10).unwrap()
}

#[test]
fn complete_short_shared_edges_keep_distinct_vertices_and_original_geometry() {
    for origin in [0., 1000., 1e9] {
        for length in [1e-6, 0.0001, 0.0015] {
            let a = sheet([origin, origin + length], 0., [0., 0., 3.]);
            let b = sheet([origin, origin + length], 0., [0., 3., 0.]);
            let before = [a.clone(), b.clone()];
            for distance in [0., 0.002] {
                let result = join_breps(&[&a, &b], distance, tolerance()).unwrap();
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].joined_edge_count, 1);
                let result = &result[0].brep;
                assert_eq!((result.vertices.len(), result.edges.len()), (6, 7));
                for v in &result.vertices {
                    assert_eq!(v.tolerance, 0.);
                    assert!(
                        before
                            .iter()
                            .any(|b| b.vertices.iter().any(|old| old.point == v.point))
                    );
                }
                assert!(
                    result
                        .edges
                        .iter()
                        .all(|e| result.vertices[e.vertices[0]].point
                            != result.vertices[e.vertices[1]].point)
                );
                for (new, old) in result.faces.iter().zip(&before) {
                    assert_eq!(new.surface, old.faces[0].surface);
                    assert_eq!(
                        new.loops[0]
                            .trims
                            .iter()
                            .map(|t| &t.curve)
                            .collect::<Vec<_>>(),
                        old.faces[0].loops[0]
                            .trims
                            .iter()
                            .map(|t| &t.curve)
                            .collect::<Vec<_>>()
                    );
                }
            }
            assert_eq!([a, b], before);
        }
    }
}

#[test]
fn contained_short_edges_join_without_collapsing_original_or_inserted_tips() {
    for (interval, vertices, edges) in [([2., 2.0001], 8, 9), ([0., 0.0001], 7, 8)] {
        let a = sheet([0., 4.], 0., [0., 0., 3.]);
        let b = sheet(interval, 0., [0., 3., 0.]);
        let result = join_breps(&[&a, &b], 0.002, tolerance()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].joined_edge_count, 1);
        let result = &result[0].brep;
        assert_eq!(
            (result.vertices.len(), result.edges.len()),
            (vertices, edges)
        );
        assert!(result.vertices.iter().all(|v| v.tolerance == 0.));
        for point in b.vertices.iter().map(|v| v.point) {
            assert!(result.vertices.iter().any(|v| v.point == point));
        }
    }
}

#[test]
fn crossing_tip_contacts_rebuild_without_inventing_a_short_shared_edge() {
    let a = sheet([0., 4.], 0., [0., 0., 3.]);
    let b = sheet([3.9995, 7.9995], 0., [0., 3., 0.]);
    let result = join_breps_with_report(&[&a, &b], 0.002, tolerance()).unwrap();
    assert!(result.candidate_source_pairs.is_empty());
    assert_eq!(result.components.len(), 2);
    for part in &result.components {
        assert_eq!(part.joined_edge_count, 0);
        assert_eq!((part.brep.vertices.len(), part.brep.edges.len()), (4, 4));
        assert!(
            part.brep
                .vertices
                .iter()
                .any(|v| v.point == Point3::try_new(4_f64.midpoint(3.9995), 0., 0.).unwrap())
        );
    }
    let resolved = join_breps(&[&a, &b], 0.0001, tolerance()).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].joined_edge_count, 1);
    assert_eq!(resolved[0].brep.edges.len(), 9);
}

#[test]
fn opposed_sheet_gaps_do_not_create_contradictory_tiny_side_mates() {
    let a = sheet([0., 4.], 0., [0., 0., 3.]);
    for (gap, edges, mates) in [(0.0005, 7, 1), (0.0022, 10, 2)] {
        let b = sheet([0., 4.], gap, [0., 0., -3.]);
        let before = [a.clone(), b.clone()];
        let result = join_breps(&[&a, &b], 0.002, tolerance()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].joined_edge_count, mates);
        assert_eq!(result[0].brep.edges.len(), edges);
        assert_eq!([a.clone(), b], before);
        if mates == 1 {
            assert_eq!(
                result[0]
                    .brep
                    .faces
                    .iter()
                    .map(|f| f.reversed)
                    .collect::<Vec<_>>(),
                [false, true]
            );
        }
    }
}
