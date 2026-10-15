use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn rectangle() -> TriangleMesh {
    TriangleMesh::try_new(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(4., 3., 0.), p(0., 3., 0.)],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn triangles_merge_as_a_quad_and_second_pass_is_unchanged() {
    let source = rectangle();
    let before = source.clone();
    let (result, removed) = source.merge_coplanar_faces(Tolerance::DEFAULT).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(result.face_count(), 1);
    assert!(matches!(result.faces()[0], MeshFace::Quad(_)));
    assert!((result.area().unwrap() - 12.).abs() < 1e-12);
    assert_eq!(source, before);
    assert_eq!(
        result.merge_coplanar_faces(Tolerance::DEFAULT).unwrap(),
        (result.clone(), 0)
    );
}
#[test]
fn annulus_retains_hole_wires_and_mesh_to_nurbs_hole() {
    let vertices = vec![
        p(0., 0., 0.),
        p(3., 0., 0.),
        p(3., 3., 0.),
        p(0., 3., 0.),
        p(1., 1., 0.),
        p(2., 1., 0.),
        p(2., 2., 0.),
        p(1., 2., 0.),
    ];
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (result, removed) = mesh.merge_coplanar_faces(Tolerance::DEFAULT).unwrap();
    assert_eq!(removed, 7);
    assert_eq!(result.ngons.len(), 1);
    assert_eq!(result.ngons[0].faces.len(), 8);
    assert_eq!(
        result
            .visible_wireframe_lines(Tolerance::DEFAULT)
            .unwrap()
            .len(),
        8
    );
    assert!((result.area().unwrap() - 8.).abs() < 1e-12);
    let b = crate::Brep::try_from_mesh_with_ngons(&result, true, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(b.faces().len(), 1);
    assert_eq!(b.faces()[0].loops().len(), 2);
    assert!((b.area(Tolerance::DEFAULT).unwrap() - 8.).abs() < 1e-9);
}

#[test]
fn concave_triangle_pairs_retain_area_for_every_face_rotation() {
    let points = vec![p(0., 0., 0.), p(2., 0., 0.), p(0.5, 0.5, 0.), p(0., 2., 0.)];
    for r in 0..3 {
        for s in 0..3 {
            let mut a = [0, 1, 2];
            a.rotate_left(r);
            let mut b = [0, 2, 3];
            b.rotate_left(s);
            let mesh =
                TriangleMesh::try_new(points.clone(), vec![a, b], Tolerance::DEFAULT).unwrap();
            let (result, count) = mesh.merge_coplanar_faces(Tolerance::DEFAULT).unwrap();
            assert_eq!(count, 1);
            assert!(
                (result.area().unwrap() - mesh.area().unwrap()).abs() < 1e-12,
                "rotation {r},{s}: {:?}",
                result.faces()
            );
        }
    }
}

#[test]
fn grid_keeps_interior_vertices_and_colors_and_all_boundary_wires() {
    let t = Tolerance::DEFAULT;
    let frame = Frame3::try_from_normal(
        p(0., 0., 0.),
        crate::Vector3::try_new(0., 0., 1.).unwrap(),
        t,
    )
    .unwrap();
    let mesh = TriangleMesh::try_plane_grid(frame, [0., 2.], [0., 2.], 2, 2, t)
        .unwrap()
        .try_with_vertex_colors(Some((0..9).map(|i| [i, 0, 0, 0]).collect()))
        .unwrap();
    let (result, n) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(n, 3);
    assert_eq!(result.face_count(), 4);
    assert_eq!(result.vertices(), mesh.vertices());
    assert_eq!(result.vertex_colors(), mesh.vertex_colors());
    assert_eq!(result.ngons.len(), 1);
    assert_eq!(result.visible_wireframe_lines(t).unwrap().len(), 8);
}
#[test]
fn cube_retains_closed_boundary_volume_and_six_regions() {
    let t = Tolerance::DEFAULT;
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(2., 3., 0.),
            p(0., 3., 0.),
            p(0., 0., 4.),
            p(2., 0., 4.),
            p(2., 3., 4.),
            p(0., 3., 4.),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
        t,
    )
    .unwrap();
    let (result, n) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(n, 6);
    assert_eq!(result.face_count(), 6);
    assert_eq!(result.ngons.len(), 6);
    assert!((result.area().unwrap() - 52.).abs() < 1e-12);
    assert!((result.signed_volume().unwrap() - 24.).abs() < 1e-12);
    assert_eq!(result.visible_wireframe_lines(t).unwrap().len(), 12);
}
#[test]
fn angular_cutoff_and_unwelded_seams_use_native_topology() {
    let t = Tolerance::try_new(1e-3, 1e-12, 1e-8).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 1e-4)],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(mesh.merge_coplanar_faces(t).unwrap(), (mesh.clone(), 0));
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(4., 0., 0.),
            p(4., 3., 0.),
            p(0., 0., 0.),
            p(4., 3., 0.),
            p(0., 3., 0.),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (result, n) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(n, 1);
    assert_eq!(result.face_count(), 1);
    assert_eq!(result.vertices().len(), 4);
    assert!((result.area().unwrap() - 12.).abs() < 1e-12);
}
#[test]
fn neighboring_existing_ngons_merge_and_reversed_holes_remain_valid() {
    let t = Tolerance::DEFAULT;
    let vertices = vec![
        p(0., 0., 0.),
        p(1., 0., 0.),
        p(2., 0., 0.),
        p(0., 1., 0.),
        p(1., 1., 0.),
        p(2., 1., 0.),
    ];
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 1, 4], [0, 4, 3], [1, 2, 5], [1, 5, 4]],
        t,
    )
    .unwrap()
    .try_with_ngons(vec![
        MeshNgon::from_parts(vec![0, 1, 4, 3], vec![0, 1]),
        MeshNgon::from_parts(vec![1, 2, 5, 4], vec![2, 3]),
    ])
    .unwrap();
    let (result, n) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(n, 1);
    assert_eq!(result.ngons.len(), 1);
    assert_eq!(result.ngons[0].faces.len(), 4);
    assert!((result.area().unwrap() - 2.).abs() < 1e-12);
    let reversed = result.reversed();
    assert!(
        reversed
            .clone()
            .try_with_ngons(reversed.ngons.clone())
            .is_ok()
    );
}

