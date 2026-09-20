use super::*;

fn request() -> ProbeRequest {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/borders.json"
    ))
    .unwrap()
}

fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10_f64.max(1e-10 * a.abs().max(b.abs())),
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

#[test]
fn borders_replay_rhino_definitions_domains_and_document_state() {
    let request = request();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/borders.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 50);
    assert_eq!(observed["results"].as_array().unwrap().len(), 50);
    let mut parity = 0;
    let mut diagnostics = 0;
    for ((operation, result), reference) in request
        .operations
        .iter()
        .zip(&actual.results)
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(result.id, reference["id"]);
        let diagnostic = [
            "DupBorder-shell-",
            "DupBorder-panels-",
            "DupBorder-opposite-walls-",
            "DupBorder-adjacent-walls-",
        ]
        .iter()
        .any(|prefix| result.id.starts_with(prefix));
        if !diagnostic {
            close(&result.value, &reference["value"], &result.id);
            parity += 1;
            continue;
        }
        diagnostics += 1;
        // Do not silently normalize or bless different native parameters. The
        // raw report remains failing. Separately verify every recorded point
        // lies on a true naked edge and both engines cover the full perimeter.
        let Operation::BorderCommand { fixture, .. } = operation else {
            panic!("border fixture")
        };
        let Geometry::Brep(source) = fixture.source.geometry(Tolerance::DEFAULT).unwrap() else {
            panic!("B-rep")
        };
        let counts = source.edge_use_counts();
        let edges = source
            .edges()
            .iter()
            .zip(counts)
            .filter(|(_, count)| *count == 1)
            .map(|(edge, _)| {
                let curve = edge.curve();
                assert_eq!(curve.degree(), 1);
                assert_eq!(curve.control_points().len(), 2);
                LineSegment::try_new(
                    curve.evaluate(*curve.domain().start()).unwrap(),
                    curve.evaluate(*curve.domain().end()).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let length = edges.iter().map(|edge| edge.length().unwrap()).sum::<f64>();
        assert_ne!(
            result.value["outputs"][0]["curve"]["samples"],
            reference["value"]["outputs"][0]["curve"]["samples"]
        );
        for record in [&result.value, &reference["value"]] {
            let mut perimeter = 0.;
            for output in record["outputs"].as_array().unwrap() {
                let curve = &output["curve"];
                assert_eq!(curve["type"], "polyline");
                assert_eq!(curve["closed"], true);
                perimeter +=
                    curve["domain"][1].as_f64().unwrap() - curve["domain"][0].as_f64().unwrap();
                for p in curve["samples"].as_array().unwrap() {
                    let p =
                        Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                            .unwrap();
                    assert!(
                        edges.iter().any(|edge| edge
                            .closest_point(p, Tolerance::DEFAULT)
                            .unwrap()
                            .distance_to(p)
                            .unwrap()
                            < 1e-12),
                        "{}",
                        result.id
                    );
                }
            }
            assert!((perimeter - length).abs() < 1e-12);
        }
        let mut a = result.value.clone();
        let mut b = reference["value"].clone();
        for value in [&mut a, &mut b] {
            for output in value["outputs"].as_array_mut().unwrap() {
                output["curve"]["samples"] = Value::Null;
            }
        }
        close(&a, &b, &result.id);
    }
    assert_eq!((parity, diagnostics), (42, 8));
}

#[test]
fn malformed_border_fixture_options_fail_instead_of_running_another_command() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/borders.json"
    ))
    .unwrap();
    let original = fixtures["operations"][0].clone();
    for (field, value) in [
        ("command", json!("Delete")),
        ("output_layer", json!("Input _Delete")),
        ("faces", json!([])),
        ("faces", json!([0, 0])),
        ("faces", json!([0])),
        ("preselect", json!(1)),
    ] {
        let mut operation = original.clone();
        operation[field] = value;
        if let Ok(Operation::BorderCommand { fixture, .. }) =
            serde_json::from_value::<Operation>(operation)
        {
            assert!(run(&fixture, Tolerance::DEFAULT).is_err(), "{field}");
        }
    }
}

#[test]
fn source_artifacts_never_overwrite_existing_files() {
    let request = request();
    let mut fixture = request
        .operations
        .iter()
        .find_map(|operation| match operation {
            Operation::BorderCommand { fixture, .. }
                if matches!(
                    fixture.source,
                    BorderSource::Primitive(BorderPrimitive::Box { .. })
                ) =>
            {
                Some(fixture.clone())
            }
            _ => None,
        })
        .unwrap();
    let temporary = OracleTemporaryFile::new("border-existing");
    std::fs::write(&temporary.path, b"owned sentinel").unwrap();
    fixture.artifact_path = Some(temporary.path.to_str().unwrap().into());
    assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    assert_eq!(std::fs::read(&temporary.path).unwrap(), b"owned sentinel");
}

#[test]
fn every_shared_brep_fixture_survives_its_topology_checked_artifact() {
    for operation in request().operations {
        let Operation::BorderCommand { mut fixture, .. } = operation else {
            unreachable!()
        };
        if !matches!(
            fixture.source.geometry(Tolerance::DEFAULT).unwrap(),
            Geometry::Brep(_)
        ) {
            continue;
        }
        let expected = run(&fixture, Tolerance::DEFAULT).unwrap().0;
        let temporary = OracleTemporaryFile::new("border-roundtrip");
        fixture.artifact_path = Some(temporary.path.to_str().unwrap().into());
        let actual = run(&fixture, Tolerance::DEFAULT).unwrap().0;
        assert_eq!(actual, expected);
        assert!(temporary.path.is_file());
    }
}
