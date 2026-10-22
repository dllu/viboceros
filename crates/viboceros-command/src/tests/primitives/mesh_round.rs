//! primitives mesh round command regressions.
use super::*;

#[test]
fn mesh_sphere_matches_all_rhino_styles_and_orientations() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed = parse_mesh_sphere_options(&["0,0,0", "2"]).unwrap();
    assert_eq!(
        parsed.topology,
        MeshSphereCommandTopology::Uv {
            vertical_count: 10,
            around_count: 10,
        }
    );
    assert_eq!(
        parse_mesh_sphere_options(&["0,0,0", "2", "Style=Quads"])
            .unwrap()
            .topology,
        MeshSphereCommandTopology::Quads { subdivisions: 3 }
    );

    let message = registry
        .execute(&mut document, "MeshSphere 0,0,0 2")
        .unwrap();
    assert!(message.ends_with("(10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshSphere must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 92);
    assert_eq!(default.face_count(), 100);
    assert_eq!(
        default.vertices()[0],
        Point3::try_new(0.0, 0.0, -2.0).unwrap()
    );
    assert_eq!(
        default.vertices()[91],
        Point3::try_new(0.0, 0.0, 2.0).unwrap()
    );
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Triangle([0, 2, 1])
    );
    assert_eq!(
        default.faces()[10],
        viboceros_geometry::MeshFace::Quad([1, 2, 12, 11])
    );
    assert_eq!(
        default.faces()[90],
        viboceros_geometry::MeshFace::Triangle([81, 82, 91])
    );
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshSphere"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshSphere 1,2,3 9,5,3 Axis=1,0,0 Style=UV VerticalFaces=4 AroundFaces=6",
        )
        .unwrap();
    let Geometry::Mesh(oriented) = document.objects().next().unwrap().geometry() else {
        panic!("oriented MeshSphere must create a mesh")
    };
    assert_eq!(oriented.vertices().len(), 20);
    assert_eq!(oriented.face_count(), 24);
    assert_eq!(
        oriented.vertices()[0],
        Point3::try_new(-2.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.vertices()[19],
        Point3::try_new(4.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.faces()[6],
        viboceros_geometry::MeshFace::Quad([1, 2, 8, 7])
    );
    assert!(oriented.topology().is_solid());

    registry.execute(&mut document, "Undo").unwrap();
    let message = registry
        .execute(
            &mut document,
            "MeshSphere 0,0,0 2 Style=Quad Subdivisions=2",
        )
        .unwrap();
    assert!(message.contains("Added quad mesh sphere"));
    assert!(message.ends_with("(2 subdivisions, 96 faces)"));
    let Geometry::Mesh(quad) = document.objects().next().unwrap().geometry() else {
        panic!("quad MeshSphere must create a mesh")
    };
    assert_eq!(quad.vertices().len(), 98);
    assert_eq!(quad.face_count(), 96);
    assert_eq!(
        quad.faces()[0],
        viboceros_geometry::MeshFace::Quad([48, 79, 27, 0])
    );
    assert!((quad.vertices()[0].x() + 0.731_390_176_158_283_5).abs() < 1.0e-15);
    assert!((quad.vertices()[0].y() - 0.731_390_176_158_283_5).abs() < 1.0e-15);
    assert!((quad.vertices()[0].z() + 1.711_764_242_072_578_5).abs() < 1.0e-15);
    assert!(quad.topology().is_solid());
    assert!(quad.signed_volume().unwrap() > 0.0);

    registry.execute(&mut document, "Undo").unwrap();
    let message = registry
        .execute(
            &mut document,
            "MeshSphere 0,0,0 2 Style=Triangle Subdivisions=1",
        )
        .unwrap();
    assert!(message.contains("Added triangle mesh sphere"));
    assert!(message.ends_with("(1 subdivisions, 80 faces)"));
    let Geometry::Mesh(triangle) = document.objects().next().unwrap().geometry() else {
        panic!("triangle MeshSphere must create a mesh")
    };
    assert_eq!(triangle.vertices().len(), 42);
    assert_eq!(triangle.face_count(), 80);
    assert_eq!(
        triangle.faces()[0],
        viboceros_geometry::MeshFace::Triangle([0, 12, 14])
    );
    assert!(triangle.topology().is_solid());
    assert!(triangle.signed_volume().unwrap() > 0.0);
}

#[test]
fn mesh_sphere_rejects_incompatible_styles_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshSphere",
        "MeshSphere 0,0,0 0",
        "MeshSphere 0,0,0 -2",
        "MeshSphere 0,0,0 2 VerticalFaces=1",
        "MeshSphere 0,0,0 2 AroundFaces=2",
        "MeshSphere 0,0,0 2 AroundFaces=nope",
        "MeshSphere 0,0,0 2 AroundFaces=6 AroundFaces=8",
        "MeshSphere 0,0,0 2 Style=Hexagons",
        "MeshSphere 0,0,0 2 Subdivisions=1",
        "MeshSphere 0,0,0 2 Style=UV Subdivisions=1",
        "MeshSphere 0,0,0 2 Style=Quads VerticalFaces=4",
        "MeshSphere 0,0,0 2 Style=Triangles AroundFaces=6",
        "MeshSphere 0,0,0 2 Style=Quads Subdivisions=7",
        "MeshSphere 0,0,0 2 Style=Triangles Subdivisions=6",
        "MeshSphere 0,0,0 2 Style=Triangles Subdivisions=nope",
        "MeshSphere 0,0,0 2 Style=Quads Subdivisions=1 Subdivisions=2",
        "MeshSphere 0,0,0 2 Style=UV Style=UV",
        "MeshSphere 0,0,0 2 Solid=Yes",
        "MeshSphere 0,0,0 2 Axis=0,0,0",
        "MeshSphere 0,0,0 0,0,2 Axis=0,0,1",
        "MeshSphere 0,0,0 2 VerticalFaces=1000001 AroundFaces=3",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0, "{command}");
        assert_eq!(document.undo_label(), None, "{command}");
    }
}

