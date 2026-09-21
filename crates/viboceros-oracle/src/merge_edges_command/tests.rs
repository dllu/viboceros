use super::*;

#[test]
fn partition_kernel_replays_retained_face_split_geometry() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/merge_edges_face_splits.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/merge_edges_face_splits.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 107);
    assert_eq!(expected["results"].as_array().unwrap().len(), 107);
    let mut cutoff_differences = 0;
    for (op, row) in request
        .operations
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let Operation::MergeEdgesCommand { fixture, .. } = op else {
            panic!()
        };
        let Source::Brep { brep } = &fixture.sources[0] else {
            panic!()
        };
        let tolerance =
            Tolerance::try_new(1e-9, 1e-12, fixture.angular_tolerance.unwrap()).unwrap();
        let source = brep.build(Tolerance::DEFAULT).unwrap();
        let cleaned = source
            .try_cleanup_edges(
                tolerance
                    .angular()
                    .clamp(0.1_f64.to_radians(), 1_f64.to_radians()),
                tolerance,
            )
            .unwrap();
        let partitioned = cleaned
            .try_split_kinky_faces(
                tolerance
                    .angular()
                    .clamp(0.1_f64.to_radians(), 2_f64.to_radians()),
                tolerance,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", row["id"]))
            .unwrap_or(cleaned);
        let actual = crate::brep_join::geometry_record(&partitioned, Tolerance::DEFAULT).unwrap();
        let observed = row["value"]["after"][0]["geometry"]["brep"].clone();
        if ["crease-2-doc0.0872665", "upper-2-doc2.5", "upper-2-doc10"]
            .contains(&row["id"].as_str().unwrap())
        {
            cutoff_differences += 1;
            assert_eq!(partitioned.faces().len(), 1);
            assert_eq!(partitioned.edges().len(), 5);
            assert_eq!(observed["faces"].as_array().unwrap().len(), 2);
            assert_eq!(observed["edges"].as_array().unwrap().len(), 7);
            let surface = source.faces()[0].surface();
            let profile = surface.isocurve_u(*surface.domain_v().start()).unwrap();
            let a = profile
                .tangent_at_on_side(1., viboceros_geometry::ParameterSide::Left)
                .unwrap();
            let b = profile
                .tangent_at_on_side(1., viboceros_geometry::ParameterSide::Right)
                .unwrap();
            let angle = a
                .as_vector()
                .cross(b.as_vector())
                .unwrap()
                .length()
                .unwrap()
                .atan2(a.as_vector().dot(b.as_vector()).unwrap());
            let cutoff = 2_f64.to_radians();
            assert!(angle <= cutoff && cutoff - angle < 1e-14);
            // Stable atan2 does not exceed the requested cutoff here; Rhino's
            // cosine comparison splits. Keep the raw disagreement explicit.
            continue;
        }
        // Compare the complete raw geometry record, including face/trim order
        // and all native parameter intervals; no representation normalization.
        close(&actual, &observed, row["id"].as_str().unwrap());
    }
    assert_eq!(cutoff_differences, 3);
}

fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-9_f64.max(1e-10 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                close(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

// Only the recorded representation differences are excluded. Every spatial
// curve definition, UV control, history state, and other raw field is retained.
fn representation_limits(mut value: Value, id: &str) -> Value {
    for key in ["before", "after", "undo", "redo"] {
        let Some(objects) = value.get_mut(key).and_then(Value::as_array_mut) else {
            continue;
        };
        for object in objects {
            let Some(brep) = object["geometry"].get_mut("brep") else {
                continue;
            };
            if id.starts_with("unclamped-") {
                // 3DM omits these two phantom knots, already before cleanup.
                let knots = brep["surfaces"][0]["knots_u"].as_array_mut().unwrap();
                knots.pop();
                knots.remove(0);
            }
            if (id.starts_with("box-") && matches!(key, "after" | "redo"))
                || (id.starts_with("surface-") && matches!(key, "before" | "undo"))
            {
                for face in brep["trim_curves"].as_array_mut().unwrap() {
                    for ring in face.as_array_mut().unwrap() {
                        for trim in ring.as_array_mut().unwrap() {
                            // Exactly two unit-weight controls prove a segment;
                            // only its affine parameter interval is excluded.
                            assert_eq!(trim["degree"], 1);
                            let controls = trim["control_points"].as_array().unwrap();
                            assert_eq!(controls.len(), 2);
                            assert!(controls.iter().all(|cp| cp["weight"] == 1.0));
                            trim.as_object_mut().unwrap().remove("domain");
                            trim.as_object_mut().unwrap().remove("knots");
                        }
                    }
                }
            }
        }
    }
    value
}

#[test]
fn command_replays_full_geometry_and_history_with_only_known_representation_limits() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/merge_edges_command.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/merge_edges_command.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 39);
    assert_eq!(expected["results"].as_array().unwrap().len(), 39);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(
            &representation_limits(a.value.clone(), &a.id),
            &representation_limits(b["value"].clone(), &a.id),
            &a.id,
        );
    }
}

#[test]
fn invalid_command_fixtures_fail_before_source_export() {
    let artifact = OracleTemporaryFile::new("invalid-edge-merge");
    let source = json!({"brep":{"source":{"type":"box","min":[0,0,0],"max":[1,1,1]},"artifact_path":artifact.path}});
    for update in [
        json!({"selected":[]}),
        json!({"selected":[0,0]}),
        json!({"selected":[1]}),
        json!({"preselect":true,"cancel":true}),
        json!({"angular_tolerance":0}),
        json!({"absolute_tolerance":-1}),
    ] {
        let mut fixture = json!({"sources":[source.clone()]});
        fixture
            .as_object_mut()
            .unwrap()
            .extend(update.as_object().unwrap().clone());
        let fixture: MergeEdgesFixture = serde_json::from_value(fixture).unwrap();
        assert!(matches!(
            run(&fixture, Tolerance::DEFAULT),
            Err(ProbeError::FixtureInvariant(_) | ProbeError::Geometry(_))
        ));
        assert!(!artifact.path.exists());
    }
}

