#[path = "support/planar_regions.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
use serde_json::Value;

#[test]
fn open_planar_boundary_chains_match_native_regions_sources_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut unsupported = Vec::new();
    for (family, count) in [
        ("planar_chains", 35),
        ("planar_chains_holdouts", 27),
        ("planar_chains_contexts", 52),
        ("planar_chains_salvage", 8),
        ("planar_chains_filled", 6),
        ("planar_chains_modes", 8),
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        assert_eq!(request["operations"].as_array().unwrap().len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let q = serde_json::json!({"protocol_version":1,"iterations":1,"tolerance":request["tolerance"],"operations":[op]});
            let report = viboceros_oracle::run_request(&serde_json::from_value(q).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            let actual = serde_json::to_value(report).unwrap()["results"][0]["value"].clone();
            // The exact Boolean exporter rejects this narrow tangent sliver.
            // Keep the native success and our atomic decline visible as a gap.
            if op["id"] == "chains_filled_outside_tangent" {
                assert_eq!(row["value"]["success"], true);
                assert_eq!(actual["success"], false);
                assert_eq!(actual["before"], actual["after"]);
                assert_eq!(actual["before"], actual["undo"]);
                assert_eq!(actual["before"], actual["redo"]);
                unsupported.push(op["id"].as_str().unwrap().to_owned());
                continue;
            }
            match comparison::compare(&actual, &row["value"], 1e-9, op["id"].as_str().unwrap()) {
                Ok(gaps) => assert_eq!(gaps, 0),
                Err(e) => {
                    let f: viboceros_oracle::planar_regions::Fixture =
                        serde_json::from_value(op.clone()).unwrap();
                    let (mut doc, _) = f
                        .setup(viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap())
                        .unwrap();
                    let result = viboceros_command::CommandRegistry::with_builtins()
                        .execute(&mut doc, "PlanarSrf");
                    failures.push(format!("{} ({result:?}): {e}", op["id"]));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(unsupported, ["chains_filled_outside_tangent"]);
}

#[path = "support/planar_chains.rs"]
mod geometry;
#[test]
fn planar_chains_archive_boundaries_match_native_and_mesh_with_normals() {
    use viboceros_document::Geometry;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("chains.3dm");
    let mut maximum = 0f64;
    for (op, native) in geometry::rows(&base, observation::read) {
        let f: viboceros_oracle::planar_regions::Fixture =
            serde_json::from_value(op.clone()).unwrap();
        let (mut doc, _) = f.setup(tolerance).unwrap();
        let registry = viboceros_command::CommandRegistry::with_builtins();
        let result = registry.execute(&mut doc, "PlanarSrf");
        if op["id"] == "chains_filled_outside_tangent" {
            assert!(result.is_err());
            continue;
        }
        assert_eq!(
            result.is_ok(),
            native["success"] == true,
            "{}: {result:?}",
            op["id"]
        );
        let outputs = doc
            .objects()
            .filter_map(|o| match o.geometry() {
                Geometry::Brep(b) => Some(b),
                _ => None,
            })
            .collect::<Vec<_>>();
        maximum = maximum.max(geometry::assert_geometry(
            &outputs,
            &native["after"],
            tolerance,
            op["id"].as_str().unwrap(),
        ));
        if outputs.is_empty() {
            continue;
        }
        for b in outputs {
            b.tessellate_with_normals(8, tolerance)
                .unwrap_or_else(|e| panic!("{}: {e:?}", op["id"]));
        }
        registry
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, tolerance).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        let restored = model
            .objects
            .iter()
            .filter_map(|o| match &o.geometry {
                viboceros_io::ThreeDmGeometry::Brep(b) => Some(b),
                _ => None,
            })
            .collect::<Vec<_>>();
        maximum = maximum.max(geometry::assert_geometry(
            &restored,
            &native["after"],
            tolerance,
            op["id"].as_str().unwrap(),
        ));
        for b in restored {
            b.tessellate_with_normals(8, tolerance).unwrap();
        }
    }
    println!("PLANAR_CHAINS_BOUNDARY_MAX {maximum}");
}
