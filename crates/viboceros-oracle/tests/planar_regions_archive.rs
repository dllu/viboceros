#[path = "support/planar_regions.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::Tolerance;
use viboceros_oracle::planar_regions::{Fixture, snapshot};
#[test]
fn planar_regions_archives_preserve_boundaries_area_evidence_and_normal_meshes() {
    family("planar_regions_holdouts", 69);
    family("planar_regions_confirmations", 32);
    family("planar_regions_planes", 8);
    family("planar_crossings", 48);
    family("planar_crossings_details", 8);
    family("planar_crossings_holdouts", 33);
}
fn family(family: &str, expected_count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, family);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("regions.3dm");
    let mut count = 0;
    let mut area_gaps = 0;
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        count += 1;
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
        let (mut doc, ids) = f.setup(tol).unwrap();
        let sources = doc
            .objects()
            .map(|o| {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!("source")
                };
                c.clone()
            })
            .collect::<Vec<_>>();
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, "PlanarSrf").unwrap();
        commands
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, tol).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        let mut expected = row["value"]["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["kind"] == "brep")
            .cloned()
            .collect::<Vec<_>>();
        for output in doc.objects().filter(|o| !ids.contains(&o.id())) {
            let saved = model
                .objects
                .iter()
                .find(|o| o.id == output.id().uuid())
                .unwrap();
            let viboceros_io::ThreeDmGeometry::Brep(restored) = &saved.geometry else {
                panic!("planar B-rep lost in archive")
            };
            for b in [
                match output.geometry() {
                    Geometry::Brep(b) => b,
                    _ => unreachable!(),
                },
                restored,
            ] {
                b.tessellate_with_normals(8, tol)
                    .unwrap_or_else(|e| panic!("{}: {e:?}", op["id"]));
            }
            let mut restored_doc = Document::new(tol);
            restored_doc
                .add_geometry(Geometry::Brep(restored.clone()))
                .unwrap();
            let actual = snapshot(&restored_doc, &[], &sources).unwrap()[0].clone();
            let matches = expected
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    comparison::compare(&actual, e, 1e-9, op["id"].as_str().unwrap())
                        .ok()
                        .map(|gaps| (i, gaps))
                })
                .collect::<Vec<_>>();
            assert!(!matches.is_empty(), "{}: {actual}", op["id"]);
            area_gaps += matches[0].1;
            expected.remove(matches[0].0);
        }
        assert!(expected.is_empty());
    }
    assert_eq!(count, expected_count);
    assert_eq!(
        area_gaps,
        usize::from(family == "planar_crossings_holdouts")
    );
}
