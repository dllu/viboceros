//! primitives mesh planar command regressions.
use super::*;

#[test]
fn mesh_creates_unselected_attribute_copies_without_source_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        Point3::try_new(10.0, 6.0, 0.0).unwrap(),
        Point3::try_new(0.0, 6.0, 0.0).unwrap(),
    ])
    .unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(20.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        tolerance,
    )
    .unwrap();
    let solid = Brep::try_box(frame, [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]], tolerance).unwrap();
    let surface_attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Panel mesh")
        .with_object_color(ColorRgb::new(23, 61, 149));
    let solid_attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Solid mesh")
        .with_object_color(ColorRgb::new(193, 83, 31));
    let surface_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsSurface(surface.clone()),
            surface_attributes.clone(),
        )
        .unwrap();
    let solid_id = document
        .add_geometry_with_attributes(Geometry::Brep(solid.clone()), solid_attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("NURBS sources".to_owned()), [surface_id, solid_id])
        .unwrap();
    document
        .select_objects_direct([surface_id, solid_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "Mesh").unwrap(),
        "Created 2 polygon mesh(es) with 28 point(s) and 7 polygon(s); source object(s) retained"
    );
    assert_eq!(document.objects().len(), 4);
    assert!(document.is_selected(surface_id));
    assert!(document.is_selected(solid_id));
    let outputs = document
        .objects()
        .filter(|object| ![surface_id, solid_id].contains(&object.id()))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|output| !document.is_selected(output.id()))
    );
    let panel = outputs
        .iter()
        .find(|output| output.attributes().name() == Some("Panel mesh"))
        .unwrap();
    assert_eq!(panel.attributes(), &surface_attributes);
    let Geometry::Mesh(panel_mesh) = panel.geometry() else {
        panic!("Mesh must create polygon meshes")
    };
    assert_eq!(
        panel_mesh.faces(),
        &[viboceros_geometry::MeshFace::Quad([0, 1, 3, 2])]
    );
    let solid_output = outputs
        .iter()
        .find(|output| output.attributes().name() == Some("Solid mesh"))
        .unwrap();
    assert_eq!(solid_output.attributes(), &solid_attributes);
    let Geometry::Mesh(solid_mesh) = solid_output.geometry() else {
        panic!("Mesh must create polygon meshes")
    };
    assert_eq!(solid_mesh.face_count(), 6);
    assert!(solid_mesh.faces().iter().all(|face| face.is_quad()));
    assert!(solid_mesh.topology().is_solid());
    assert_eq!(
        document
            .group(group)
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([surface_id, solid_id])
    );
    assert_eq!(document.undo_label(), Some("Mesh"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert_eq!(
        document.object(solid_id).unwrap().geometry(),
        &Geometry::Brep(solid)
    );
}

#[test]
fn mesh_options_validate_and_mixed_geometry_rolls_back_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert_eq!(
        parse_mesh_arguments(&[]).unwrap(),
        MeshCommandOptions {
            density: 0.5,
            jagged_seams: false,
            simple_planes: false,
        }
    );
    assert!(matches!(
        registry.execute(&mut document, "Mesh"),
        Err(CommandError::NoObjectsSelected)
    ));
    for command in [
        "Mesh Density=-0.1",
        "Mesh Density=1.1",
        "Mesh Density=nan",
        "Mesh Density=0.5 Density=0.25",
        "Mesh JaggedSeams=Maybe",
        "Mesh SimplePlanes=Yes SimplePlanes=No",
        "Mesh Unknown=Yes",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0);
    }

    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        Point3::try_new(4.0, 3.0, 1.0).unwrap(),
        Point3::try_new(0.0, 3.0, 0.0).unwrap(),
    ])
    .unwrap();
    let surface_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let point_id = document
        .add_geometry(Geometry::Point(Point3::try_new(10.0, 10.0, 10.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([surface_id, point_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(
            &mut document,
            "Mesh Density 0.25 JaggedSeams Yes SimplePlanes No"
        ),
        Err(CommandError::UnsupportedMeshGeometry)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
}

#[test]
fn mesh_plane_creates_an_unselected_rhino_ordered_top_view_grid() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert_eq!(
        parse_mesh_plane_arguments(&[]).unwrap(),
        MeshPlaneCommandOptions {
            x_count: 10,
            y_count: 10,
        }
    );
    assert_eq!(
        parse_mesh_plane_arguments(&["_XCount=_2", "-YCount", "3"]).unwrap(),
        MeshPlaneCommandOptions {
            x_count: 2,
            y_count: 3,
        }
    );

    let message = registry
        .execute(&mut document, "MeshPlane XCount=2 YCount=3 4,3,2 0,0,9")
        .unwrap();
    assert!(message.ends_with("(2 × 3 faces)"));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(mesh) = document.objects().next().unwrap().geometry() else {
        panic!("MeshPlane must create a polygon mesh")
    };
    assert_eq!(mesh.vertices().len(), 12);
    assert_eq!(mesh.face_count(), 6);
    assert_eq!(mesh.vertices()[0], Point3::try_new(0.0, 0.0, 2.0).unwrap());
    assert_eq!(mesh.vertices()[11], Point3::try_new(4.0, 3.0, 2.0).unwrap());
    assert_eq!(
        mesh.faces(),
        &[
            viboceros_geometry::MeshFace::Quad([0, 1, 4, 3]),
            viboceros_geometry::MeshFace::Quad([1, 2, 5, 4]),
            viboceros_geometry::MeshFace::Quad([3, 4, 7, 6]),
            viboceros_geometry::MeshFace::Quad([4, 5, 8, 7]),
            viboceros_geometry::MeshFace::Quad([6, 7, 10, 9]),
            viboceros_geometry::MeshFace::Quad([7, 8, 11, 10]),
        ]
    );
    assert_eq!(
        mesh.face_normal(0).unwrap().as_vector().to_array(),
        [0.0, 0.0, 1.0]
    );
    assert_eq!(document.undo_label(), Some("MeshPlane"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 0);
}

#[test]
fn mesh_plane_rejects_invalid_counts_and_rectangles_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshPlane",
        "MeshPlane 0,0 4,3 XCount=0",
        "MeshPlane 0,0 4,3 XCount=nope",
        "MeshPlane 0,0 4,3 XCount=2 XCount=3",
        "MeshPlane 0,0 4,3 Unknown=2",
        "MeshPlane 0,0 4,3 XCount=1000001 YCount=1",
        "MeshPlane 0,0 0,3 XCount=2 YCount=3",
        "MeshPlane 0,0 4,0 XCount=2 YCount=3",
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
fn mesh_box_creates_an_unselected_rhino_ordered_closed_grid() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert_eq!(
        parse_mesh_box_arguments(&[]).unwrap(),
        MeshBoxCommandOptions {
            x_count: 1,
            y_count: 1,
            z_count: 1,
        }
    );
    assert_eq!(
        parse_mesh_box_arguments(&["_XCount=_2", "-YCount", "3", "ZCount=2"]).unwrap(),
        MeshBoxCommandOptions {
            x_count: 2,
            y_count: 3,
            z_count: 2,
        }
    );

    let message = registry
        .execute(
            &mut document,
            "MeshBox XCount=2 YCount 3 ZCount=2 5,8,3 1,2,3 9 9 -1",
        )
        .unwrap();
    assert!(message.ends_with("(2 × 3 × 2 side divisions)"));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.selected_object_count(), 0);
    let Geometry::Mesh(mesh) = document.objects().next().unwrap().geometry() else {
        panic!("MeshBox must create a polygon mesh")
    };
    assert_eq!(mesh.vertices().len(), 66);
    assert_eq!(mesh.face_count(), 32);
    assert_eq!(mesh.vertices()[0], Point3::try_new(1.0, 8.0, 3.0).unwrap());
    assert_eq!(
        mesh.faces()[0],
        viboceros_geometry::MeshFace::Quad([0, 3, 4, 1])
    );
    assert_eq!(
        mesh.faces()[6],
        viboceros_geometry::MeshFace::Quad([12, 15, 16, 13])
    );
    assert_eq!(
        mesh.faces()[12],
        viboceros_geometry::MeshFace::Quad([24, 27, 28, 25])
    );
    assert_eq!(
        mesh.faces()[16],
        viboceros_geometry::MeshFace::Quad([33, 37, 38, 34])
    );
    assert_eq!(
        mesh.faces()[22],
        viboceros_geometry::MeshFace::Quad([45, 48, 49, 46])
    );
    assert_eq!(
        mesh.faces()[26],
        viboceros_geometry::MeshFace::Quad([54, 58, 59, 55])
    );
    assert_eq!(
        mesh.bounds().min(),
        Point3::try_new(1.0, 2.0, -1.0).unwrap()
    );
    assert_eq!(mesh.bounds().max(), Point3::try_new(5.0, 8.0, 3.0).unwrap());
    assert_eq!(mesh.topology().topological_vertex_count(), 34);
    assert_eq!(mesh.topology().edge_count(), 64);
    assert!(mesh.topology().is_solid());
    assert!(document.tolerance().approx_eq(mesh.area().unwrap(), 128.0));
    assert!(
        document
            .tolerance()
            .approx_eq(mesh.signed_volume().unwrap(), 96.0)
    );
    assert_eq!(document.undo_label(), Some("MeshBox"));

    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(
            &mut document,
            "MeshBox 0,0,1 2,3,1 9,9,5 XCount=2 YCount=3 ZCount=4",
        )
        .unwrap();
    let Geometry::Mesh(point_height) = document.objects().next().unwrap().geometry() else {
        panic!("MeshBox height-point form must create a mesh")
    };
    assert_eq!(point_height.face_count(), 52);
    assert_eq!(
        point_height.bounds().max(),
        Point3::try_new(2.0, 3.0, 5.0).unwrap()
    );
}

#[test]
fn mesh_box_rejects_invalid_counts_and_extents_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "MeshBox",
        "MeshBox 0,0,0 2,3,0 4 XCount=0",
        "MeshBox 0,0,0 2,3,0 4 YCount=nope",
        "MeshBox 0,0,0 2,3,0 4 ZCount=2 ZCount=3",
        "MeshBox 0,0,0 2,3,0 4 Unknown=2",
        "MeshBox 0,0,0 2,3,0 4 XCount=500001 YCount=1 ZCount=1",
        "MeshBox 0,0,0 0,3,0 4",
        "MeshBox 0,0,0 2,0,0 4",
        "MeshBox 0,0,0 2,3,0 0",
        "MeshBox XCount=2 0,0,0 2,3,0 4 extra",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().len(), 0, "{command}");
        assert_eq!(document.undo_label(), None, "{command}");
    }
}
