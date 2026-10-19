//! Replay actual public commands from shared native 3dm sources.
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Brep, Point3, Real, Tolerance};

fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn recorded_faces_keep_topology_geometry_explicit_charts_and_one_step_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reparameterize_faces.json"
    ))
    .unwrap();
    check_capture(
        &capture,
        "reparameterize_faces",
        Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap(),
    );
}

#[test]
fn automatic_domains_match_curved_rational_multispan_chart_and_tolerance_controls() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    for (label, epsilon) in [("main", 1e-8), ("tight", 1e-12), ("loose", 1e-5)] {
        let capture: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "observations/reparameterize_automatic_{label}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        check_capture(
            &capture,
            "reparameterize_automatic",
            Tolerance::try_new(epsilon, 1e-12, 1e-8).unwrap(),
        );
    }
}

fn check_capture(capture: &Value, folder: &str, tolerance: Tolerance) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = CommandRegistry::with_builtins();
    for row in capture["results"].as_array().unwrap() {
        let key = row["id"].as_str().unwrap();
        let v = &row["value"];
        assert_eq!(v["success"], true, "{key}");
        let file = root.join(format!("tools/rhino_oracle/fixtures/{folder}/{key}.3dm"));
        let model = viboceros_io::read_3dm_file(file, tolerance).unwrap();
        assert_eq!(model.unsupported_object_count(), 0, "{key}");
        assert_eq!(model.objects.len(), 1);
        let viboceros_io::ThreeDmGeometry::Brep(brep) = &model.objects[0].geometry else {
            panic!("{key}")
        };
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(Geometry::Brep(brep.clone())).unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut doc, "SetObjectName UVsource")
            .unwrap();
        registry.execute(&mut doc, "Group UVgroup").unwrap();
        doc.set_object_geometry_user_text([id], "GeometryCode", Some("retained"))
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.object(id).unwrap().clone();
        let face = v["face"].as_u64().unwrap() as usize;
        let automatic = key.ends_with("automatic");
        let args = if automatic {
            "Automatic".to_string()
        } else {
            let d = &v["domains"];
            format!("{},{},{},{}", d[0][0], d[0][1], d[1][0], d[1][1])
        };
        // Exercise selection for single faces and explicit references for
        // polysurfaces, even with no whole-object preselection.
        if brep.faces().len() > 1 {
            doc.clear_selection();
        }
        registry
            .execute(
                &mut doc,
                &format!("Reparameterize Surface={id} Face={face} {args}"),
            )
            .unwrap_or_else(|e| panic!("{key}: {e}"));
        let after = doc.object(id).unwrap().clone();
        let Geometry::Brep(result) = after.geometry() else {
            panic!()
        };
        assert_eq!(result.vertices(), brep.vertices(), "{key}");
        assert_eq!(result.edges(), brep.edges(), "{key}");
        assert_eq!(after.attributes(), before.attributes());
        assert_eq!(after.group_ids(), before.group_ids());
        assert_eq!(after.geometry_user_text(), before.geometry_user_text());
        let expected = &v["after"]["geometry"]["geometry"];
        let topology = &expected["topology"];
        assert_eq!(
            result.vertices().len(),
            topology["vertices"].as_u64().unwrap() as usize
        );
        assert_eq!(
            result.edges().len(),
            topology["edges"].as_array().unwrap().len()
        );
        assert_eq!(result.is_solid(), topology["solid"].as_bool().unwrap());
        let mut domain_gap = 0_f64;
        for (i, (actual, reference)) in result
            .faces()
            .iter()
            .zip(expected["faces"].as_array().unwrap())
            .enumerate()
        {
            let s = actual.surface();
            let definition = &reference["definition"];
            assert_eq!(
                actual.is_reversed(),
                topology["faces"][i]["reversed"].as_bool().unwrap()
            );
            assert_eq!(
                actual.loops().len(),
                reference["loops"].as_array().unwrap().len()
            );
            for (axis, domain) in [s.domain_u(), s.domain_v()].iter().enumerate() {
                let d = &definition[if axis == 0 { "domain_u" } else { "domain_v" }];
                domain_gap = domain_gap
                    .max((domain.start() - d[0].as_f64().unwrap()).abs())
                    .max((domain.end() - d[1].as_f64().unwrap()).abs());
            }
            // Check the surface image independently of the literal domains.
            for (j, p) in reference["samples"].as_array().unwrap().iter().enumerate() {
                let a = s
                    .evaluate(
                        s.parameter_at_u((j % 9) as Real / 8.).unwrap(),
                        s.parameter_at_v((j / 9) as Real / 8.).unwrap(),
                    )
                    .unwrap();
                assert!(
                    a.distance_to(point(p)).unwrap() <= tolerance.absolute(),
                    "{key}: surface sample {j}"
                );
            }
            if i != face {
                assert_eq!(actual, &brep.faces()[i]);
            }
            for (a, e) in actual
                .loops()
                .iter()
                .zip(reference["loops"].as_array().unwrap())
            {
                assert_eq!(a.trims().len(), e.as_array().unwrap().len());
                for (a, e) in a.trims().iter().zip(e.as_array().unwrap()) {
                    for (j, p) in e["lifted"].as_array().unwrap().iter().enumerate() {
                        let uv = a
                            .curve()
                            .evaluate(a.curve().parameter_at(j as Real / 32.).unwrap())
                            .unwrap();
                        assert!(
                            s.evaluate(uv.x(), uv.y())
                                .unwrap()
                                .distance_to(point(p))
                                .unwrap()
                                <= tolerance.absolute(),
                            "{key}: trim sample {j}"
                        );
                    }
                }
            }
        }
        assert!(
            domain_gap <= tolerance.absolute().min(1e-8),
            "{key}: domain error {domain_gap}"
        );
        if automatic {
            let baseline = brep.faces()[face].surface().estimated_size().unwrap();
            let definition = &expected["faces"][face]["definition"];
            let baseline_error = (baseline[0] - definition["domain_u"][1].as_f64().unwrap())
                .abs()
                .max((baseline[1] - definition["domain_v"][1].as_f64().unwrap()).abs());
            let surface = result.faces()[face].surface();
            eprintln!(
                "AUTOMATIC_DOMAIN_AUDIT {}",
                serde_json::json!({
                    "fixture": folder, "id": key, "model_epsilon": tolerance.absolute(),
                    "domain_epsilon": tolerance.absolute().min(1e-8),
                    "candidate_domains": [surface.domain_u(),surface.domain_v()],
                    "reference_domains": [definition["domain_u"].clone(),definition["domain_v"].clone()],
                    "candidate_error": domain_gap,
                    "baseline_control_polygon_size": baseline,
                    "baseline_policy_error": baseline_error
                })
            );
        }
        if before.geometry() != after.geometry() {
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.object(id).unwrap(), &before);
            assert!(!doc.can_undo());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.object(id).unwrap(), &after);
        }
        // The native interchange path retains all edited charts and trims.
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("edited.3dm");
        registry
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let restored = viboceros_io::read_3dm_file(path, tolerance).unwrap();
        let viboceros_io::ThreeDmGeometry::Brep(restored) = &restored.objects[0].geometry else {
            panic!()
        };
        check_roundtrip(restored, result, tolerance);
        if automatic && brep.faces().len() == 1 {
            let mut raw_doc = Document::new(tolerance);
            let raw_id = raw_doc
                .add_geometry(Geometry::NurbsSurface(brep.faces()[0].surface().clone()))
                .unwrap();
            raw_doc
                .select_object(raw_id, SelectionMode::Replace)
                .unwrap();
            registry
                .execute(&mut raw_doc, "Reparameterize Automatic")
                .unwrap();
            let Geometry::NurbsSurface(surface) = raw_doc.object(raw_id).unwrap().geometry() else {
                panic!()
            };
            assert_eq!(
                surface,
                result.faces()[0].surface(),
                "{key}: raw surface and face sizing"
            );
        }
    }
}

