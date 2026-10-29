use viboceros_geometry::{Point3, PolygonEdgePoint, SubD, Tolerance, Vector3};
fn source() -> SubD {
    SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [2., 0., 0.],
            [4., 0., 0.],
            [0., 3., 0.],
            [2., 3., 0.],
            [4., 3., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
}
fn pick(s: &SubD, a: u32, b: u32, t: f64) -> PolygonEdgePoint {
    let edge = s
        .edges()
        .iter()
        .position(|e| e == &[a, b] || e == &[b, a])
        .unwrap();
    PolygonEdgePoint {
        edge,
        parameter: if s.edges()[edge][0] == a { t } else { 1. - t },
    }
}
#[test]
fn invalid_later_picks_and_native_chords_never_mutate_the_source_archive() {
    let s = source();
    let before = s.archive().to_vec();
    let good = pick(&s, 0, 3, 0.5);
    for bad in [
        PolygonEdgePoint {
            edge: usize::MAX,
            ..good
        },
        PolygonEdgePoint {
            parameter: f64::NAN,
            ..good
        },
        PolygonEdgePoint {
            parameter: f64::INFINITY,
            ..good
        },
        PolygonEdgePoint {
            parameter: -1.,
            ..good
        },
        PolygonEdgePoint {
            parameter: 1.1,
            ..good
        },
    ] {
        assert!(
            s.try_insert_edge_path(&[good, bad], None, Tolerance::default())
                .is_err()
        );
        assert_eq!(s.archive(), before);
    }
    let p = [0., 1.5, 0.];
    for (splits, chords) in [
        (vec![(usize::MAX, 0.5, p)], vec![]),
        (vec![(good.edge, 0.5, p), (good.edge, 0.5, p)], vec![]),
        (vec![(good.edge, 0.5, p)], vec![[0, u32::MAX]]),
    ] {
        assert!(viboceros_opennurbs::subd_insert_path(s.archive(), &splits, &chords).is_err());
        assert_eq!(s.archive(), before);
        SubD::try_from_archive(s.archive()).unwrap();
    }
}
#[test]
fn disconnected_picks_do_not_bridge_separate_components() {
    let s = SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [5., 0., 0.],
            [6., 0., 0.],
            [6., 1., 0.],
            [5., 1., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]],
    )
    .unwrap();
    let before = s.archive().to_vec();
    assert!(
        s.try_insert_edge_path(
            &[pick(&s, 0, 3, 0.5), pick(&s, 5, 6, 0.5)],
            None,
            Tolerance::DEFAULT,
        )
        .is_err()
    );
    assert_eq!(s.archive(), before);
}
#[test]
fn default_face_projection_and_translated_rotated_paths_preserve_crossings() {
    let s = source();
    let picks = [pick(&s, 0, 3, 0.5), pick(&s, 2, 5, 0.5)];
    let out = s
        .try_insert_edge_path(&picks, None, Tolerance::default())
        .unwrap();
    assert_eq!(out.faces().len(), 4);
    assert_eq!(out.control_points().len(), 9);
    let points = s
        .control_points()
        .iter()
        .map(|p| Point3::try_new(1e6 + p.y(), -2e6 + p.z(), 3e6 + p.x()).unwrap())
        .collect();
    let rotated = SubD::try_from_polygons(points, s.faces().to_vec()).unwrap();
    let normal = Vector3::try_new(0., 1., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let moved = rotated
        .try_insert_edge_path(
            &[pick(&rotated, 0, 3, 0.5), pick(&rotated, 2, 5, 0.5)],
            Some(normal),
            Tolerance::default(),
        )
        .unwrap();
    assert_eq!(moved.faces(), out.faces());
    for (a, b) in out.control_points().iter().zip(moved.control_points()) {
        assert!(
            Point3::try_new(1e6 + a.y(), -2e6 + a.z(), 3e6 + a.x())
                .unwrap()
                .distance_to(*b)
                .unwrap()
                <= 1e-9
        );
    }
}
#[test]
fn a_path_through_an_existing_vertex_connects_all_incident_faces_without_duplicates() {
    let s = SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [0., 1., 0.],
            [1., 1., 0.],
            [2., 1., 0.],
            [0., 2., 0.],
            [1., 2., 0.],
            [2., 2., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![4, 5, 8, 7],
            vec![3, 4, 7, 6],
        ],
    )
    .unwrap();
    let out = s
        .try_insert_edge_path(
            &[pick(&s, 0, 3, 0.5), pick(&s, 5, 8, 0.5)],
            None,
            Tolerance::default(),
        )
        .unwrap();
    assert_eq!(out.control_points().len(), 11);
    assert_eq!(out.faces().len(), 6);
    assert_eq!(
        out.edges()
            .iter()
            .filter(|e| e.contains(&4) && e.iter().any(|&v| v >= 9))
            .count(),
        2
    );
    SubD::try_from_archive(out.archive()).unwrap();
}
#[test]
fn repeated_points_noops_and_resource_limits_keep_valid_topology() {
    let s = source();
    let pick = pick(&s, 0, 3, 0.5);
    let tol = Tolerance::default();
    assert_eq!(
        s.try_insert_edge_path(&[], None, tol).unwrap().archive(),
        s.archive()
    );
    let one = s.try_insert_edge_path(&[pick], None, tol).unwrap();
    let repeated = s.try_insert_edge_path(&[pick, pick], None, tol).unwrap();
    assert_eq!(one.control_points(), repeated.control_points());
    assert_eq!(one.faces(), repeated.faces());
    assert!(s.try_insert_edge_path(&vec![pick; 257], None, tol).is_err());
    SubD::try_from_archive(repeated.archive()).unwrap();
}

#[test]
fn edge_splits_keep_existing_uvw_prefixes_and_leave_new_corners_unset() {
    let source = source();
    let texture = vec![[0., 0., 1.], [1., 0., 2.], [1., 1., 3.], [0., 1., 4.]];
    let source = source
        .try_with_face_texture_points(&[(0, Some(texture.clone()))])
        .unwrap();
    let original = source.archive().to_vec();
    let picks = [pick(&source, 0, 3, 0.25), pick(&source, 0, 3, 0.75)];
    let path = source
        .try_insert_edge_path(&picks, None, Tolerance::DEFAULT)
        .unwrap();
    let mut expected = texture.into_iter().map(Some).collect::<Vec<_>>();
    expected.extend([None, None]);
    assert_eq!(path.face_texture_points(), &[Some(expected.clone()), None]);
    assert_eq!(
        SubD::try_from_archive(path.archive())
            .unwrap()
            .face_texture_points(),
        &[Some(expected.clone()), None]
    );
    assert_eq!(source.archive(), original);
    // The single-edge binding must also retain an already incomplete prefix.
    let point = Point3::try_new(0., 1.5, 0.).unwrap();
    let first = source
        .try_insert_edge_vertex(picks[0].edge, point)
        .unwrap()
        .unwrap()
        .subd;
    assert_eq!(
        first.face_texture_points()[0].as_ref().unwrap().last(),
        Some(&None)
    );
    let next = pick(&first, 0, 6, 0.5);
    let point = first.control_points()[0]
        .midpoint(first.control_points()[6])
        .unwrap();
    let second = first
        .try_insert_edge_vertex(next.edge, point)
        .unwrap()
        .unwrap()
        .subd;
    assert_eq!(second.face_texture_points(), &[Some(expected), None]);
}
