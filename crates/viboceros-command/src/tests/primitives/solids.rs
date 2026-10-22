//! primitives solids command regressions.
use super::*;

#[test]
fn sphere_creates_exact_opennurbs_surface_from_radius_or_point() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Sphere 1,2,3 2.5").unwrap();
    let sphere_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(sphere_id));
    let Geometry::NurbsSurface(surface) = document.object(sphere_id).unwrap().geometry() else {
        panic!("Sphere must create a NURBS surface")
    };
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 2);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 5);
    assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
    assert_eq!(
        surface.domain_v(),
        -std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2
    );
    assert_eq!(
        surface.control_point(0, 0).unwrap().point(),
        Point3::try_new(1.0, 2.0, 0.5).unwrap()
    );
    assert_eq!(
        surface.control_point(0, 1).unwrap().point(),
        Point3::try_new(3.5, 2.0, 0.5).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Sphere"));

    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "_Sph 1,2,3 1,5,3").unwrap();
    let Geometry::NurbsSurface(point_radius_surface) =
        document.objects().next().unwrap().geometry()
    else {
        panic!("Sph alias must create a NURBS surface")
    };
    let equator = point_radius_surface.evaluate(0.0, 0.0).unwrap();
    assert_eq!(equator, Point3::try_new(4.0, 2.0, 3.0).unwrap());

    for invalid in [
        "Sphere 0,0,0 0",
        "Sphere 0,0,0 -1",
        "Sphere 0,0,0 inf",
        "Sphere 0,0,0 1 2",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Sphere"));
}

