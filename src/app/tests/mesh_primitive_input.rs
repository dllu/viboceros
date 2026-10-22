use super::*;

#[test]
fn interactive_mesh_ellipsoid_retains_topology_and_validates_each_axis() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshEllipsoid VerticalFaces=1"));
    assert!(!app.try_start_interactive_command("MeshEllipsoid AroundFaces=2"));
    assert!(
        !app.try_start_interactive_command("MeshEllipsoid VerticalFaces=1000001 AroundFaces=3")
    );
    assert!(app.try_start_interactive_command(
        "MeshEllipsoid VerticalFaces=4 AroundFaces=6 CapFaceStyle=Quadrilaterals"
    ));
    let center = point(1.0, 2.0, 3.0);
    let first_axis = point(5.0, 2.0, 3.0);
    let second_axis = point(1.0, 5.0, 3.0);

    app.accept_drafting_point(center);
    app.accept_drafting_point(center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshEllipsoid {
            points: [Some(center), None, None],
            vertical_count: 4,
            around_count: 6,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(first_axis);
    let awaiting_second = app.active_command.unwrap();
    assert_eq!(awaiting_second.anchor(), Some(center));
    assert_eq!(awaiting_second.reference(), Some(first_axis));
    app.accept_drafting_point(point(3.0, 2.0, 3.0));
    assert_eq!(app.active_command, Some(awaiting_second));
    assert!(app.command_log.back().unwrap().contains("coordinate frame"));

    app.accept_drafting_point(second_axis);
    let awaiting_third = app.active_command.unwrap();
    assert_eq!(awaiting_third.anchor(), Some(center));
    assert_eq!(awaiting_third.reference(), Some(second_axis));
    app.accept_drafting_point(center);
    assert_eq!(app.active_command, Some(awaiting_third));
    assert_eq!(app.document.objects().len(), 0);
    app.accept_drafting_point(point(3.0, 4.0, 3.0));
    assert_eq!(app.active_command, Some(awaiting_third));
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(1.0, 2.0, 5.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh ellipsoid")
    };
    assert_eq!(mesh.vertices().len(), 20);
    assert_eq!(mesh.face_count(), 18);
    assert_eq!(mesh.vertices()[0], point(-3.0, 2.0, 3.0));
    assert_eq!(mesh.vertices()[19], point(5.0, 2.0, 3.0));
    assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 3, 2, 1]));
    assert_eq!(mesh.faces()[15], MeshFace::Quad([13, 14, 15, 19]));
    assert!(mesh.topology().is_solid());
    assert!(mesh.signed_volume().unwrap() > 0.0);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshEllipsoid"));
}

#[test]
fn interactive_rectangle_keeps_a_degenerate_second_pick_active() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Rect"));
    let first = point(-2.0, -1.0, 3.0);
    app.accept_drafting_point(first);
    app.accept_drafting_point(point(-2.0, 4.0, 3.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Rectangle { first: Some(first) })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(5.0, 4.0, 9.0));
    assert_eq!(app.active_command, None);
    let Geometry::Polyline(rectangle) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactive rectangle polyline")
    };
    assert!(rectangle.is_closed());
    assert_eq!(rectangle.segment_count(), 4);
    assert!(
        rectangle
            .vertices()
            .iter()
            .all(|vertex| vertex.z() == first.z())
    );
    assert_eq!(app.document.undo_label(), Some("Rectangle"));
}

#[test]
fn interactive_mesh_plane_retains_counts_and_normalizes_corner_order() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("MeshPlane XCount=2 YCount=3"));
    let first = point(4.0, 3.0, 2.0);
    app.accept_drafting_point(first);
    app.accept_drafting_point(point(4.0, 0.0, 8.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshPlane {
            first: Some(first),
            x_count: 2,
            y_count: 3,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(0.0, 0.0, 8.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh plane")
    };
    assert_eq!(mesh.face_count(), 6);
    assert_eq!(mesh.vertices().len(), 12);
    assert_eq!(mesh.vertices()[0], point(0.0, 0.0, 2.0));
    assert_eq!(mesh.vertices()[11], point(4.0, 3.0, 2.0));
    assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 1, 4, 3]));
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshPlane"));
}

