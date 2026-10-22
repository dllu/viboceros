//! primitives mesh axial command regressions.
use super::*;

#[test]
fn mesh_cylinder_matches_default_and_configurable_rhino_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed = parse_mesh_cylinder_options(&["0,0,0", "2", "5"]).unwrap();
    assert_eq!(parsed.vertical_count, 10);
    assert_eq!(parsed.around_count, 10);
    assert!(parsed.solid);
    assert_eq!(parsed.cap_style, MeshCapFaceStyle::Triangles);

    let message = registry
        .execute(&mut document, "MeshCylinder 0,0,0 2 5")
        .unwrap();
    assert!(message.ends_with("(10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCylinder must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 132);
    assert_eq!(default.face_count(), 120);
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Quad([0, 1, 11, 10])
    );
    assert_eq!(
        default.faces()[100],
        viboceros_geometry::MeshFace::Triangle([110, 112, 111])
    );
    assert_eq!(
        default.faces()[110],
        viboceros_geometry::MeshFace::Triangle([121, 122, 123])
    );
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshCylinder"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshCylinder 1,2,3 2 -5 VerticalFaces=2 AroundFaces=6 CapFaceStyle=Quad",
        )
        .unwrap();
    let Geometry::Mesh(quads) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCylinder quad-cap form must create a mesh")
    };
    assert_eq!(quads.vertices().len(), 32);
    assert_eq!(quads.face_count(), 18);
    assert_eq!(
        quads.faces()[12],
        viboceros_geometry::MeshFace::Quad([18, 21, 20, 19])
    );
    assert_eq!(
        quads.faces()[15],
        viboceros_geometry::MeshFace::Quad([25, 26, 27, 28])
    );
    assert_eq!(quads.bounds().min().z(), -2.0);
    assert_eq!(quads.bounds().max().z(), 3.0);
    assert!(quads.topology().is_solid());

    registry.execute(&mut document, "Undo").unwrap();
    registry
            .execute(
                &mut document,
                "MeshCylinder 0,0,0 0,2,0 4 Axis=1,0,0 BothSides=Yes Solid=No VerticalFaces=2 AroundFaces=4",
            )
            .unwrap();
    let Geometry::Mesh(open) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCylinder open form must create a mesh")
    };
    assert_eq!(open.vertices().len(), 12);
    assert_eq!(open.face_count(), 8);
    assert_eq!(open.bounds().min().x(), -4.0);
    assert_eq!(open.bounds().max().x(), 4.0);
    assert!(!open.topology().is_closed());
}