#[test]
fn mesh_ellipsoid_matches_rhino_parameterization_caps_and_orientation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed =
        parse_mesh_ellipsoid_options(&["1,2,3", "4", "3", "2"], document.tolerance()).unwrap();
    assert_eq!(parsed.radii, [4.0, 3.0, 2.0]);
    assert_eq!(parsed.vertical_count, 10);
    assert_eq!(parsed.around_count, 10);
    assert_eq!(parsed.cap_style, MeshCapFaceStyle::Triangles);

    let message = registry
        .execute(&mut document, "MeshEllipsoid 1,2,3 4 3 2")
        .unwrap();
    assert!(message.ends_with("(10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshEllipsoid must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 92);
    assert_eq!(default.face_count(), 100);
    assert_eq!(
        default.vertices()[0],
        Point3::try_new(-3.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        default.vertices()[91],
        Point3::try_new(5.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Triangle([0, 2, 1])
    );
    assert_eq!(
        default.faces()[10],
        viboceros_geometry::MeshFace::Quad([1, 2, 12, 11])
    );
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshEllipsoid"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshEllipsoid 1,2,3 1,6,3 1,4,6 3,4,5 VerticalFaces=4 AroundFaces=6 CapFaceStyle=Quad",
        )
        .unwrap();
    let Geometry::Mesh(oriented) = document.objects().next().unwrap().geometry() else {
        panic!("oriented MeshEllipsoid must create a mesh")
    };
    assert_eq!(oriented.vertices().len(), 20);
    assert_eq!(oriented.face_count(), 18);
    assert_eq!(
        oriented.vertices()[0],
        Point3::try_new(1.0, -2.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.vertices()[19],
        Point3::try_new(1.0, 6.0, 3.0).unwrap()
    );
    assert!((oriented.vertices()[2].x() - 2.233_562_514_616_302_5).abs() < 1.0e-14);
    assert!((oriented.vertices()[2].y() + 0.828_427_124_746_190_3).abs() < 1.0e-14);
    assert!((oriented.vertices()[2].z() - 4.037_414_057_018_887).abs() < 1.0e-14);
    assert_eq!(
        oriented.faces()[0],
        viboceros_geometry::MeshFace::Quad([0, 3, 2, 1])
    );
    assert_eq!(
        oriented.faces()[15],
        viboceros_geometry::MeshFace::Quad([13, 14, 15, 19])
    );
    assert!(oriented.topology().is_solid());

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshEllipsoid 0,0,0 4 3 2 VerticalFaces=3 AroundFaces=5 CapFaceStyle=Quad",
        )
        .unwrap();
    let Geometry::Mesh(odd) = document.objects().next().unwrap().geometry() else {
        panic!("odd MeshEllipsoid must create a mesh")
    };
    assert_eq!(odd.face_count(), 15);
    assert!(matches!(
        odd.faces()[0],
        viboceros_geometry::MeshFace::Triangle(_)
    ));
}

#[test]
fn mesh_ellipsoid_rejects_invalid_options_and_dimensions_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshEllipsoid",
        "MeshEllipsoid 0,0,0 0 2 3",
        "MeshEllipsoid 0,0,0 1 -2 3",
        "MeshEllipsoid 0,0,0 1 2 inf",
        "MeshEllipsoid 0,0,0 1,0,0 2,0,0 0,0,3",
        "MeshEllipsoid 0,0,0 1,0,0 0,2,0 0,0,0",
        "MeshEllipsoid 0,0,0 1,0,0 0,2,0 1,1,0",
        "MeshEllipsoid 0,0,0 1 2 3 VerticalFaces=1",
        "MeshEllipsoid 0,0,0 1 2 3 AroundFaces=2",
        "MeshEllipsoid 0,0,0 1 2 3 AroundFaces=nope",
        "MeshEllipsoid 0,0,0 1 2 3 AroundFaces=6 AroundFaces=8",
        "MeshEllipsoid 0,0,0 1 2 3 CapFaceStyle=Pentagon",
        "MeshEllipsoid 0,0,0 1 2 3 Solid=Yes",
        "MeshEllipsoid 0,0,0 1 2 3 VerticalFaces=1000001 AroundFaces=3",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0, "{command}");
        assert_eq!(document.undo_label(), None, "{command}");
    }
}