#[test]
fn interactive_mesh_box_retains_counts_and_validates_all_three_picks() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshBox XCount=0"));
    assert!(!app.try_start_interactive_command("MeshBox XCount=500001 YCount=1 ZCount=1"));
    assert!(app.try_start_interactive_command("MeshBox XCount=2 YCount=3 ZCount=2"));
    let base = point(5.0, 8.0, 3.0);
    app.accept_drafting_point(base);
    app.accept_drafting_point(point(5.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshBox {
            base: Some(base),
            opposite: None,
            x_count: 2,
            y_count: 3,
            z_count: 2,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    let opposite = point(1.0, 2.0, 3.0);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    app.accept_drafting_point(point(9.0, 9.0, 3.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshBox {
            base: Some(base),
            opposite: Some(opposite),
            x_count: 2,
            y_count: 3,
            z_count: 2,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(9.0, 9.0, -1.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh box")
    };
    assert_eq!(mesh.vertices().len(), 66);
    assert_eq!(mesh.face_count(), 32);
    assert_eq!(mesh.vertices()[0], point(1.0, 8.0, 3.0));
    assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 3, 4, 1]));
    assert_eq!(mesh.bounds().min(), point(1.0, 2.0, -1.0));
    assert_eq!(mesh.bounds().max(), point(5.0, 8.0, 3.0));
    assert!(mesh.topology().is_solid());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshBox"));
}

#[test]
fn interactive_mesh_cone_retains_options_and_validates_three_picks() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshCone AroundFaces=2"));
    assert!(
        !app.try_start_interactive_command("MeshCone VerticalFaces=1000001 AroundFaces=3 Solid=No")
    );
    assert!(app.try_start_interactive_command(
        "MeshCone VerticalFaces=2 AroundFaces=6 Solid=Yes CapFaceStyle=Quadrilaterals"
    ));
    let center = point(1.0, 2.0, 1.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshCone {
            center: Some(center),
            radius_point: None,
            vertical_count: 2,
            around_count: 6,
            solid: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    let radius_point = point(3.0, 2.0, 1.0);
    app.accept_drafting_point(point(3.0, 2.0, 9.0));
    app.accept_drafting_point(point(9.0, 9.0, 1.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshCone {
            center: Some(center),
            radius_point: Some(radius_point),
            vertical_count: 2,
            around_count: 6,
            solid: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(9.0, 9.0, 4.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh cone")
    };
    assert_eq!(mesh.vertices().len(), 20);
    assert_eq!(mesh.face_count(), 15);
    assert_eq!(mesh.vertices()[0], point(1.0, 2.0, 4.0));
    assert_eq!(mesh.faces()[0], MeshFace::Triangle([0, 2, 1]));
    assert_eq!(mesh.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
    assert_eq!(mesh.faces()[12], MeshFace::Quad([13, 14, 15, 16]));
    assert_eq!(mesh.bounds().min().z(), 1.0);
    assert_eq!(mesh.bounds().max().z(), 4.0);
    assert!(mesh.topology().is_solid());
    assert!(mesh.signed_volume().unwrap() > 0.0);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshCone"));
}

#[test]
fn interactive_mesh_truncated_cone_retains_options_and_validates_four_picks() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshTruncatedCone AroundFaces=2"));
    assert!(!app.try_start_interactive_command(
        "MeshTruncatedCone VerticalFaces=1000001 AroundFaces=3 Solid=No"
    ));
    assert!(app.try_start_interactive_command(
        "MeshTruncatedCone VerticalFaces=2 AroundFaces=6 Solid=Yes CapFaceStyle=Quadrilaterals"
    ));
    let center = point(1.0, 2.0, 1.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshTruncatedCone {
            center: Some(center),
            base_radius_point: None,
            end_center: None,
            vertical_count: 2,
            around_count: 6,
            solid: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    let base_radius_point = point(3.0, 2.0, 1.0);
    app.accept_drafting_point(point(3.0, 2.0, 9.0));
    let awaiting_height = InteractiveCommand::MeshTruncatedCone {
        center: Some(center),
        base_radius_point: Some(base_radius_point),
        end_center: None,
        vertical_count: 2,
        around_count: 6,
        solid: true,
        cap_style: MeshCapFaceStyle::Quadrilaterals,
    };
    assert_eq!(app.active_command, Some(awaiting_height));
    assert_eq!(awaiting_height.anchor(), Some(center));
    app.accept_drafting_point(point(9.0, 9.0, 1.0));
    assert_eq!(app.active_command, Some(awaiting_height));
    assert_eq!(app.document.objects().len(), 0);

    let end_center = point(1.0, 2.0, 4.0);
    app.accept_drafting_point(point(9.0, 9.0, 4.0));
    let awaiting_end_radius = InteractiveCommand::MeshTruncatedCone {
        center: Some(center),
        base_radius_point: Some(base_radius_point),
        end_center: Some(end_center),
        vertical_count: 2,
        around_count: 6,
        solid: true,
        cap_style: MeshCapFaceStyle::Quadrilaterals,
    };
    assert_eq!(app.active_command, Some(awaiting_end_radius));
    assert_eq!(awaiting_end_radius.anchor(), Some(end_center));
    app.accept_drafting_point(point(1.0, 2.0, 20.0));
    assert_eq!(app.active_command, Some(awaiting_end_radius));
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(5.0, 2.0, 99.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh truncated cone")
    };
    assert_eq!(mesh.vertices().len(), 32);
    assert_eq!(mesh.face_count(), 18);
    assert_eq!(mesh.vertices()[0], point(3.0, 2.0, 1.0));
    assert_eq!(mesh.vertices()[6], point(4.0, 2.0, 2.5));
    assert_eq!(mesh.vertices()[12], point(5.0, 2.0, 4.0));
    assert_eq!(mesh.faces()[12], MeshFace::Quad([18, 21, 20, 19]));
    assert_eq!(mesh.faces()[15], MeshFace::Quad([25, 26, 27, 28]));
    assert_eq!(mesh.bounds().min().z(), 1.0);
    assert_eq!(mesh.bounds().max().z(), 4.0);
    assert!(mesh.topology().is_solid());
    assert!(mesh.signed_volume().unwrap() > 0.0);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshTruncatedCone"));
}

#[test]
fn interactive_mesh_sphere_retains_each_style_and_validates_radius_pick() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshSphere VerticalFaces=1"));
    assert!(!app.try_start_interactive_command("MeshSphere Style=UV Subdivisions=1"));
    assert!(!app.try_start_interactive_command("MeshSphere Style=Quads VerticalFaces=4"));
    assert!(!app.try_start_interactive_command("MeshSphere Style=Quads Subdivisions=7"));
    assert!(!app.try_start_interactive_command("MeshSphere Style=Triangles Subdivisions=6"));
    assert!(!app.try_start_interactive_command("MeshSphere VerticalFaces=1000001 AroundFaces=3"));
    assert!(app.try_start_interactive_command("MeshSphere Style=UV VerticalFaces=4 AroundFaces=6"));
    let center = point(1.0, 2.0, 1.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshSphere {
            center: Some(center),
            topology: InteractiveMeshSphereTopology::Uv {
                vertical_count: 4,
                around_count: 6,
            },
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(4.0, 2.0, 9.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created UV mesh sphere")
    };
    assert_eq!(mesh.vertices().len(), 20);
    assert_eq!(mesh.face_count(), 24);
    assert_eq!(mesh.vertices()[0], point(1.0, 2.0, -2.0));
    assert_eq!(mesh.vertices()[19], point(1.0, 2.0, 4.0));
    assert_eq!(mesh.faces()[0], MeshFace::Triangle([0, 2, 1]));
    assert_eq!(mesh.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
    assert_eq!(mesh.faces()[18], MeshFace::Triangle([13, 14, 19]));
    assert!(mesh.topology().is_solid());
    assert!(mesh.signed_volume().unwrap() > 0.0);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshSphere"));

    let mut quad_app = test_app();
    assert!(quad_app.try_start_interactive_command("MeshSphere Style=Quads Subdivisions=2"));
    quad_app.accept_drafting_point(point(0.0, 0.0, 0.0));
    quad_app.accept_drafting_point(point(2.0, 0.0, 4.0));
    let Geometry::Mesh(quad) = quad_app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created quad mesh sphere")
    };
    assert_eq!(quad.vertices().len(), 98);
    assert_eq!(quad.face_count(), 96);
    assert_eq!(quad.faces()[0], MeshFace::Quad([48, 79, 27, 0]));
    assert!(quad.topology().is_solid());

    let mut triangle_app = test_app();
    assert!(
        triangle_app.try_start_interactive_command("MeshSphere Style=Triangles Subdivisions=1")
    );
    triangle_app.accept_drafting_point(point(0.0, 0.0, 0.0));
    triangle_app.accept_drafting_point(point(2.0, 0.0, 4.0));
    let Geometry::Mesh(triangle) = triangle_app.document.objects().next().unwrap().geometry()
    else {
        panic!("expected an interactively created triangular mesh sphere")
    };
    assert_eq!(triangle.vertices().len(), 42);
    assert_eq!(triangle.face_count(), 80);
    assert_eq!(triangle.faces()[0], MeshFace::Triangle([0, 12, 14]));
    assert!(triangle.topology().is_solid());
}

#[test]
fn interactive_mesh_torus_retains_counts_and_validates_three_picks() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshTorus VerticalFaces=2"));
    assert!(!app.try_start_interactive_command("MeshTorus VerticalFaces=1000001 AroundFaces=3"));
    assert!(app.try_start_interactive_command("MeshTorus VerticalFaces=4 AroundFaces=6"));
    let center = point(1.0, 2.0, 1.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshTorus {
            center: Some(center),
            major_point: None,
            vertical_count: 4,
            around_count: 6,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    let major_point = point(5.0, 2.0, 1.0);
    app.accept_drafting_point(point(5.0, 2.0, 9.0));
    let awaiting_minor = InteractiveCommand::MeshTorus {
        center: Some(center),
        major_point: Some(major_point),
        vertical_count: 4,
        around_count: 6,
    };
    assert_eq!(app.active_command, Some(awaiting_minor));
    assert_eq!(awaiting_minor.anchor(), Some(major_point));

    app.accept_drafting_point(major_point);
    assert_eq!(app.active_command, Some(awaiting_minor));
    app.accept_drafting_point(center);
    assert_eq!(app.active_command, Some(awaiting_minor));
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(5.0, 3.0, 1.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh torus")
    };
    assert_eq!(mesh.vertices().len(), 24);
    assert_eq!(mesh.face_count(), 24);
    assert_eq!(mesh.vertices()[0], point(6.0, 2.0, 1.0));
    assert!((mesh.vertices()[6].x() - 5.0).abs() < 1.0e-12);
    assert!((mesh.vertices()[6].y() - 2.0).abs() < 1.0e-12);
    assert!((mesh.vertices()[6].z() - 2.0).abs() < 1.0e-12);
    assert_eq!(mesh.faces()[0], MeshFace::Quad([0, 1, 7, 6]));
    assert_eq!(mesh.faces()[18], MeshFace::Quad([18, 19, 1, 0]));
    assert!(mesh.topology().is_solid());
    assert!(mesh.signed_volume().unwrap() > 0.0);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshTorus"));
}

#[test]
fn interactive_mesh_cylinder_retains_options_and_validates_three_picks() {
    let mut app = test_app();
    assert!(!app.try_start_interactive_command("MeshCylinder AroundFaces=2"));
    assert!(!app.try_start_interactive_command(
        "MeshCylinder VerticalFaces=1000001 AroundFaces=3 Solid=No"
    ));
    assert!(app.try_start_interactive_command(
        "MeshCylinder VerticalFaces=2 AroundFaces=6 Solid=Yes BothSides=Yes CapFaceStyle=Quadrilaterals"
    ));
    let center = point(1.0, 2.0, 1.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(point(1.0, 2.0, 9.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshCylinder {
            center: Some(center),
            radius_point: None,
            vertical_count: 2,
            around_count: 6,
            solid: true,
            both_sides: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    let radius_point = point(3.0, 2.0, 1.0);
    app.accept_drafting_point(point(3.0, 2.0, 9.0));
    app.accept_drafting_point(point(9.0, 9.0, 1.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::MeshCylinder {
            center: Some(center),
            radius_point: Some(radius_point),
            vertical_count: 2,
            around_count: 6,
            solid: true,
            both_sides: true,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(9.0, 9.0, 4.0));
    assert_eq!(app.active_command, None);
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created mesh cylinder")
    };
    assert_eq!(mesh.vertices().len(), 32);
    assert_eq!(mesh.face_count(), 18);
    assert_eq!(mesh.faces()[12], MeshFace::Quad([18, 21, 20, 19]));
    assert_eq!(mesh.bounds().min().z(), -2.0);
    assert_eq!(mesh.bounds().max().z(), 4.0);
    assert!(mesh.topology().is_solid());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshCylinder"));
}