#[test]
fn mesh_cylinder_rejects_invalid_options_and_dimensions_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshCylinder",
        "MeshCylinder 0,0,0 0 5",
        "MeshCylinder 0,0,0 2 0",
        "MeshCylinder 0,0,0 2 5 VerticalFaces=0",
        "MeshCylinder 0,0,0 2 5 AroundFaces=2",
        "MeshCylinder 0,0,0 2 5 AroundFaces=nope",
        "MeshCylinder 0,0,0 2 5 AroundFaces=6 AroundFaces=8",
        "MeshCylinder 0,0,0 2 5 CapFaceStyle=Pentagon",
        "MeshCylinder 0,0,0 2 5 Solid=Maybe",
        "MeshCylinder 0,0,0 2 5 Axis=0,0,0",
        "MeshCylinder 0,0,0 0,0,2 5 Axis=0,0,1",
        "MeshCylinder 0,0,0 2 5 VerticalFaces=1000001 AroundFaces=3 Solid=No",
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
fn mesh_cone_matches_default_and_configurable_rhino_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed = parse_mesh_cone_options(&["0,0,0", "2", "5"]).unwrap();
    assert_eq!(parsed.vertical_count, 10);
    assert_eq!(parsed.around_count, 10);
    assert!(parsed.solid);
    assert_eq!(parsed.cap_style, MeshCapFaceStyle::Triangles);

    let message = registry
        .execute(&mut document, "MeshCone 0,0,0 2 5")
        .unwrap();
    assert!(message.ends_with("(10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCone must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 112);
    assert_eq!(default.face_count(), 110);
    assert_eq!(
        default.vertices()[0],
        Point3::try_new(0.0, 0.0, 5.0).unwrap()
    );
    assert!(default.vertices()[2].y() < 0.0);
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Triangle([0, 2, 1])
    );
    assert_eq!(
        default.faces()[10],
        viboceros_geometry::MeshFace::Quad([1, 2, 12, 11])
    );
    assert_eq!(
        default.faces()[100],
        viboceros_geometry::MeshFace::Triangle([101, 102, 103])
    );
    assert_eq!(default.bounds().min().z(), 0.0);
    assert_eq!(default.bounds().max().z(), 5.0);
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshCone"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshCone 1,2,3 2 -5 VerticalFaces=2 AroundFaces=6 CapFaceStyle=Quad",
        )
        .unwrap();
    let Geometry::Mesh(quads) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCone quad-cap form must create a mesh")
    };
    assert_eq!(quads.vertices().len(), 20);
    assert_eq!(quads.face_count(), 15);
    assert_eq!(
        quads.vertices()[0],
        Point3::try_new(1.0, 2.0, -2.0).unwrap()
    );
    assert!(quads.vertices()[2].y() > 2.0);
    assert_eq!(
        quads.faces()[6],
        viboceros_geometry::MeshFace::Quad([1, 2, 8, 7])
    );
    assert_eq!(
        quads.faces()[12],
        viboceros_geometry::MeshFace::Quad([13, 14, 15, 16])
    );
    assert_eq!(quads.bounds().min().z(), -2.0);
    assert_eq!(quads.bounds().max().z(), 3.0);
    assert!(quads.topology().is_solid());
    assert!(quads.signed_volume().unwrap() > 0.0);

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshCone 0,0,0 0,2,0 4 Axis=1,0,0 Solid=No VerticalFaces=2 AroundFaces=4",
        )
        .unwrap();
    let Geometry::Mesh(open) = document.objects().next().unwrap().geometry() else {
        panic!("MeshCone open form must create a mesh")
    };
    assert_eq!(open.vertices().len(), 9);
    assert_eq!(open.face_count(), 8);
    assert_eq!(open.bounds().min().x(), 0.0);
    assert_eq!(open.bounds().max().x(), 4.0);
    assert!(!open.topology().is_closed());
}

