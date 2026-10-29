//! Preserve public tangent rounding, repeated queries, source data and clocks.
use serde_json::{Value, json};
use viboceros_oracle::{NurbsCurveDefinition, Operation, ProbeRequest, run_request};

fn read_family(folder: &str, family: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap())
        .unwrap()
}

#[test]
fn ordered_tangent_batches_match_public_native_queries() {
    compare_family("curve_tangents", 14, 56);
}

#[test]
fn stationary_tangent_availability_matches_native() {
    compare_family("curve_tangents_stationary", 2, 5);
}

#[test]
fn explicit_domains_control_query_parameters() {
    compare_family("curve_tangents_domains", 2, 8);
}

fn compare_family(family: &str, expected_contexts: usize, expected_directions: usize) {
    let fixture = read_family("fixtures", family);
    let native = read_family("observations", family);
    let request: ProbeRequest = serde_json::from_value(fixture.clone()).unwrap();
    let before = request.clone();
    let response = run_request(&request).unwrap();
    assert_eq!(request, before);
    assert_eq!(response.results.len(), expected_contexts);
    let mut directions = 0;
    for ((actual, expected), op) in response
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
        .zip(fixture["operations"].as_array().unwrap())
    {
        assert_eq!(actual.id, expected["id"].as_str().unwrap());
        let a = &actual.value;
        let b = &expected["value"];
        assert_eq!(a["before"], a["after"]);
        assert_eq!(b["before"], b["after"]);
        let numbers = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(numbers(&a["parameters"]), numbers(&op["parameters"]));
        assert_eq!(numbers(&a["parameters"]), numbers(&b["parameters"]));
        let definition =
            |v: &Value| serde_json::from_value::<NurbsCurveDefinition>(v.clone()).unwrap();
        let authored = definition(&op["curve"]);
        let constructed = definition(&a["before"]);
        assert_eq!(constructed.degree, authored.degree);
        assert_eq!(constructed.control_points, authored.control_points);
        if let Some(domain) = authored.domain {
            assert_eq!(constructed.domain, Some(domain));
        } else {
            assert_eq!(constructed.knots, authored.knots);
        }
        assert_eq!(a["before"]["degree"], b["before"]["degree"]);
        for key in ["knots", "domain"] {
            assert_eq!(numbers(&a["before"][key]), numbers(&b["before"][key]));
        }
        // Rhino stores homogeneous controls. Multiplying and dividing a
        // nonunit weight can round its constructed Euclidean point once.
        for (a, b) in a["before"]["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["before"]["control_points"].as_array().unwrap())
        {
            assert_eq!(a["weight"], b["weight"]);
            for (a, b) in a["point"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["point"].as_array().unwrap())
            {
                assert!((a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12);
            }
        }
        let a = a["tangents"].as_array().unwrap();
        let b = b["tangents"].as_array().unwrap();
        assert_eq!((a.len(), b.len()), (4, 4));
        if family == "curve_tangents_stationary" {
            assert_eq!(a[1], a[3]);
        } else {
            assert_eq!(a[0], a[3]);
        }
        for (a, b) in a.iter().zip(b) {
            if b.is_null() {
                assert!(a.is_null());
                continue;
            }
            let a = a.as_array().unwrap();
            let b = b.as_array().unwrap();
            assert_eq!((a.len(), b.len()), (3, 3));
            for (a, b) in a.iter().zip(b) {
                assert!(
                    (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12,
                    "{}",
                    actual.id
                );
            }
            directions += 1;
        }
    }
    assert_eq!(directions, expected_directions);
    println!(
        "NATIVE_TANGENT_BATCH family={family} contexts={expected_contexts} directions={directions} epsilon=1e-12"
    );
}

#[test]
fn tangent_query_metadata_is_rejected_before_invalid_source_coordinates() {
    for parameters in [
        vec![],
        vec![-1.],
        vec![2.],
        vec![f64::NAN],
        vec![f64::INFINITY],
        vec![0.; 4097],
    ] {
        let mut query = read_family("fixtures", "curve_tangents");
        query["operations"] = json!([query["operations"][0]]);
        let mut request: ProbeRequest = serde_json::from_value(query).unwrap();
        let Operation::NurbsCurveTangents { fixture, .. } = &mut request.operations[0] else {
            panic!("typed tangent request")
        };
        fixture.parameters = parameters;
        fixture.curve.control_points[0].point[0] = f64::NAN;
        let error = run_request(&request).unwrap_err().to_string();
        assert!(
            error.contains("tangent dimensions or query count")
                || error.contains("tangent parameters are outside"),
            "{error}"
        );
    }
}

#[test]
fn the_query_limit_is_inclusive_and_retains_every_duplicate() {
    let mut query = read_family("fixtures", "curve_tangents");
    query["operations"] = json!([query["operations"][0]]);
    query["operations"][0]["parameters"] = json!(vec![0.; 4096]);
    let request: ProbeRequest = serde_json::from_value(query).unwrap();
    let response = run_request(&request).unwrap();
    let tangents = response.results[0].value["tangents"].as_array().unwrap();
    assert_eq!(tangents.len(), 4096);
    assert!(!tangents[0].is_null());
    assert!(tangents.iter().all(|t| t == &tangents[0]));
}