#[test]
fn ellipsoid_creates_exact_affine_sphere_from_radii_or_axis_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Ellipsoid 1,2,3 2 3 4")
        .unwrap();
    let ellipsoid_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(ellipsoid_id));
    let Geometry::NurbsSurface(surface) = document.object(ellipsoid_id).unwrap().geometry() else {
        panic!("Ellipsoid must create a NURBS surface")
    };
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 2);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 5);
    assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
    assert_eq!(
        surface.domain_v(),
        -std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2
    );
    assert_eq!(
        surface.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(3.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        surface.evaluate(std::f64::consts::FRAC_PI_2, 0.0).unwrap(),
        Point3::try_new(1.0, 5.0, 3.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Ellipsoid"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Ellipsoid 1,2,3 1,4,3 -2,3,3 3,4,7")
        .unwrap();
    let Geometry::NurbsSurface(oriented) = document.objects().next().unwrap().geometry() else {
        panic!("Ellipsoid must support oriented axis points")
    };
    assert_eq!(
        oriented.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 4.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.evaluate(std::f64::consts::FRAC_PI_2, 0.0).unwrap(),
        Point3::try_new(-2.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.evaluate(0.0, std::f64::consts::FRAC_PI_2).unwrap(),
        Point3::try_new(1.0, 2.0, 7.0).unwrap()
    );

    for invalid in [
        "Ellipsoid 0,0,0 0 2 3",
        "Ellipsoid 0,0,0 1 -2 3",
        "Ellipsoid 0,0,0 1 2 inf",
        "Ellipsoid 0,0,0 1,0,0 2,0,0 0,0,3",
        "Ellipsoid 0,0,0 1,0,0 0,2,0 0,0,0",
        "Ellipsoid 0,0,0 1,0,0 0,2,0 1,1,0",
        "Ellipsoid 0,0,0 1 2",
        "Ellipsoid 0,0,0 1 2 3 4",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Ellipsoid"));
}

#[test]
fn cone_creates_exact_apex_surface_for_signed_arbitrary_axes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Cone 0,0,0 3 4 Solid=No")
        .unwrap();
    let cone_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(cone_id));
    let Geometry::NurbsSurface(surface) = document.object(cone_id).unwrap().geometry() else {
        panic!("Cone must create a NURBS surface")
    };
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 2);
    assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
    assert_eq!(surface.domain_v(), -4.0..=0.0);
    assert_eq!(
        surface.evaluate(0.0, -4.0).unwrap(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        surface.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 4.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Cone"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Cone 1,2,3 4,7,3 -4 Axis=0,1,0 Solid=No")
        .unwrap();
    let Geometry::NurbsSurface(negative) = document.objects().next().unwrap().geometry() else {
        panic!("Cone must support a point radius and arbitrary axis")
    };
    assert_eq!(negative.domain_v(), 0.0..=4.0);
    assert_eq!(
        negative.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(1.0, -2.0, 3.0).unwrap()
    );
    assert_eq!(
        negative.evaluate(0.0, 4.0).unwrap(),
        Point3::try_new(4.0, 2.0, 3.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(&mut document, "Cone 1,2,3 2.5 -4 Solid=Yes")
        .unwrap();
    assert!(result.contains("closed B-rep cone"));
    let Geometry::Brep(solid) = document.objects().next().unwrap().geometry() else {
        panic!("Cone Solid=Yes must create a B-rep")
    };
    assert_eq!(solid.faces().len(), 2);
    assert!(solid.is_solid());
    let mesh = solid.tessellate(8, document.tolerance()).unwrap();
    assert!(mesh.topology().is_solid());
    let expected_volume = std::f64::consts::PI * 2.5 * 2.5 * 4.0 / 3.0;
    assert!((mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume < 0.01);

    for invalid in [
        "Cone 0,0,0 1 0",
        "Cone 0,0,0 0 2",
        "Cone 0,0,0 0,0,2 3 Axis=0,0,1",
        "Cone 0,0,0 1 2 Axis=0,0,0",
        "Cone 0,0,0 1 2 BothSides=Yes",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Cone"));
}

#[test]
fn truncated_cone_creates_exact_open_and_solid_frustums() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "TruncatedCone 0,0,0 3 4 1")
        .unwrap();
    let open_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(open_id));
    let Geometry::NurbsSurface(surface) = document.object(open_id).unwrap().geometry() else {
        panic!("TruncatedCone must create a NURBS surface by default")
    };
    let slant = 20.0_f64.sqrt();
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 2);
    assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU);
    assert_eq!(surface.domain_v(), 0.0..=slant);
    assert_eq!(
        surface.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        surface.evaluate(0.0, slant).unwrap(),
        Point3::try_new(1.0, 0.0, 4.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("TruncatedCone"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "TruncatedCone 1,2,3 4,7,3 -4 2 Axis=0,1,0 Solid=No",
        )
        .unwrap();
    let Geometry::NurbsSurface(negative) = document.objects().next().unwrap().geometry() else {
        panic!("TruncatedCone must support a picked base radius and arbitrary axis")
    };
    let negative_slant = 17.0_f64.sqrt();
    assert_eq!(negative.domain_v(), 0.0..=negative_slant);
    assert_eq!(
        negative.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        negative.evaluate(0.0, negative_slant).unwrap(),
        Point3::try_new(3.0, -2.0, 3.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(&mut document, "TruncatedCone 1,2,3 2.5 -4 1.5 Solid=Yes")
        .unwrap();
    assert!(result.contains("closed B-rep truncated cone"));
    let Geometry::Brep(solid) = document.objects().next().unwrap().geometry() else {
        panic!("TruncatedCone Solid=Yes must create a B-rep")
    };
    assert_eq!(solid.faces().len(), 3);
    assert!(solid.is_solid());
    let mesh = solid.tessellate(12, document.tolerance()).unwrap();
    assert!(mesh.topology().is_solid());
    let expected_volume =
        std::f64::consts::PI * 4.0 * (2.5_f64.powi(2) + 2.5 * 1.5 + 1.5_f64.powi(2)) / 3.0;
    assert!((mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume < 0.01);

    for invalid in [
        "TruncatedCone",
        "TruncatedCone 0,0,0 0 4 1",
        "TruncatedCone 0,0,0 3 0 1",
        "TruncatedCone 0,0,0 3 4 0",
        "TruncatedCone 0,0,0 3 4 -1",
        "TruncatedCone 0,0,0 3 4 1 Axis=0,0,0",
        "TruncatedCone 0,0,0 0,0,2 4 1 Axis=0,0,1",
        "TruncatedCone 0,0,0 3 4 1 BothSides=Yes",
        "TruncatedCone 0,0,0 3 4 1 Solid=Yes Solid=No",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("TruncatedCone"));
}

#[test]
fn pyramid_commands_create_exact_open_and_capped_polygonal_breps() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let result = registry
        .execute(&mut document, "Pyramid 4 0,0,0 3 5")
        .unwrap();
    assert!(result.contains("4-sided open B-rep pyramid"));
    let open_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(open_id));
    let Geometry::Brep(open) = document.object(open_id).unwrap().geometry() else {
        panic!("Pyramid must create a joined B-rep")
    };
    assert_eq!(open.vertices().len(), 5);
    assert_eq!(open.edges().len(), 8);
    assert_eq!(open.faces().len(), 4);
    assert!(!open.is_closed());
    assert_eq!(
        open.vertices()[0].point(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        open.vertices()[2].point(),
        Point3::try_new(0.0, 0.0, 5.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Pyramid"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "Pyramid 5 1,2,3 4,2,3 -4 Axis=0,1,0 Solid=Yes",
        )
        .unwrap();
    let Geometry::Brep(negative) = document.objects().next().unwrap().geometry() else {
        panic!("Pyramid must support a picked radius and arbitrary axis")
    };
    assert_eq!(negative.faces().len(), 6);
    assert!(negative.is_solid());
    assert_eq!(
        negative.vertices()[0].point(),
        Point3::try_new(4.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        negative.vertices()[2].point(),
        Point3::try_new(1.0, -2.0, 3.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(&mut document, "TruncatedPyramid 4 0,0,0 3 5 1 Solid=Yes")
        .unwrap();
    assert!(result.contains("4-sided closed B-rep truncated pyramid"));
    let Geometry::Brep(truncated) = document.objects().next().unwrap().geometry() else {
        panic!("TruncatedPyramid must create a B-rep")
    };
    assert_eq!(truncated.vertices().len(), 8);
    assert_eq!(truncated.edges().len(), 12);
    assert_eq!(truncated.faces().len(), 6);
    assert!(truncated.is_solid());
    assert_eq!(
        truncated.vertices()[0].point(),
        Point3::try_new(1.0, 0.0, 5.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "TruncatedPyramid 3 1,2,3 4,2,3 -4 2 Axis=0,1,0 Solid=No",
        )
        .unwrap();
    let Geometry::Brep(open_truncated) = document.objects().next().unwrap().geometry() else {
        panic!("TruncatedPyramid must support signed arbitrary axes")
    };
    assert_eq!(open_truncated.faces().len(), 3);
    assert!(!open_truncated.is_closed());
    assert_eq!(
        open_truncated.vertices()[0].point(),
        Point3::try_new(3.0, -2.0, 3.0).unwrap()
    );

    for invalid in [
        "Pyramid",
        "Pyramid 2 0,0,0 3 5",
        "Pyramid 4 0,0,0 0 5",
        "Pyramid 4 0,0,0 3 0",
        "Pyramid 4 0,0,0 3 5 Axis=0,0,0",
        "Pyramid 4 0,0,0 3 5 Solid=Yes Solid=No",
        "TruncatedPyramid",
        "TruncatedPyramid 2 0,0,0 3 5 1",
        "TruncatedPyramid 4 0,0,0 3 5 0",
        "TruncatedPyramid 4 0,0,0 3 0 1",
        "TruncatedPyramid 4 0,0,0 3 5 -1",
        "TruncatedPyramid 4 0,0,0 0,0,2 5 1 Axis=0,0,1",
        "TruncatedPyramid 4 0,0,0 3 5 1 BothSides=Yes",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("TruncatedPyramid"));
}

#[test]
fn paraboloid_supports_focus_vertex_marker_and_exact_solid_forms() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let result = registry
        .execute(&mut document, "Paraboloid 0,0,1 0,0,2 2,0,4")
        .unwrap();
    assert!(result.contains("open B-rep paraboloid"));
    let Geometry::Brep(open) = document.objects().next().unwrap().geometry() else {
        panic!("Paraboloid's default Focus form must create a B-rep")
    };
    assert_eq!(open.faces().len(), 1);
    assert!(!open.is_solid());
    let focal_distance = (13.0_f64.sqrt() - 3.0) * 0.5;
    assert!(
        document
            .tolerance()
            .approx_eq(open.vertices()[0].point().z(), 1.0 - focal_distance)
    );
    assert!(
        open.vertices()[1]
            .point()
            .distance_to(Point3::try_new(2.0, 0.0, 4.0).unwrap())
            .unwrap()
            < 1.0e-14
    );
    assert_eq!(document.undo_label(), Some("Paraboloid"));

    registry.execute(&mut document, "Undo").unwrap();
    let result = registry
        .execute(
            &mut document,
            "Paraboloid Vertex 1,2,3 2,2,3 2,5,8 MarkFocus=Yes Solid=Yes",
        )
        .unwrap();
    assert!(result.contains("closed B-rep paraboloid"));
    assert!(result.contains("focus point"));
    let mut objects = document.objects();
    let Geometry::Point(marked_focus) = objects.next().unwrap().geometry() else {
        panic!("MarkFocus=Yes must add the focus before the paraboloid")
    };
    assert_eq!(*marked_focus, Point3::try_new(2.0, 2.0, 3.0).unwrap());
    let Geometry::Brep(solid) = objects.next().unwrap().geometry() else {
        panic!("Solid=Yes must add a capped paraboloid B-rep")
    };
    assert_eq!(solid.faces().len(), 2);
    assert!(solid.is_solid());
    assert!(
        solid.vertices()[1]
            .point()
            .distance_to(Point3::try_new(9.5, 5.0, 8.0).unwrap())
            .unwrap()
            < 1.0e-14
    );
    drop(objects);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);

    for invalid in [
        "Paraboloid",
        "Paraboloid Focus 0,0,0 0,0,0 1,0,0",
        "Paraboloid Focus 0,0,0 0,0,1 0,0,2",
        "Paraboloid Vertex 0,0,0 0,0,0 1,0,0",
        "Paraboloid Vertex 0,0,0 0,0,1 0,0,2",
        "Paraboloid Vertex 0,0,0 0,0,1 nan,0,0",
        "Paraboloid Vertex 0,0,0 0,0,1 2,0,0 MarkFocus=Maybe",
        "Paraboloid Vertex 0,0,0 0,0,1 2,0,0 Solid=Yes Solid=No",
        "Paraboloid Vertex 0,0,0 0,0,1 2,0,0 Axis=0,0,1",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.undo_label(), None);
}

#[test]
fn tube_creates_exact_concentric_walls_and_annular_caps() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let result = registry.execute(&mut document, "Tube 0,0,0 3 1 5").unwrap();
    assert!(result.contains("closed B-rep tube"));
    let tube_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(tube_id));
    let Geometry::Brep(tube) = document.object(tube_id).unwrap().geometry() else {
        panic!("Tube must create a B-rep")
    };
    assert_eq!(tube.faces().len(), 4);
    assert_eq!(tube.edges().len(), 6);
    assert!(tube.is_solid());
    assert_eq!(
        tube.vertices()[0].point(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        tube.vertices()[2].point(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Tube"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Tube 1,2,3 4,7,3 1 -4 Axis=0,1,0")
        .unwrap();
    let Geometry::Brep(negative) = document.objects().next().unwrap().geometry() else {
        panic!("Tube must support a picked radius and signed arbitrary axis")
    };
    assert_eq!(
        negative.vertices()[0].point(),
        Point3::try_new(4.0, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        negative.vertices()[1].point(),
        Point3::try_new(4.0, -2.0, 3.0).unwrap()
    );
    assert_eq!(
        negative.vertices()[2].point(),
        Point3::try_new(2.0, 2.0, 3.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "Tube 0,0,0 3 5 WallThickness=1 BothSides=Yes",
        )
        .unwrap();
    let Geometry::Brep(both_sides) = document.objects().next().unwrap().geometry() else {
        panic!("Tube WallThickness and BothSides must create a B-rep")
    };
    assert_eq!(
        both_sides.vertices()[0].point(),
        Point3::try_new(4.0, 0.0, -5.0).unwrap()
    );
    assert_eq!(
        both_sides.vertices()[1].point(),
        Point3::try_new(4.0, 0.0, 5.0).unwrap()
    );
    assert_eq!(
        both_sides.vertices()[2].point(),
        Point3::try_new(3.0, 0.0, -5.0).unwrap()
    );
    let mesh = both_sides.tessellate(12, document.tolerance()).unwrap();
    let expected_volume = std::f64::consts::PI * (4.0_f64.powi(2) - 3.0_f64.powi(2)) * 10.0;
    assert!((mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume < 0.01);

    for invalid in [
        "Tube",
        "Tube 0,0,0 3 5",
        "Tube 0,0,0 0 1 5",
        "Tube 0,0,0 3 3 5",
        "Tube 0,0,0 3 1 0",
        "Tube 0,0,0 3 1 5 WallThickness=1",
        "Tube 0,0,0 3 5 WallThickness=0",
        "Tube 0,0,0 3 5 WallThickness=-1",
        "Tube 0,0,0 3 1 5 BothSides=Maybe",
        "Tube 0,0,0 3 1 5 Axis=0,0,0",
        "Tube 0,0,0 0,0,2 1 5 Axis=0,0,1",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Tube"));
}

#[test]
fn torus_creates_exact_tensor_surface_with_oriented_point_radius() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Torus 1,2,3 5 1.5")
        .unwrap();
    let torus_id = document.objects().next().unwrap().id();
    assert!(!document.is_selected(torus_id));
    let Geometry::NurbsSurface(surface) = document.object(torus_id).unwrap().geometry() else {
        panic!("Torus must create a NURBS surface")
    };
    assert_eq!(surface.degree_u(), 2);
    assert_eq!(surface.degree_v(), 2);
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 9);
    assert_eq!(surface.domain_u(), 0.0..=std::f64::consts::TAU * 5.0);
    assert_eq!(surface.domain_v(), 0.0..=std::f64::consts::TAU * 1.5);
    assert_eq!(
        surface.control_point(0, 0).unwrap().point(),
        Point3::try_new(7.5, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        surface
            .evaluate(std::f64::consts::TAU * 5.0 * 0.25, 0.0)
            .unwrap(),
        Point3::try_new(1.0, 8.5, 3.0).unwrap()
    );
    assert_eq!(document.undo_label(), Some("Torus"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Torus 1,2,3 4,7,3 1.5 Axis=0,1,0")
        .unwrap();
    let Geometry::NurbsSurface(oriented) = document.objects().next().unwrap().geometry() else {
        panic!("Torus must support a point radius and arbitrary axis")
    };
    let domain_v = std::f64::consts::TAU * 1.5;
    assert_eq!(oriented.domain_u(), 0.0..=std::f64::consts::TAU * 3.0);
    assert_eq!(
        oriented.evaluate(0.0, 0.0).unwrap(),
        Point3::try_new(5.5, 2.0, 3.0).unwrap()
    );
    assert_eq!(
        oriented.evaluate(0.0, domain_v * 0.25).unwrap(),
        Point3::try_new(4.0, 3.5, 3.0).unwrap()
    );

    for invalid in [
        "Torus 0,0,0 1 0",
        "Torus 0,0,0 1 1",
        "Torus 0,0,0 1 2",
        "Torus 0,0,0 -2 1",
        "Torus 0,0,0 0,0,2 1 Axis=0,0,1",
        "Torus 0,0,0 3 1 Axis=0,0,0",
        "Torus 0,0,0 3 1 Solid=No",
        "Torus 0,0,0 3 1 BothSides=Yes",
        "Torus 0,0,0 3 1 Axis=0,0,1 Axis=0,1,0",
        "Torus 0,0,0 3 1 Other=Yes",
        "Torus 0,0,0 3 1 2",
        "Torus 0,0,0 inf 1",
    ] {
        assert!(registry.execute(&mut document, invalid).is_err());
    }
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), Some("Torus"));
}

#[test]
fn box_creates_an_exact_closed_brep_from_signed_height_or_height_point() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let message = registry
        .execute(&mut document, "Box 1,2,3 5,8,3 -4")
        .unwrap();
    assert!(message.contains("closed extrusion box"));
    let Geometry::Brep(brep) = document.objects().next().unwrap().geometry() else {
        panic!("Box must create a B-rep")
    };
    assert_eq!(brep.vertices().len(), 8);
    assert_eq!(brep.edges().len(), 12);
    assert_eq!(brep.faces().len(), 6);
    assert!(brep.is_solid());
    assert_eq!(
        brep.bounds().min(),
        Point3::try_new(1.0, 2.0, -1.0).unwrap()
    );
    assert_eq!(brep.bounds().max(), Point3::try_new(5.0, 8.0, 3.0).unwrap());
    let mesh = brep.tessellate(1, document.tolerance()).unwrap();
    assert!(mesh.topology().is_solid());
    assert!(
        document
            .tolerance()
            .approx_eq(mesh.signed_volume().unwrap(), 96.0)
    );
    assert_eq!(document.undo_label(), Some("Box"));
    let model = document_3dm_model(&document).unwrap();
    assert!(matches!(
        model.objects[0].geometry,
        ThreeDmGeometry::Brep(_)
    ));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "Box 0,0,1 2,3,1 9,9,5")
        .unwrap();
    let Geometry::Brep(point_height) = document.objects().next().unwrap().geometry() else {
        panic!("Box height-point form must create a B-rep")
    };
    assert_eq!(
        point_height.bounds().max(),
        Point3::try_new(2.0, 3.0, 5.0).unwrap()
    );

    registry.execute(&mut document, "Undo").unwrap();
    for invalid in [
        "Box 0,0,0 0,3,0 4",
        "Box 0,0,0 2,3,0 0",
        "Box 0,0,0 2,3,0 4 extra",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 0);
    }
}