#[test]
fn mesh_cone_rejects_invalid_options_and_dimensions_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshCone",
        "MeshCone 0,0,0 0 5",
        "MeshCone 0,0,0 2 0",
        "MeshCone 0,0,0 2 5 VerticalFaces=0",
        "MeshCone 0,0,0 2 5 AroundFaces=2",
        "MeshCone 0,0,0 2 5 AroundFaces=nope",
        "MeshCone 0,0,0 2 5 AroundFaces=6 AroundFaces=8",
        "MeshCone 0,0,0 2 5 CapFaceStyle=Pentagon",
        "MeshCone 0,0,0 2 5 Solid=Maybe",
        "MeshCone 0,0,0 2 5 BothSides=Yes",
        "MeshCone 0,0,0 2 5 Axis=0,0,0",
        "MeshCone 0,0,0 0,0,2 5 Axis=0,0,1",
        "MeshCone 0,0,0 2 5 VerticalFaces=1000001 AroundFaces=3 Solid=No",
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
fn mesh_truncated_cone_matches_default_and_configurable_rhino_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let parsed = parse_mesh_truncated_cone_options(&["0,0,0", "3", "5", "1"]).unwrap();
    assert_eq!(parsed.vertical_count, 10);
    assert_eq!(parsed.around_count, 10);
    assert!(parsed.solid);
    assert_eq!(parsed.cap_style, MeshCapFaceStyle::Triangles);

    let message = registry
        .execute(&mut document, "MeshTruncatedCone 0,0,0 3 5 1")
        .unwrap();
    assert!(message.ends_with("(10 around × 10 vertical faces)"));
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(default) = document.objects().next().unwrap().geometry() else {
        panic!("MeshTruncatedCone must create a polygon mesh")
    };
    assert_eq!(default.vertices().len(), 132);
    assert_eq!(default.face_count(), 120);
    assert_eq!(
        default.vertices()[0],
        Point3::try_new(3.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        default.vertices()[100],
        Point3::try_new(1.0, 0.0, 5.0).unwrap()
    );
    assert_eq!(
        default.faces()[0],
        viboceros_geometry::MeshFace::Quad([0, 1, 11, 10])
    );
    assert_eq!(
        default.faces()[100],
        viboceros_geometry::MeshFace::Triangle([110, 112, 111])
    );
    assert_eq!(
        default.faces()[110],
        viboceros_geometry::MeshFace::Triangle([121, 122, 123])
    );
    assert!(default.topology().is_solid());
    assert!(default.signed_volume().unwrap() > 0.0);
    assert_eq!(document.undo_label(), Some("MeshTruncatedCone"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshTruncatedCone 1,2,3 2 -5 4 VerticalFaces=2 AroundFaces=6 CapFaceStyle=Quad",
        )
        .unwrap();
    let Geometry::Mesh(quads) = document.objects().next().unwrap().geometry() else {
        panic!("MeshTruncatedCone quad-cap form must create a mesh")
    };
    assert_eq!(quads.vertices().len(), 32);
    assert_eq!(quads.face_count(), 18);
    assert_eq!(quads.vertices()[0], Point3::try_new(3.0, 2.0, 3.0).unwrap());
    assert!(quads.vertices()[1].y() < 2.0);
    assert_eq!(
        quads.vertices()[12],
        Point3::try_new(5.0, 2.0, -2.0).unwrap()
    );
    assert_eq!(
        quads.faces()[12],
        viboceros_geometry::MeshFace::Quad([18, 21, 20, 19])
    );
    assert_eq!(
        quads.faces()[15],
        viboceros_geometry::MeshFace::Quad([25, 26, 27, 28])
    );
    assert_eq!(quads.bounds().min().z(), -2.0);
    assert_eq!(quads.bounds().max().z(), 3.0);
    assert!(quads.topology().is_solid());
    assert!(quads.signed_volume().unwrap() > 0.0);

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshTruncatedCone 0,0,0 0,2,0 4 1 Axis=1,0,0 Solid=No VerticalFaces=2 AroundFaces=4",
        )
        .unwrap();
    let Geometry::Mesh(open) = document.objects().next().unwrap().geometry() else {
        panic!("open MeshTruncatedCone must create a mesh")
    };
    assert_eq!(open.vertices().len(), 12);
    assert_eq!(open.face_count(), 8);
    assert_eq!(open.bounds().min().x(), 0.0);
    assert_eq!(open.bounds().max().x(), 4.0);
    assert!(!open.topology().is_closed());
}

#[test]
fn mesh_truncated_cone_rejects_invalid_options_and_dimensions_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshTruncatedCone",
        "MeshTruncatedCone 0,0,0 0 5 1",
        "MeshTruncatedCone 0,0,0 3 0 1",
        "MeshTruncatedCone 0,0,0 3 5 0",
        "MeshTruncatedCone 0,0,0 3 5 -1",
        "MeshTruncatedCone 0,0,0 3 5 1 VerticalFaces=0",
        "MeshTruncatedCone 0,0,0 3 5 1 AroundFaces=2",
        "MeshTruncatedCone 0,0,0 3 5 1 AroundFaces=nope",
        "MeshTruncatedCone 0,0,0 3 5 1 AroundFaces=6 AroundFaces=8",
        "MeshTruncatedCone 0,0,0 3 5 1 CapFaceStyle=Pentagon",
        "MeshTruncatedCone 0,0,0 3 5 1 Solid=Maybe",
        "MeshTruncatedCone 0,0,0 3 5 1 BothSides=Yes",
        "MeshTruncatedCone 0,0,0 3 5 1 Axis=0,0,0",
        "MeshTruncatedCone 0,0,0 0,0,2 5 1 Axis=0,0,1",
        "MeshTruncatedCone 0,0,0 3 5 1 VerticalFaces=1000001 AroundFaces=3 Solid=No",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0, "{command}");
        assert_eq!(document.undo_label(), None, "{command}");
    }
}
