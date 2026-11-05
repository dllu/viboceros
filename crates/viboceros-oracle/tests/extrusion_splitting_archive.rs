#[path = "support/apex_splitting/cap_chart.rs"]
mod cap_charts;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/extrude_apex_subd.rs"]
mod subd_apex_comparison;
#[path = "support/extrude_curve_subd.rs"]
mod subd_face_comparison;
#[path = "support/revolve_subd.rs"]
mod subd_profile_comparison;
#[path = "support/extrusion_splitting.rs"]
mod topology;
use viboceros_command::CommandRegistry;
use viboceros_document::Geometry;
use viboceros_geometry::Tolerance;
use viboceros_oracle::extrusion_splitting::{Fixture, geometry_record, setup};

#[test]
fn straight_split_extrusions_roundtrip_native_geometry_and_normal_meshes() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in [
        "extrusion_splitting_pilot",
        "extrusion_splitting_holdouts",
        "extrusion_splitting_confirmations",
        "extrusion_splitting_cancellations",
        "extrusion_splitting_curved",
        "extrusion_splitting_mixed",
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
            .filter(|o| o["op"] == "extrusion_splitting")
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
            commands
                .execute(
                    &mut doc,
                    if f.use_extrusions {
                        "UseExtrusions Extrusion"
                    } else {
                        "UseExtrusions Polysurface"
                    },
                )
                .unwrap();
            commands.set_curve_extrusion_split_at_tangents(if f.seed_subd {
                false
            } else if f.seed_cancel {
                true
            } else {
                f.seed_split.unwrap_or(true)
            });
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
                viboceros_io::ThreeDmGeometry::Brep(b) => Geometry::Brep(b.clone()),
                viboceros_io::ThreeDmGeometry::NurbsSurface(s) => Geometry::NurbsSurface(s.clone()),
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
                &geometry_record(&restored, doc.tolerance()).unwrap(),
                &expected,
                &format!("{}/3dm", op["id"]),
                1e-12,
            );
            for geometry in [&original, &restored] {
                match geometry {
                    Geometry::Brep(b) => {
                        assert!(b.tessellate_with_normals(8, doc.tolerance()).is_ok());
                    }
                    Geometry::NurbsSurface(s) => {
                        assert!(s.tessellate(8, doc.tolerance()).is_ok());
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    assert_eq!(count, 163);
}
