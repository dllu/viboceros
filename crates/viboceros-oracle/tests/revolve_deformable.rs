#[path = "support/observation.rs"]
mod observation;
#[path = "support/edge_analysis_mixed.rs"]
mod values;

#[test]
fn cubic_revolutions_match_native_geometry_selection_and_history() {
    compare_family("revolve_deformable_pilot", 4);
    compare_family("revolve_deformable", 51);
    compare_family("revolve_deformable_holdouts", 12);
    compare_family("revolve_deformable_domains", 18);
    compare_family("revolve_deformable_thresholds", 18);
    compare_family("revolve_deformable_scales", 26);
    compare_family("revolve_deformable_policy", 26);
    compare_family("revolve_deformable_upper", 17);
}

#[test]
fn deformable_revolutions_roundtrip_and_remain_meshable() {
    for family in [
        "revolve_deformable_pilot",
        "revolve_deformable",
        "revolve_deformable_holdouts",
        "revolve_deformable_domains",
        "revolve_deformable_thresholds",
        "revolve_deformable_scales",
        "revolve_deformable_policy",
        "revolve_deformable_upper",
    ] {
        roundtrip_family(family);
    }
}

fn roundtrip_family(family: &str) {
    use viboceros_command::CommandRegistry;
    use viboceros_document::Geometry;
    use viboceros_geometry::Tolerance;
    use viboceros_oracle::revolve_deformable::{Fixture, setup};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    for op in request["operations"].as_array().unwrap() {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let (mut doc, source) = setup(&f, Tolerance::DEFAULT).unwrap();
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, &f.input()).unwrap();
        let output = doc.objects().find(|o| o.id() != source).unwrap();
        let id = output.id();
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            panic!("NURBS surface required")
        };
        let original = surface.clone();
        assert!(original.tessellate(4, doc.tolerance()).is_ok());
        let path = temp.path().join("revolve.3dm");
        commands
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, doc.tolerance()).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        let restored = model.objects.iter().find(|o| o.id == id.uuid()).unwrap();
        let viboceros_io::ThreeDmGeometry::NurbsSurface(surface) = &restored.geometry else {
            panic!("NURBS surface lost in 3DM")
        };
        assert_eq!(surface.degree_u(), original.degree_u());
        assert_eq!(surface.degree_v(), original.degree_v());
        assert_eq!(
            surface.control_points().len(),
            original.control_points().len()
        );
        for (a, b) in surface
            .control_points()
            .iter()
            .zip(original.control_points())
        {
            assert_eq!(a.weight(), b.weight());
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
        }
        assert_eq!(
            &surface.knots_u()[1..surface.knots_u().len() - 1],
            &original.knots_u()[1..original.knots_u().len() - 1]
        );
        assert_eq!(
            &surface.knots_v()[1..surface.knots_v().len() - 1],
            &original.knots_v()[1..original.knots_v().len() - 1]
        );
        for i in 0..=8 {
            for j in 0..=8 {
                let u = original.parameter_at_u(i as f64 / 8.).unwrap();
                let v = original.parameter_at_v(j as f64 / 8.).unwrap();
                assert!(
                    surface
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(original.evaluate(u, v).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
        assert!(surface.tessellate(4, doc.tolerance()).is_ok());
    }
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
    }
}
