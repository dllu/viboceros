use std::collections::BTreeSet;
use viboceros_geometry::{Point3, SubD};
fn cube() -> SubD {
    SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [4., 0., 0.],
            [4., 3., 0.],
            [0., 3., 0.],
            [0., 0., 2.],
            [4., 0., 2.],
            [4., 3., 2.],
            [0., 3., 2.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![
            vec![0, 3, 2, 1],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
            vec![4, 5, 6, 7],
        ],
    )
    .unwrap()
}
fn edge(s: &SubD, a: u32, b: u32) -> usize {
    s.edges()
        .iter()
        .position(|p| *p == [a, b] || *p == [b, a])
        .unwrap()
}
#[test]
fn native_chains_partition_paths_cycles_branches_and_disconnected_selections() {
    let s = cube();
    let before = s.clone();
    for (pairs, count, closed) in [
        (vec![(0, 1), (1, 2), (2, 3)], 1, 0),
        (vec![(0, 1), (1, 2), (2, 3), (3, 0)], 1, 1),
        (vec![(0, 1), (0, 3), (0, 4)], 3, 0),
        (vec![(0, 1), (6, 7)], 2, 0),
    ] {
        let selected = pairs
            .into_iter()
            .map(|(a, b)| edge(&s, a, b))
            .collect::<Vec<_>>();
        let chains = s.try_edge_chains(&selected).unwrap();
        assert_eq!(chains.len(), count);
        assert_eq!(chains.iter().filter(|c| c.is_closed()).count(), closed);
        let mut seen = BTreeSet::new();
        for c in &chains {
            assert_eq!(c.vertices.len(), c.edges.len() + 1);
            for (e, v) in c.edges.iter().zip(c.vertices.windows(2)) {
                assert!(seen.insert(e.index));
                let p = s.edges()[e.index];
                let expected = if e.reversed {
                    [p[1] as usize, p[0] as usize]
                } else {
                    [p[0] as usize, p[1] as usize]
                };
                assert_eq!(v, expected);
            }
        }
        assert_eq!(seen, selected.iter().copied().collect());
        let reversed = selected.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(chains, s.try_edge_chains(&reversed).unwrap());
        assert_eq!(s, before);
    }
}
#[test]
fn invalid_chain_requests_leave_geometry_and_soft_weights_unchanged() {
    let (s, _) = cube()
        .try_with_soft_crease_weights(&[(0, [24.75, 25.25])])
        .unwrap();
    let before = s.clone();
    for selected in [vec![usize::MAX], vec![0, 0], vec![0, usize::MAX]] {
        assert!(s.try_edge_chains(&selected).is_err());
        assert_eq!(s, before);
    }
    assert!(s.try_edge_chains(&[]).unwrap().is_empty());
    assert_eq!(s, before);
}

#[test]
fn boundary_edges_remain_available_to_the_generic_chain_query() {
    let source = SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let selected = (0..source.edges().len()).collect::<Vec<_>>();
    let chains = source.try_edge_chains(&selected).unwrap();
    assert_eq!(chains.len(), 1);
    assert!(chains[0].is_closed());
    assert_eq!(chains[0].edges.len(), 4);
    assert!(source.edge_face_counts().iter().all(|&n| n == 1));
}

#[test]
fn native_interpolation_uses_unrounded_requests_and_sanitizes_each_interval_once() {
    let high = SubD::soft_crease_sharpness(80.).unwrap();
    assert_eq!(high, 3.2);
    assert_ne!(high, 3.2_f32 as f64);
    assert_eq!(
        SubD::interpolate_edge_sharpness([high, 2.], 3).unwrap(),
        vec![
            [3.2_f32 as f64, 2.8_f32 as f64],
            [2.8_f32 as f64, 2.4_f32 as f64],
            [2.4_f32 as f64, 2.]
        ]
    );
    let (source, _) = cube()
        .try_with_soft_crease_weights(&[(0, [24.75; 2])])
        .unwrap();
    let raw = source.edge_sharpness()[0];
    assert_eq!(raw, [0.99_f32 as f64; 2]);
    let weights = SubD::interpolate_edge_sharpness([high, raw[0]], 3).unwrap();
    assert_eq!(weights[2][1], 1.); // Explicit interpolation re-sanitizes this endpoint.
    assert_eq!(weights[0][1], 2.4633334_f32 as f64); // Interior arithmetic still uses the original float.
    let restored = cube()
        .try_with_stored_edge_sharpness(&[(0, raw)])
        .unwrap()
        .0;
    assert_eq!(restored.edge_sharpness()[0], raw); // Applying stored values does not re-sanitize.
    assert_eq!(restored.surface_points(), source.surface_points());
    assert_eq!(
        restored.tessellate(2).unwrap(),
        source.tessellate(2).unwrap()
    );
    assert!(
        !restored
            .try_with_stored_edge_sharpness(&[(0, raw)])
            .unwrap()
            .1
    );
    for pair in [[f64::NAN, 0.], [-1., 0.], [0., 5.]] {
        assert!(SubD::interpolate_edge_sharpness(pair, 1).is_err());
    }
    for n in [0, 400_001, usize::MAX] {
        assert!(SubD::interpolate_edge_sharpness([0., 4.], n).is_err());
    }
    for p in [f64::NAN, -1., 101., f64::INFINITY] {
        assert!(SubD::soft_crease_sharpness(p).is_err());
    }
    for pair in [[0.99, 0.], [f64::NAN, 0.], [-1., 0.], [5., 0.]] {
        assert!(
            restored
                .try_with_stored_edge_sharpness(&[(0, pair)])
                .is_err()
        );
    }
}
