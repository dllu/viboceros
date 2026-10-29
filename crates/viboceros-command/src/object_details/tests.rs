use super::*;
use viboceros_document::{ObjectAttributes, SelectionMode};
use viboceros_geometry::{MeshFace, WeightedPoint3};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn spline() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 2., 0.), 2.).unwrap(),
            WeightedPoint3::try_new(p(3., 0., 0.), 1.).unwrap(),
        ],
        vec![-3., -3., -3., 8., 8., 8.],
    )
    .unwrap()
}

#[test]
fn selected_object_properties_include_identity_layer_groups_and_both_user_text_stores() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let layer = doc
        .add_layer("曲線", viboceros_document::ColorRgb::BLACK)
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(spline()),
            ObjectAttributes::on_layer(layer).with_name("Rail\nA"),
        )
        .unwrap();
    doc.add_group(Some("Profiles".into()), [id]).unwrap();
    doc.set_object_user_text([id], "part", Some("A")).unwrap();
    doc.set_object_geometry_user_text([id], "role", Some("rail"))
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.add_geometry(Geometry::Point(p(9., 9., 9.))).unwrap();
    doc.undo().unwrap();
    for command in ["_wHaT", "_lIsT"] {
        let before = format!("{doc:?}");
        let storage = doc.object(id).unwrap().geometry_snapshot().clone();
        let output = registry.execute(&mut doc, command).unwrap();
        for expected in [
            id.to_string(),
            "曲線".into(),
            "Profiles".into(),
            "Rail\\nA".into(),
            "Attribute user text: \"part\" = \"A\"".into(),
            "Geometry user text: \"role\" = \"rail\"".into(),
            "NURBS curve: degree 2, 3 control points; rational: true".into(),
            "Domain: [-3, 8]".into(),
        ] {
            assert!(output.contains(&expected), "{expected}: {output}");
        }
        if command == "_lIsT" {
            assert!(output.contains("ON_NurbsCurve"));
            assert!(output.contains("Knot Vector"));
        }
        assert_eq!(format!("{doc:?}"), before);
        assert!(doc.can_redo());
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &storage);
    }
}

#[test]
fn reports_use_selection_order_and_invalid_invocations_leave_the_entire_document_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let a = doc.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
    let b = doc.add_geometry(Geometry::EmptyMesh).unwrap();
    for command in ["What", "List"] {
        let before = format!("{doc:?}");
        assert!(matches!(
            registry.execute(&mut doc, command),
            Err(CommandError::NoObjectsSelected)
        ));
        assert_eq!(format!("{doc:?}"), before);
    }
    doc.select_objects_direct([b], SelectionMode::Replace)
        .unwrap();
    doc.select_objects_direct([a], SelectionMode::Add).unwrap();
    for command in ["What", "List"] {
        let text = registry.execute(&mut doc, command).unwrap();
        assert!(text.find(&b.to_string()).unwrap() < text.find(&a.to_string()).unwrap());
        assert!(text.contains("Empty mesh: 0 vertices, 0 faces"));
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("{command} unexpected"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}

#[test]
fn technical_reports_cover_surfaces_meshes_subds_and_invalid_geometry_without_editing() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(2., 0., 0.), p(2., 3., 0.), p(0., 3., 0.)])
            .unwrap();
    let subd = viboceros_geometry::SubD::try_from_polygons(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)],
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let bad = TriangleMesh::try_from_face_records(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    for geometry in [
        Geometry::NurbsSurface(surface),
        Geometry::SubD(subd),
        Geometry::Mesh(bad),
    ] {
        doc.add_geometry(geometry).unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    let text = registry.execute(&mut doc, "List").unwrap();
    for expected in ["ON_NurbsSurface", "SubD", "ON_Mesh", "3 vertices, 1 faces"] {
        assert!(text.contains(expected), "{text}");
    }
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn native_instance_dump_keeps_the_definition_identity_and_placement() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 Sample").unwrap();
    registry.execute(&mut doc, "Insert Sample 5,6,7").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    let text = registry.execute(&mut doc, "List").unwrap();
    assert!(text.contains("Block instance: \"Sample\""), "{text}");
    assert!(text.contains("Instance definition:"), "{text}");
    assert!(text.contains("Transform:"), "{text}");
    assert_eq!(format!("{doc:?}"), before);
}