fn check_roundtrip(actual: &Brep, expected: &Brep, tolerance: Tolerance) {
    assert_eq!(actual.faces().len(), expected.faces().len());
    assert_eq!(actual.vertices(), expected.vertices());
    assert_eq!(actual.edges().len(), expected.edges().len());
    for (a, e) in actual.edges().iter().zip(expected.edges()) {
        assert_eq!(a.vertices(), e.vertices());
        assert_eq!(a.tolerance(), e.tolerance());
        assert_eq!(a.curve().domain(), e.curve().domain());
        for j in 0..=32 {
            let u = j as Real / 32.;
            assert!(
                a.curve()
                    .evaluate(a.curve().parameter_at(u).unwrap())
                    .unwrap()
                    .distance_to(
                        e.curve()
                            .evaluate(e.curve().parameter_at(u).unwrap())
                            .unwrap()
                    )
                    .unwrap()
                    <= tolerance.absolute()
            );
        }
    }
    for (a, e) in actual.faces().iter().zip(expected.faces()) {
        assert_eq!(a.is_reversed(), e.is_reversed());
        assert_eq!(a.surface(), e.surface());
        assert_eq!(a.loops().len(), e.loops().len());
        let surface = e.surface();
        for (a, e) in a.loops().iter().zip(e.loops()) {
            assert_eq!(a.loop_type(), e.loop_type());
            assert_eq!(a.trims().len(), e.trims().len());
            for (a, e) in a.trims().iter().zip(e.trims()) {
                assert_eq!(a.vertices(), e.vertices());
                assert_eq!(a.edge(), e.edge());
                assert_eq!(a.is_reversed_3d(), e.is_reversed_3d());
                assert_eq!(a.trim_type(), e.trim_type());
                assert_eq!(a.iso(), e.iso());
                assert_eq!(a.tolerance(), e.tolerance());
                assert_eq!(a.curve().knots(), e.curve().knots());
                assert_eq!(a.curve().degree(), e.curve().degree());
                // Rational controls pass through homogeneous 3dm coefficients.
                // Bound the coefficient change in the original chart units;
                // the ordinary read-side continuous spatial checks still run.
                assert_eq!(
                    a.curve().control_points().len(),
                    e.curve().control_points().len()
                );
                for (p, q) in a
                    .curve()
                    .control_points()
                    .iter()
                    .zip(e.curve().control_points())
                {
                    assert_eq!(p.weight(), q.weight());
                    for (x, y, domain) in [
                        (p.point().x(), q.point().x(), surface.domain_u()),
                        (p.point().y(), q.point().y(), surface.domain_v()),
                    ] {
                        assert!((x - y).abs() / (domain.end() - domain.start()) <= 1e-14);
                    }
                }
                for j in 0..=32 {
                    let t = j as Real / 32.;
                    let p = a
                        .curve()
                        .evaluate(a.curve().parameter_at(t).unwrap())
                        .unwrap();
                    let q = e
                        .curve()
                        .evaluate(e.curve().parameter_at(t).unwrap())
                        .unwrap();
                    assert!(
                        surface
                            .evaluate(p.x(), p.y())
                            .unwrap()
                            .distance_to(surface.evaluate(q.x(), q.y()).unwrap())
                            .unwrap()
                            <= tolerance.absolute()
                    );
                }
            }
        }
    }
}
