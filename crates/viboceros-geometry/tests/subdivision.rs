use viboceros_geometry::{Point3, SubD};
fn strip() -> SubD {
    SubD::try_from_polygons(
        [
            [0., 0., 0.],
            [2., 0., 0.],
            [4., 0., 1.],
            [0., 3., 0.],
            [2., 3., 0.],
            [4., 3., 1.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
}
#[test]
fn global_and_local_subdivision_preserve_sources_and_archive_topology() {
    let source = strip();
    let archive = source.archive().to_vec();
    let (local, selected) = source.subdivided_faces_with_selection(&[0, 0, 0]).unwrap();
    assert_eq!(selected.len(), 4);
    assert!(selected.iter().all(|&i| local.faces()[i].len() == 4));
    assert_eq!(
        (
            local.control_points().len(),
            local.edges().len(),
            local.faces().len()
        ),
        (11, 15, 5)
    );
    assert_eq!(local.faces().iter().filter(|f| f.len() == 5).count(), 1);
    assert_eq!(
        local.control_points()[0],
        Point3::try_new(0.25, 0.375, 0.).unwrap()
    );
    assert_eq!(local.control_points()[1], source.control_points()[1]);
    assert!(local.has_boundary());
    let all = source.subdivided_faces(&[1, 0]).unwrap();
    let global = source.subdivided(1).unwrap();
    assert_eq!(all.control_points(), global.control_points());
    assert_eq!(all.faces(), global.faces());
    let twice = global.subdivided(1).unwrap();
    let direct = source.subdivided(2).unwrap();
    assert_eq!(twice.control_points(), direct.control_points());
    assert_eq!(twice.faces(), direct.faces());
    for value in [local, global, twice] {
        let read = SubD::try_from_archive(value.archive()).unwrap();
        assert_eq!(read.control_points(), value.control_points());
        assert_eq!(read.faces(), value.faces());
        assert_eq!(read.vertex_tags(), value.vertex_tags());
        assert_eq!(read.edge_tags(), value.edge_tags());
    }
    assert_eq!(source.archive(), archive);
}
#[test]
fn display_lineage_addresses_the_active_net_after_global_local_and_archive_edits() {
    let source = strip();
    let global = source.subdivided(2).unwrap();
    let local = global.subdivided_faces(&[1, 3]).unwrap();
    for source in [source, global, local] {
        for source in [
            source.clone(),
            SubD::try_from_archive(source.archive()).unwrap(),
        ] {
            for density in [1, 2] {
                let display = source.tessellate_with_normals(density).unwrap();
                assert_eq!(display.face_sources.len(), display.mesh.face_count());
                let mut counts = vec![0; source.faces().len()];
                for face in display.face_sources {
                    assert!(face < counts.len());
                    counts[face] += 1;
                }
                for (face, count) in source.faces().iter().zip(counts) {
                    assert_eq!(count, face.len() * 4usize.pow(density - 1));
                }
            }
        }
    }
}
#[test]
fn invalid_and_excessive_refinement_is_rejected_without_modifying_the_archive() {
    let source = strip();
    let original = source.clone();
    for faces in [vec![], vec![2], vec![0, usize::MAX]] {
        assert!(source.subdivided_faces(&faces).is_err());
    }
    for count in [0, 8, 9, u32::MAX] {
        assert!(source.subdivided(count).is_err());
    }
    assert_eq!(source, original);
}
