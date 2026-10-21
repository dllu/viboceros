use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{AffineTransform3, Point3, Vector3};
#[path = "support/subd.rs"]
mod native;

#[test]
fn subd_faces_match_rhino_control_topology_tags_and_exact_surface_stations() {
    for row in native::rows() {
        let value = &row["value"];
        assert_eq!(value["success"], true);
        let mut document = native::setup();
        let commands = CommandRegistry::with_builtins();
        commands
            .execute(&mut document, &native::command(value))
            .unwrap();
        let check = native::check(&document, value);
        assert_eq!(document.undo_label(), Some("3DFace"));
        document.undo().unwrap();
        assert_eq!(document.objects().count(), 0);
        document.redo().unwrap();
        assert_eq!(check, native::check(&document, value));
        println!(
            "SUBD_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn subd_save_open_control_edits_and_units_retain_real_geometry() {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    commands
        .execute(&mut document, "3DFace Output=SubD 0,0,0 4,0,0 4,3,2 0,3,0")
        .unwrap();
    let object = document.objects().next().unwrap();
    let id = object.id();
    let Geometry::SubD(original) = object.geometry() else {
        panic!()
    };
    let original = original.clone();
    let offset = AffineTransform3::from_translation(Vector3::try_new(2., -1., 5.).unwrap());
    let moved = Geometry::SubD(original.clone())
        .transformed(offset, document.tolerance())
        .unwrap();
    let Geometry::SubD(moved) = moved else {
        panic!()
    };
    for (a, b) in original.surface_points().iter().zip(moved.surface_points()) {
        assert!(offset.transform_point(*a).unwrap().distance_to(*b).unwrap() < 1e-12);
    }
    let grip_edit = Geometry::SubD(original.clone())
        .with_transformed_grips(
            &std::collections::BTreeSet::from([0]),
            offset,
            document.tolerance(),
        )
        .unwrap();
    let Geometry::SubD(grip_edit) = grip_edit else {
        panic!()
    };
    assert_eq!(
        grip_edit.control_points()[0],
        offset
            .transform_point(original.control_points()[0])
            .unwrap()
    );
    assert_eq!(
        &grip_edit.control_points()[1..],
        &original.control_points()[1..]
    );
    assert_eq!(grip_edit.vertex_tags(), original.vertex_tags());
    let scale = AffineTransform3::try_nonuniform_scale(
        Point3::try_new(0., 0., 0.).unwrap(),
        [-2., 3., 0.5],
    )
    .unwrap();
    let scaled = original.transformed(scale).unwrap();
    for (a, b) in original
        .surface_points()
        .iter()
        .zip(scaled.surface_points())
    {
        assert!(scale.transform_point(*a).unwrap().distance_to(*b).unwrap() < 1e-12);
    }
    let mut controls = original.control_points().to_vec();
    controls[0] = Point3::try_new(-1., -1., 0.).unwrap();
    let edited = original.try_with_control_points(&controls).unwrap();
    document
        .replace_object_geometries(vec![(id, Geometry::SubD(edited.clone()))])
        .unwrap();
    document.undo().unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::SubD(original.clone())
    );
    document.redo().unwrap();
    let path = std::env::temp_dir().join(format!("viboceros-subd-{}.3dm", id));
    commands
        .execute(&mut document, &format!("SaveAs {}", path.display()))
        .unwrap();
    let mut reopened = Document::default();
    commands
        .execute(&mut reopened, &format!("Open {}", path.display()))
        .unwrap();
    let restored = reopened.object(id).unwrap();
    let Geometry::SubD(subd) = restored.geometry() else {
        panic!("3DM must retain a SubD")
    };
    assert_eq!(subd.control_points(), edited.control_points());
    assert_eq!(subd.surface_points(), edited.surface_points());
    assert_eq!(subd.faces(), edited.faces());
    assert_eq!(subd.vertex_tags(), edited.vertex_tags());
    let native =
        viboceros_io::check_3dm_geometry(&viboceros_io::ThreeDmGeometry::SubD(subd.clone()));
    assert_eq!(
        native.status,
        viboceros_io::GeometryCheckStatus::Valid,
        "{}",
        native.diagnostics
    );
    let scaled = viboceros_io::read_3dm_file_in_units(
        &path,
        &viboceros_geometry::LengthUnitSystem::Meters,
        document.tolerance(),
    )
    .unwrap();
    let viboceros_io::ThreeDmGeometry::SubD(scaled) = &scaled.objects[0].geometry else {
        panic!()
    };
    for (a, b) in edited.control_points().iter().zip(scaled.control_points()) {
        for (x, y) in a.to_array().into_iter().zip(b.to_array()) {
            assert!((x * 0.001 - y).abs() < 1e-14);
        }
    }
    std::fs::remove_file(path).unwrap();
}
