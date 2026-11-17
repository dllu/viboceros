use super::*;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn mesh_to_nurbs_matches_rhino_triangle_surfaces_and_parameter_domains() {
    let source = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();

    let untrimmed = Brep::try_from_mesh(&source, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(untrimmed.vertices().len(), 3);
    assert_eq!(untrimmed.edges().len(), 3);
    assert!(
        untrimmed
            .edges()
            .iter()
            .all(|edge| edge.curve().domain() == (0.0..=1.0))
    );
    let face = &untrimmed.faces()[0];
    assert_eq!(face.surface().degree_u(), 1);
    assert_eq!(face.surface().degree_v(), 1);
    assert_eq!(face.surface().domain_u(), 0.0..=4.0);
    assert_eq!(face.surface().domain_v(), 0.0..=5.0);
    assert_eq!(face.loops()[0].trims().len(), 4);
    assert_eq!(
        face.loops()[0]
            .trims()
            .iter()
            .map(BrepTrim::trim_type)
            .collect::<Vec<_>>(),
        vec![
            BrepTrimType::Boundary,
            BrepTrimType::Boundary,
            BrepTrimType::Boundary,
            BrepTrimType::Singular,
        ]
    );
    assert_eq!(face.loops()[0].trims()[3].iso(), SurfaceIso::West);
    assert_eq!(
        face.surface().evaluate(0.0, 0.0).unwrap(),
        point(0.0, 0.0, 0.0)
    );
    assert_eq!(
        face.surface().evaluate(4.0, 0.0).unwrap(),
        point(4.0, 0.0, 0.0)
    );
    assert_eq!(
        face.surface().evaluate(4.0, 5.0).unwrap(),
        point(0.0, 3.0, 0.0)
    );
    assert_eq!(
        face.surface().evaluate(0.0, 5.0).unwrap(),
        point(0.0, 0.0, 0.0)
    );

    let trimmed = Brep::try_from_mesh(&source, true, Tolerance::DEFAULT).unwrap();
    let face = &trimmed.faces()[0];
    assert_eq!(face.loops()[0].trims().len(), 3);
    assert_eq!(face.loops()[0].trims()[2].iso(), SurfaceIso::NotIso);
    assert_eq!(
        face.surface().evaluate(0.0, 5.0).unwrap(),
        point(-4.0, 3.0, 0.0)
    );
    for brep in [&untrimmed, &trimmed] {
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 6.0).abs() < 1.0e-12);
        let mesh = brep.tessellate(1, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.face_count(), 1);
        assert!(mesh.faces()[0].is_triangle());
    }

    let with_unused = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(9.0, 9.0, 9.0),
        ],
        vec![[2, 1, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let compacted = Brep::try_from_mesh(&with_unused, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        compacted
            .vertices()
            .iter()
            .map(|vertex| vertex.point())
            .collect::<Vec<_>>(),
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
        ]
    );
}

#[test]
fn mesh_to_nurbs_preserves_warped_quads_and_closed_shared_topology() {
    let warped = TriangleMesh::try_new_faces(
        vec![
            point(10.0, 0.0, 0.0),
            point(14.0, 0.0, 0.0),
            point(14.0, 3.0, 2.0),
            point(10.0, 3.0, 0.0),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let warped_brep = Brep::try_from_mesh(&warped, true, Tolerance::DEFAULT).unwrap();
    let surface = warped_brep.faces()[0].surface();
    assert_eq!(surface.domain_u(), 0.0..=20.0_f64.sqrt());
    assert_eq!(surface.domain_v(), 0.0..=13.0_f64.sqrt());
    assert_eq!(
        surface
            .evaluate(*surface.domain_u().end(), *surface.domain_v().end())
            .unwrap(),
        point(14.0, 3.0, 2.0)
    );

    let tetrahedron = TriangleMesh::try_new(
        vec![
            point(20.0, 0.0, 0.0),
            point(24.0, 0.0, 0.0),
            point(20.0, 4.0, 0.0),
            point(20.0, 0.0, 4.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for trim_triangles in [false, true] {
        let brep = Brep::try_from_mesh(&tetrahedron, trim_triangles, Tolerance::DEFAULT).unwrap();
        assert_eq!(brep.vertices().len(), 4);
        assert_eq!(brep.edges().len(), 6);
        assert_eq!(brep.faces().len(), 4);
        assert!(brep.is_solid());
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 64.0 / 6.0).abs() < 1.0e-10);
    }
}