#[test]
fn planar_angle_matrix_matches_raw_records_except_four_exact_cutoff_roundoffs() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/merge_edges_angles.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/merge_edges_angles.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let boundary_cases = [
        "planar-kink-1-doc-0.0174533",
        "planar-kink-1-doc-0.0872665",
        "planar-kink-0.2-doc-0.00349066",
        "planar-kink-0.5-doc-0.00872665",
    ];
    assert_eq!(actual.results.len(), 85);
    let mut cutoffs = 0;
    for ((a, b), op) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
        .zip(&request.operations)
    {
        assert_eq!(a.id, b["id"]);
        if !boundary_cases.contains(&a.id.as_str()) {
            close(&a.value, &b["value"], &a.id);
            continue;
        }
        cutoffs += 1;
        // No edge merges, but the first straight edge is reparameterized.
        // Check every other field, retaining its complete control definition.
        let mut before = a.value["before"].clone();
        let mut after = a.value["after"].clone();
        for state in [&mut before, &mut after] {
            let brep = &mut state[0]["geometry"]["brep"];
            let curve = &mut brep["edges"][0]["curve"];
            assert_eq!(curve["definition"]["degree"], 1);
            assert_eq!(
                curve["definition"]["control_points"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            curve.as_object_mut().unwrap().remove("domain");
            let definition = curve["definition"].as_object_mut().unwrap();
            definition.remove("domain");
            definition.remove("knots");
            let trim = &mut brep["trim_curves"][0][0][0];
            assert_eq!(trim["degree"], 1);
            assert_eq!(trim["control_points"].as_array().unwrap().len(), 2);
            trim.as_object_mut().unwrap().remove("domain");
            trim.as_object_mut().unwrap().remove("knots");
        }
        close(&before, &after, &a.id);
        close(&a.value["before"], &b["value"]["before"], &a.id);
        assert_eq!(
            a.value["after"][0]["geometry"]["brep"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            b["value"]["after"][0]["geometry"]["brep"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let Operation::MergeEdgesCommand { fixture, .. } = op else {
            panic!()
        };
        let Source::Brep { brep } = &fixture.sources[0] else {
            panic!()
        };
        let brep = brep.build(Tolerance::DEFAULT).unwrap();
        let a = brep.edges()[0].curve();
        let b = brep.edges()[1].curve();
        let ta = a
            .tangent_at_on_side(*a.domain().end(), viboceros_geometry::ParameterSide::Left)
            .unwrap();
        let tb = b
            .tangent_at_on_side(
                *b.domain().start(),
                viboceros_geometry::ParameterSide::Right,
            )
            .unwrap();
        let angle = ta.as_vector().angle_to(tb.as_vector()).unwrap();
        let limit = fixture
            .angular_tolerance
            .unwrap()
            .clamp(0.1_f64.to_radians(), 1_f64.to_radians());
        assert!(
            angle > limit && angle - limit < 1e-14,
            "{}: {angle} vs {limit}",
            op.id()
        );
    }
    assert_eq!(cutoffs, 4);
}

#[test]
fn kinky_surface_replacement_remains_an_explicit_topology_difference() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/merge_edges_kinky_surfaces.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/merge_edges_kinky_surfaces.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 64);
    let mut split_surfaces = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value["before"], &b["value"]["before"], &a.id);
        let x = &a.value["after"][0]["geometry"]["brep"];
        let y = &b["value"]["after"][0]["geometry"]["brep"];
        assert_eq!(x["faces"].as_array().unwrap().len(), 1);
        if y["faces"].as_array().unwrap().len() == 2 {
            split_surfaces += 1;
            assert_eq!(x["edges"].as_array().unwrap().len(), 5);
            assert_eq!(y["edges"].as_array().unwrap().len(), 7);
        } else {
            close(&a.value, &b["value"], &a.id);
        }
    }
    assert_eq!(split_surfaces, 33);
}

#[test]
fn straight_edge_representations_and_undo_match_with_conservative_uncertainty() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/merge_edges_linear.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/merge_edges_linear.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 18);
    assert_eq!(expected["results"].as_array().unwrap().len(), 18);
    let mut approximate = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value, &b["value"], &a.id);
        assert_eq!(a.value["history_tested"], true);
        assert_eq!(a.value["before"], a.value["undo"]);
        assert_eq!(a.value["after"], a.value["redo"]);
        let before = &a.value["before"][0]["geometry"]["brep"];
        let after = &a.value["after"][0]["geometry"]["brep"];
        assert_eq!(before["surfaces"], after["surfaces"]);
        assert_eq!(after["edges"].as_array().unwrap().len(), 4);
        let near = ["near-1e-12-", "near-1e-10-", "near-1e-09-"]
            .iter()
            .any(|p| a.id.starts_with(p));
        if near {
            approximate += 1;
        }
        // The raw epsilon comparison alone would hide this meaningful
        // metadata difference: native never reports zero approximation error.
        assert_eq!(
            after["edge_tolerances"],
            if near {
                json!([1e-9, 0., 1e-9, 0.])
            } else {
                json!([0., 0., 0., 0.])
            }
        );
        assert_eq!(
            b["value"]["after"][0]["geometry"]["brep"]["edge_tolerances"],
            json!([0., 0., 0., 0.])
        );
        let curved = a.id.starts_with("near-1e-08-") || a.id.starts_with("near-1e-06-");
        for (index, edge) in after["edges"].as_array().unwrap().iter().enumerate() {
            let degree = if curved && index % 2 == 0 { 2 } else { 1 };
            assert_eq!(edge["curve"]["definition"]["degree"], degree);
            assert_eq!(
                edge["curve"]["definition"]["control_points"]
                    .as_array()
                    .unwrap()
                    .len(),
                degree + 1
            );
        }
    }
    assert_eq!(approximate, 6);
}
