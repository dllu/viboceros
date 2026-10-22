use viboceros_geometry::{Point3, SubD};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn cube() -> SubD {
    SubD::try_from_polygons(
        vec![
            p(0., 0., 0.),
            p(4., 0., 0.),
            p(4., 3., 0.),
            p(0., 3., 0.),
            p(0., 0., 2.),
            p(4., 0., 2.),
            p(4., 3., 2.),
            p(0., 3., 2.),
        ],
        vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
    )
    .unwrap()
}
fn verify(subd: &SubD) {
    let archive = subd.archive().to_vec();
    let coarse = subd.edge_polylines(1).unwrap();
    for density in [1, 2, 3] {
        let lines = subd.edge_polylines(density).unwrap();
        assert_eq!(lines.len(), subd.edges().len());
        let mesh = subd.tessellate(density).unwrap();
        for (index, (line, &[a, b])) in lines.iter().zip(subd.edges()).enumerate() {
            assert_eq!(line.len(), (1 << density) + 1);
            assert!(
                line[0]
                    .distance_to(subd.surface_points()[a as usize])
                    .unwrap()
                    < 1e-10
            );
            assert!(
                line.last()
                    .unwrap()
                    .distance_to(subd.surface_points()[b as usize])
                    .unwrap()
                    < 1e-10
            );
            for (sample, &point) in line.iter().step_by(1 << (density - 1)).zip(&coarse[index]) {
                assert!(sample.distance_to(point).unwrap() < 1e-10);
            }
            for point in line {
                assert!(
                    mesh.vertices()
                        .iter()
                        .any(|&q| point.distance_to(q).unwrap() < 1e-10)
                );
            }
            // Every sampled segment is an edge of the independently collected
            // display topology, rather than a shortcut across a source face.
            for pair in line.windows(2) {
                assert!(mesh.faces().iter().any(|face| {
                    let v = face.indices();
                    (0..v.len()).any(|i| {
                        let a = mesh.vertices()[v[i] as usize];
                        let b = mesh.vertices()[v[(i + 1) % v.len()] as usize];
                        (a.distance_to(pair[0]).unwrap() < 1e-10
                            && b.distance_to(pair[1]).unwrap() < 1e-10)
                            || (b.distance_to(pair[0]).unwrap() < 1e-10
                                && a.distance_to(pair[1]).unwrap() < 1e-10)
                    })
                }));
            }
        }
    }
    assert_eq!(subd.archive(), archive);
}
#[test]
fn original_edge_lineage_survives_corners_ngons_and_active_refinement_levels() {
    let smooth = cube();
    verify(&smooth);
    verify(&smooth.try_crease_components(&[0, 1], &[0], true).unwrap().0);
    verify(&smooth.subdivided(1).unwrap());
    verify(&smooth.subdivided_faces(&[0]).unwrap());
    for count in [3, 5, 9] {
        let points = (0..count)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / count as f64;
                p(a.cos() * 3., a.sin() * 3., (i % 2) as f64)
            })
            .collect();
        verify(&SubD::try_from_polygons(points, vec![(0..count).collect()]).unwrap());
    }
}
#[test]
fn coincident_limit_vertices_do_not_merge_distinct_source_edge_ordinals() {
    let source = cube();
    let points = source
        .control_points()
        .iter()
        .copied()
        .cycle()
        .take(16)
        .collect();
    let mut faces = source.faces().to_vec();
    faces.extend(
        source
            .faces()
            .iter()
            .map(|f| f.iter().map(|i| i + 8).collect()),
    );
    let coincident = SubD::try_from_polygons(points, faces).unwrap();
    let lines = coincident.edge_polylines(3).unwrap();
    assert_eq!(lines.len(), 24);
    assert_eq!(&lines[..12], &lines[12..]);
}
#[test]
fn sample_density_and_work_budgets_fail_before_excessive_refinement() {
    let source = cube();
    assert!(source.edge_polylines(0).is_err());
    assert!(source.edge_polylines(7).is_err());
    assert_eq!(source.edge_polylines(6).unwrap()[0].len(), 65);
    let points = (0..=30)
        .flat_map(|y| (0..=30).map(move |x| p(x as f64, y as f64, 0.)))
        .collect();
    let faces = (0..30)
        .flat_map(|y| {
            (0..30).map(move |x| {
                let i = y * 31 + x;
                vec![i, i + 1, i + 32, i + 31]
            })
        })
        .collect();
    let grid = SubD::try_from_polygons(points, faces).unwrap();
    assert!(grid.edge_polylines(6).is_err());
    assert_eq!(
        grid.display_edge_polylines().unwrap().len(),
        grid.edges().len()
    );
}
