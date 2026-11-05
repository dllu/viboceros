#[path = "support/observation.rs"]
mod observation;
#[path = "support/revolve_subd.rs"]
mod topology;
use viboceros_command::CommandRegistry;
use viboceros_document::Geometry;
use viboceros_geometry::Tolerance;
use viboceros_oracle::revolve_subd::{Fixture, setup, subd_record};

#[test]
fn subd_revolutions_roundtrip_control_topology_limit_samples_and_normal_meshes() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in [
        "revolve_subd_pilot",
        "revolve_subd_holdouts",
        "revolve_subd_edges",
        "revolve_subd_policy",
    ] {
        let query: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        for op in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "revolve_subd")
        {
            count += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let (mut doc, source) = setup(&f, Tolerance::DEFAULT).unwrap();
            let commands = CommandRegistry::with_builtins();
            f.configure(&commands, &mut doc).unwrap();
            commands.execute(&mut doc, &f.input()).unwrap();
            let object = doc.objects().find(|o| o.id() != source).unwrap();
            let id = object.id();
            let Geometry::SubD(original) = object.geometry() else {
                panic!("SubD output required")
            };
            let original = original.clone();
            let path = directory.path().join("revolve.3dm");
            commands
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let model = viboceros_io::read_3dm_file(&path, doc.tolerance()).unwrap();
            assert_eq!(model.unsupported_object_count(), 0);
            let object = model.objects.iter().find(|o| o.id == id.uuid()).unwrap();
            let viboceros_io::ThreeDmGeometry::SubD(restored) = &object.geometry else {
                panic!("SubD lost in 3DM")
            };
            assert_eq!(restored.control_points(), original.control_points());
            assert_eq!(restored.faces(), original.faces());
            assert_eq!(restored.edges(), original.edges());
            assert_eq!(restored.vertex_tags(), original.vertex_tags());
            assert_eq!(restored.edge_tags(), original.edge_tags());
            let expected = native["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == op["id"])
                .unwrap();
            topology::compare(
                &subd_record(restored).unwrap(),
                &expected["value"]["after"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["subd"],
                &format!("{}/3dm", op["id"]),
                topology::epsilon(op["id"].as_str().unwrap()),
            );
            for subd in [&original, restored] {
                assert!(subd.tessellate_with_normals(2).is_ok(), "{}", op["id"]);
            }
        }
    }
    assert_eq!(count, 80);
}
