use super::*;
use viboceros_geometry::Point3;

#[test]
fn mesh_ellipsoid_replays_native_modes_vertices_caps_and_preferences() {
    let profiles: Vec<Vec<serde_json::Value>> =
        serde_json::from_str(include_str!("rhino_meshes.json")).unwrap();
    for profile in profiles {
        let registry = CommandRegistry::with_builtins();
        for capture in profile {
            let mut document = Document::default();
            let mut command = capture["command"].as_str().unwrap().to_owned();
            if command.contains("{guide}") {
                registry
                    .execute(&mut document, "Line 0,0,0 0,10,10")
                    .unwrap();
                let guide = document.objects().next().unwrap().id();
                command = command.replace("{guide}", &guide.to_string());
            }
            let previous_count = document.objects().count();
            registry
                .execute(&mut document, &command)
                .unwrap_or_else(|error| panic!("{command}: {error}"));
            let mut created = document.objects().skip(previous_count);
            let Geometry::Mesh(mesh) = created.next().unwrap().geometry() else {
                panic!("{command}: expected a polygon mesh");
            };
            let vertices: Vec<[f64; 3]> =
                serde_json::from_value(capture["vertices"].clone()).unwrap();
            let faces: Vec<Vec<u32>> = serde_json::from_value(capture["faces"].clone()).unwrap();
            assert_eq!(vertices.len(), mesh.vertices().len(), "{command}");
            assert_eq!(faces.len(), mesh.faces().len(), "{command}");
            assert!(mesh.topology().is_solid(), "{command}");
            assert!(mesh.signed_volume().unwrap() > 0., "{command}");
            for (index, (&actual, expected)) in mesh.vertices().iter().zip(vertices).enumerate() {
                let point = Point3::try_from(expected).unwrap();
                assert!(
                    actual.distance_to(point).unwrap() < 1e-10,
                    "{command} vertex {index}: {actual:?} != {point:?}"
                );
            }
            for (index, (actual, expected)) in mesh.faces().iter().zip(faces).enumerate() {
                assert_eq!(actual.indices(), expected, "{command} face {index}");
            }
            let expected_foci: Vec<[f64; 3]> =
                serde_json::from_value(capture["foci"].clone()).unwrap();
            let foci = created
                .map(|object| match object.geometry() {
                    Geometry::Point(point) => point.to_array(),
                    _ => panic!("{command}: expected focus markers"),
                })
                .collect::<Vec<_>>();
            assert_eq!(foci, expected_foci, "{command}");
            assert_eq!(document.undo_label(), Some("MeshEllipsoid"));
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().count(), previous_count, "{command}");
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(
                document.objects().count(),
                previous_count + 1 + foci.len(),
                "{command}"
            );
        }
    }
}
