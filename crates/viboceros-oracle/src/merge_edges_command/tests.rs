use super::*;

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

// A deliberately narrower comparison than the full raw replay report: Rhino
// reparameterizes line edges/trims and normalizes constant line weights. Keep
// the sampled loci, topology, integrals, surface controls, and document state.
fn witnesses(mut value: Value) -> Value {
    for key in ["before", "after", "undo", "redo"] {
        let Some(objects) = value.get_mut(key).and_then(Value::as_array_mut) else {
            continue;
        };
        for object in objects {
            let Some(brep) = object["geometry"]
                .get_mut("brep")
                .and_then(Value::as_object_mut)
            else {
                continue;
            };
            for edge in brep["edges"].as_array_mut().unwrap() {
                let curve = edge["curve"].as_object_mut().unwrap();
                curve.remove("definition");
                curve.remove("domain");
            }
            for face in brep["trim_curves"].as_array_mut().unwrap() {
                for ring in face.as_array_mut().unwrap() {
                    for trim in ring.as_array_mut().unwrap() {
                        let trim = trim.as_object_mut().unwrap();
                        // Rhino also removes collinear interior UV controls.
                        // Only collapse unit-weight, degree-one, axis-aligned
                        // monotone polygons: this proves the same entire line
                        // locus, not merely matching endpoints or samples.
                        if trim["degree"] == 1 {
                            let controls = trim["control_points"].as_array().unwrap();
                            let first = controls.first().unwrap();
                            let last = controls.last().unwrap();
                            let line = (0..2).any(|axis| {
                                let varying = 1 - axis;
                                let a = first["point"][varying].as_f64().unwrap();
                                let b = last["point"][varying].as_f64().unwrap();
                                a != b
                                    && controls.iter().all(|cp| {
                                        cp["weight"] == 1.0
                                            && cp["point"][axis] == first["point"][axis]
                                    })
                                    && controls.windows(2).all(|p| {
                                        let x = p[0]["point"][varying].as_f64().unwrap();
                                        let y = p[1]["point"][varying].as_f64().unwrap();
                                        if b > a { x <= y } else { x >= y }
                                    })
                            });
                            if line {
                                trim.insert("control_points".into(), json!([first, last]));
                            }
                        }
                        trim.remove("domain");
                        trim.remove("knots");
                    }
                }
            }
            // 3DM omits the two phantom knots outside the usable surface
            // domain. Preserve every other surface field and knot.
            for surface in brep["surfaces"].as_array_mut().unwrap() {
                for key in ["knots_u", "knots_v"] {
                    let knots = surface[key].as_array_mut().unwrap();
                    knots.pop();
                    knots.remove(0);
                }
            }
        }
    }
    value
}

#[test]
fn command_replays_document_history_topology_and_geometric_witnesses() {
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
    let mut noops = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        let id = &a.id;
        let a = witnesses(a.value.clone());
        let mut b = witnesses(b["value"].clone());
        if a["history_tested"] == false && b["history_tested"] == true {
            // Native no-ops preserve the redo branch. Rhino replaces these
            // unchanged objects anyway. This difference is retained in raw data.
            noops += 1;
            close(
                &a["before"][0]["geometry"],
                &a["after"][0]["geometry"],
                "native-noop",
            );
            close(
                &b["before"][0]["geometry"],
                &b["after"][0]["geometry"],
                "rhino-noop",
            );
            b["history_tested"] = json!(false);
            b.as_object_mut().unwrap().remove("undo");
            b.as_object_mut().unwrap().remove("redo");
        }
        close(&a, &b, id);
    }
    assert_eq!(noops, 4);
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
        assert_eq!(a.value["before"], a.value["after"]);
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
            close(
                &witnesses(a.value.clone()),
                &witnesses(b["value"].clone()),
                &a.id,
            );
        }
    }
    assert_eq!(split_surfaces, 33);
}