#[test]
fn mesh_torus_matches_default_and_oriented_rhino_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed = parse_mesh_torus_options(&["0,0,0", "4", "1"]).unwrap();
    assert_eq!(parsed.vertical_count, 10);
    assert_eq!(parsed.around_count, 10);

    let message = registry
        .execute(&mut document, "MeshTorus 0,0,0 4 1")
        .unwrap();
    assert!(message.ends_with("10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshTorus must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 100);
    assert_eq!(default.face_count(), 100);
    assert_eq!(
        default.vertices()[0],
        Point3::try_new(5.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Quad([0, 1, 11, 10])
    );
    assert_eq!(
        default.faces()[90],
        viboceros_geometry::MeshFace::Quad([90, 91, 1, 0])
    );
    assert_eq!(
        default.faces()[99],
        viboceros_geometry::MeshFace::Quad([99, 90, 0, 9])
    );
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshTorus"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshTorus 1,2,3 1,6,3 1 Axis=1,0,0 VerticalFaces=4 AroundFaces=6",
        )
        .unwrap();
    let Geometry::Mesh(oriented) = document.objects().next().unwrap().geometry() else {
        panic!("oriented MeshTorus must create a mesh")
    };
    assert_eq!(oriented.vertices().len(), 24);
    assert_eq!(oriented.face_count(), 24);
    assert_eq!(
        oriented.vertices()[0],
        Point3::try_new(1.0, 7.0, 3.0).unwrap()
    );
    assert!((oriented.vertices()[6].x() - 2.0).abs() < 1.0e-12);
    assert!((oriented.vertices()[6].y() - 6.0).abs() < 1.0e-12);
    assert!((oriented.vertices()[6].z() - 3.0).abs() < 1.0e-12);
    assert_eq!(
        oriented.faces()[18],
        viboceros_geometry::MeshFace::Quad([18, 19, 1, 0])
    );
    assert!(oriented.topology().is_solid());
}

#[test]
fn mesh_torus_rejects_invalid_options_and_radii_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshTorus",
        "MeshTorus 0,0,0 0 1",
        "MeshTorus 0,0,0 4 0",
        "MeshTorus 0,0,0 4 4",
        "MeshTorus 0,0,0 1 2",
        "MeshTorus 0,0,0 4 1 VerticalFaces=2",
        "MeshTorus 0,0,0 4 1 AroundFaces=2",
        "MeshTorus 0,0,0 4 1 AroundFaces=nope",
        "MeshTorus 0,0,0 4 1 AroundFaces=6 AroundFaces=8",
        "MeshTorus 0,0,0 4 1 Solid=Yes",
        "MeshTorus 0,0,0 4 1 Axis=0,0,0",
        "MeshTorus 0,0,0 0,0,4 1 Axis=0,0,1",
        "MeshTorus 0,0,0 4 1 VerticalFaces=1000001 AroundFaces=3",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0, "{command}");
        assert_eq!(document.undo_label(), None, "{command}");
    }
}
