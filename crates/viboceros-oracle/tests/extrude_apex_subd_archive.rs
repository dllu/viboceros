#[path = "support/observation.rs"]
mod observation;
#[path = "support/extrude_curve_subd.rs"]
mod subd_face_comparison;
#[path = "support/revolve_subd.rs"]
mod subd_profile_comparison;
#[path = "support/extrude_apex_subd.rs"]
mod topology;
use viboceros_command::CommandRegistry;
use viboceros_document::Geometry;
use viboceros_geometry::Tolerance;
use viboceros_oracle::extrude_apex_subd::{Fixture, geometry_record, setup};

#[test]
fn apex_subds_and_fallback_breps_roundtrip_native_geometry_and_normal_meshes() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in [
        "extrude_apex_subd_pilot",
        "extrude_apex_subd_holdouts",
        "extrude_apex_subd_confirmations",
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
            .filter(|o| o["op"] == "extrude_apex_subd")
        {
            count += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let (mut doc, source) = setup(&f, Tolerance::DEFAULT).unwrap();
            let commands = CommandRegistry::with_builtins();
            commands
                .execute(
                    &mut doc,
                    if f.crease_splitting {
                        "CreaseSplitting Enable"
                    } else {
                        "CreaseSplitting Disable"
                    },
                )
                .unwrap();
            commands.execute(&mut doc, &f.input()).unwrap();
            let object = doc.objects().find(|o| o.id() != source).unwrap();
            let id = object.id();
            let original = object.geometry().clone();
            let path = directory.path().join("apex.3dm");
            commands
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let model = viboceros_io::read_3dm_file(&path, doc.tolerance()).unwrap();
            assert_eq!(model.unsupported_object_count(), 0);
            let object = model.objects.iter().find(|o| o.id == id.uuid()).unwrap();
            let restored = match &object.geometry {
                viboceros_io::ThreeDmGeometry::SubD(s) => Geometry::SubD(s.clone()),
                viboceros_io::ThreeDmGeometry::Brep(b) => Geometry::Brep(b.clone()),
                _ => panic!("apex geometry lost in 3DM"),
            };
            let row = native["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == op["id"])
                .unwrap();
            let mut expected = row["value"]["after"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .clone();
            expected.as_object_mut().unwrap().remove("source");
            expected.as_object_mut().unwrap().remove("selected");
            topology::compare(
                &geometry_record(&restored, f.record_brep).unwrap(),
                &expected,
                &format!("{}/3dm", op["id"]),
                1e-12,
            );
            for geometry in [&original, &restored] {
                match geometry {
                    Geometry::SubD(s) => {
                        assert!(s.tessellate_with_normals(2).is_ok());
                    }
                    Geometry::Brep(b) => {
                        assert!(b.tessellate_with_normals(8, doc.tolerance()).is_ok());
                    }
                    _ => unreachable!(),
                }
            }
            if let (Geometry::SubD(a), Geometry::SubD(b)) = (&original, &restored) {
                assert_eq!(a.control_points(), b.control_points());
                assert_eq!(a.faces(), b.faces());
                assert_eq!(a.vertex_tags(), b.vertex_tags());
                assert_eq!(a.edge_tags(), b.edge_tags());
            }
        }
    }
    assert_eq!(count, 60);
}
