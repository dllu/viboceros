#[path = "support/observation.rs"]
mod observation;
#[path = "support/revolve_splitting.rs"]
mod topology;
#[path = "support/edge_analysis_mixed.rs"]
mod values;

#[test]
fn legacy_exact_revolutions_keep_recorded_native_charts() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    for (family, count) in [
        ("curve_native_revolution", 4),
        ("curve_modeling_short_spans", 1),
    ] {
        let mut query: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        query["operations"]
            .as_array_mut()
            .unwrap()
            .retain(|op| op["op"] == "curve_revolve_command");
        let request = serde_json::from_value(query).unwrap();
        let actual =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let native = observation::read(&base, family);
        assert_eq!(actual["results"].as_array().unwrap().len(), count);
        for row in actual["results"].as_array().unwrap() {
            let expected = native["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == row["id"])
                .unwrap();
            values::compare(&row["value"], &expected["value"], &row["id"].to_string());
        }
    }
}

#[test]
fn revolve_split_policies_match_native_surfaces_topology_selection_and_history() {
    compare_family("revolve_splitting", 48);
    compare_family("revolve_splitting_holdouts", 80);
    compare_family("revolve_splitting_mixed", 32);
}

fn compare_family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    let native = observation::read(&base, family);
    assert_eq!(actual["results"].as_array().unwrap().len(), count);
    assert_eq!(native["results"].as_array().unwrap().len(), count);
    for (a, b) in actual["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], b["id"]);
        for field in ["before", "after", "undo", "redo"] {
            values::compare(
                &a["value"][field],
                &b["value"][field],
                &format!("{}/{field}", a["id"]),
            );
        }
        assert_eq!(
            a["value"]["adjacency"],
            topology::adjacency(&b["value"]["full"]),
            "{}",
            a["id"]
        );
        topology::compare_boundaries(
            &a["value"]["boundaries"],
            &b["value"]["full"],
            &a["id"].to_string(),
        );
    }
}

#[test]
fn revolve_splitting_roundtrips_preserve_native_faces_and_remain_meshable() {
    use viboceros_command::CommandRegistry;
    use viboceros_document::Geometry;
    use viboceros_geometry::{Brep, Tolerance};
    use viboceros_oracle::revolve_splitting::{Fixture, geometry_record};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let temp = tempfile::tempdir().unwrap();
    let mut total = 0;
    for family in [
        "revolve_splitting",
        "revolve_splitting_holdouts",
        "revolve_splitting_mixed",
    ] {
        let request: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            total += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let tolerance = Tolerance::DEFAULT;
            let (mut doc, source) =
                viboceros_oracle::revolve_deformable::setup(&f.profile, tolerance).unwrap();
            let commands = CommandRegistry::with_builtins();
            f.configure(&commands, &mut doc).unwrap();
            commands.execute(&mut doc, &f.input()).unwrap();
            let output = doc.objects().find(|o| o.id() != source).unwrap();
            let id = output.id();
            let meshable = |g: &Geometry| {
                let b = match g {
                    Geometry::Brep(b) => b.clone(),
                    Geometry::NurbsSurface(s) => {
                        Brep::try_surface_face(s.clone(), tolerance).unwrap()
                    }
                    _ => panic!("surface required"),
                };
                assert!(
                    b.tessellate_with_normals(4, tolerance).is_ok(),
                    "{}",
                    op["id"]
                );
            };
            meshable(output.geometry());
            let path = temp.path().join("revolve.3dm");
            commands
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let model = viboceros_io::read_3dm_file(&path, tolerance).unwrap();
            assert_eq!(model.unsupported_object_count(), 0);
            let restored = model.objects.iter().find(|o| o.id == id.uuid()).unwrap();
            let geometry = match &restored.geometry {
                viboceros_io::ThreeDmGeometry::Brep(b) => Geometry::Brep(b.clone()),
                viboceros_io::ThreeDmGeometry::NurbsSurface(s) => Geometry::NurbsSurface(s.clone()),
                _ => panic!("surface lost in 3DM"),
            };
            values::compare(
                &geometry_record(&geometry, tolerance).unwrap(),
                &row["value"]["after"].as_array().unwrap().last().unwrap()["brep"],
                &format!("{}/3dm", op["id"]),
            );
            meshable(&geometry);
            let mut restored_doc = viboceros_document::Document::new(tolerance);
            restored_doc.add_geometry(geometry).unwrap();
            assert_eq!(
                viboceros_oracle::revolve_splitting::adjacency(&restored_doc, source).unwrap(),
                topology::adjacency(&row["value"]["full"]),
                "{}/3dm incidence",
                op["id"]
            );
        }
    }
    assert_eq!(total, 160);
}