#[test]
fn hole_boundary_starting_at_a_collinear_vertex_cannot_be_the_outer_loop() {
    let t = Tolerance::DEFAULT;
    let vertices = (0..5)
        .flat_map(|y| (0..5).map(move |x| p(x as f64, y as f64, 0.)))
        .collect();
    let mut faces = vec![[1, 7, 6], [1, 2, 7]];
    for y in 0..4 {
        for x in 0..4 {
            if (x == 1 || x == 2) && (y == 1 || y == 2) || (x == 1 && y == 0) {
                continue;
            }
            let a = (y * 5 + x) as u32;
            faces.push([a, a + 1, a + 6]);
            faces.push([a, a + 6, a + 5]);
        }
    }
    let mesh = TriangleMesh::try_new(vertices, faces, t).unwrap();
    let (result, n) = mesh.merge_coplanar_faces(t).unwrap();
    assert_eq!(n, 23);
    assert_eq!(result.ngons.len(), 1);
    assert_eq!(result.ngons[0].vertices.len(), 16);
    assert!(result.ngons[0].vertices.iter().all(|&i| {
        let p = result.vertices[i as usize];
        p.x() == 0. || p.x() == 4. || p.y() == 0. || p.y() == 4.
    }));
    let reversed = result.reversed();
    assert!(
        reversed
            .clone()
            .try_with_ngons(reversed.ngons.clone())
            .is_ok()
    );
}
