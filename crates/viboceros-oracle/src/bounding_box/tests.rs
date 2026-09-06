use super::*;

const ORDINARY: &str = include_str!("../../../../tools/rhino_oracle/fixtures/bounding_box.json");
const DIAGNOSTICS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/bounding_box_diagnostics.json");

#[test]
fn permanent_commands_check_output_topology_selection_groups_and_report_counts() {
    for (text, count) in [(ORDINARY, 57), (DIAGNOSTICS, 26)] {
        let request: ProbeRequest = serde_json::from_str(text).unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), count);
        for (op, result) in request.operations.iter().zip(response.results) {
            let Operation::BoundingBoxCommand { id, fixture } = op else {
                panic!("BoundingBox fixture")
            };
            let value = result.value;
            let indices = (0..fixture.sources.len()).collect::<Vec<_>>();
            assert_eq!(value["sources_retained"], json!(indices));
            assert_eq!(value["selected_sources"], json!(indices));
            let failure =
                id.starts_with("line-") || id.starts_with("point-") || id.starts_with("mixed-");
            assert_eq!(value["succeeded"], !failure, "{id}");
            assert_eq!(
                value["reported_boxes"],
                if failure {
                    0
                } else if fixture.cumulative {
                    1
                } else {
                    fixture.sources.len()
                },
                "{id}"
            );
            let outputs = value["objects"].as_array().unwrap();
            if failure || fixture.output == Output::None {
                assert!(outputs.is_empty(), "{id}");
            } else {
                assert!(!outputs.is_empty(), "{id}");
            }
            for record in outputs {
                assert_eq!(record["selected"], false);
                assert_eq!(record["current_layer"], true);
                assert_eq!(record["name"], Value::Null);
                assert_eq!(record["closed"], true);
                match record["kind"].as_str().unwrap() {
                    "brep" => {
                        assert_eq!(record["faces"], 6);
                        assert_eq!(record["points"].as_array().unwrap().len(), 8);
                    }
                    "mesh" => {
                        assert_eq!(record["face_sizes"], json!(vec![4; 6]));
                        assert_eq!(record["points"].as_array().unwrap().len(), 24);
                    }
                    "curve" => {
                        assert_eq!(record["degree"], 1);
                        assert_eq!(record["points"].as_array().unwrap().len(), 4);
                    }
                    _ => panic!("unexpected output"),
                }
            }
            for size in value["group_sizes"].as_array().unwrap() {
                assert_eq!(size, 6);
                assert_eq!(fixture.output, Output::Curves);
            }
        }
    }
}

#[test]
fn curved_diagnostic_outputs_match_independent_quadratic_extrema() {
    let request: ProbeRequest = serde_json::from_str(DIAGNOSTICS).unwrap();
    let tolerance = request.tolerance.geometry().unwrap();
    let mut checked = 0;
    for op in &request.operations {
        let Operation::BoundingBoxCommand { id, fixture: f } = op else {
            panic!("fixture")
        };
        if !id.contains("disk") && !id.contains("surface") {
            continue;
        }
        let frame = Frame3::try_from_directions(
            Point3::try_from(f.origin).unwrap(),
            Vector3::try_from(f.x_axis).unwrap(),
            Vector3::try_from(f.y_axis).unwrap(),
            tolerance,
        )
        .unwrap();
        let source = f.sources[0].geometry(tolerance).unwrap();
        let surface = match &source {
            Geometry::Brep(b) => b.faces()[0].surface(),
            Geometry::NurbsSurface(s) => s,
            _ => panic!("quadratic"),
        };
        let sample = |u, v| {
            frame
                .coordinates_of(surface.evaluate(u, v).unwrap())
                .unwrap()
        };
        let c = sample(0., 0.);
        let (a, b, qu, qv): ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) = if id.contains("disk") {
            let xp = sample(1., 0.);
            let xm = sample(-1., 0.);
            let yp = sample(0., 1.);
            let ym = sample(0., -1.);
            (
                std::array::from_fn(|i| (xp[i] - xm[i]) / 2.),
                std::array::from_fn(|i| (yp[i] - ym[i]) / 2.),
                std::array::from_fn(|i| (xp[i] + xm[i]) / 2. - c[i]),
                std::array::from_fn(|i| (yp[i] + ym[i]) / 2. - c[i]),
            )
        } else {
            let xp = sample(1., 0.);
            let xh = sample(0.5, 0.);
            let yp = sample(0., 1.);
            let yh = sample(0., 0.5);
            let qu: [f64; 3] = std::array::from_fn(|i| 2. * (xp[i] + c[i] - 2. * xh[i]));
            let qv: [f64; 3] = std::array::from_fn(|i| 2. * (yp[i] + c[i] - 2. * yh[i]));
            (
                std::array::from_fn(|i| xp[i] - c[i] - qu[i]),
                std::array::from_fn(|i| yp[i] - c[i] - qv[i]),
                qu,
                qv,
            )
        };
        // Check the assumed polynomial independently of the bounds routine.
        for (u, v) in [(0.1, 0.2), (0.3, 0.7), (0.9, 0.4)] {
            let p = sample(u, v);
            for i in 0..3 {
                assert!(
                    (p[i] - (c[i] + a[i] * u + b[i] * v + qu[i] * u * u + qv[i] * v * v)).abs()
                        < 1e-12
                );
            }
        }
        let mut min = [0.; 3];
        let mut max = [0.; 3];
        for i in 0..3 {
            let (lo, hi) = if id.contains("disk") {
                assert!((qu[i] - qv[i]).abs() < 1e-12);
                let r = 0.8;
                let edge = qu[i] * r * r;
                let slope = a[i].hypot(b[i]);
                let mut values = vec![edge - r * slope, edge + r * slope];
                if qu[i] != 0. && slope <= 2. * qu[i].abs() * r {
                    values.push(-slope * slope / (4. * qu[i]));
                }
                (
                    values.iter().copied().fold(f64::INFINITY, f64::min),
                    values.into_iter().fold(f64::NEG_INFINITY, f64::max),
                )
            } else {
                let extrema = |linear: f64, quadratic: f64| {
                    let mut values = vec![0., linear + quadratic];
                    let t = -linear / (2. * quadratic);
                    if (0. ..=1.).contains(&t) {
                        values.push(linear * t + quadratic * t * t);
                    }
                    (
                        values.iter().copied().fold(f64::INFINITY, f64::min),
                        values.into_iter().fold(f64::NEG_INFINITY, f64::max),
                    )
                };
                let x = extrema(a[i], qu[i]);
                let y = extrema(b[i], qv[i]);
                (x.0 + y.0, x.1 + y.1)
            };
            min[i] = c[i] + lo;
            max[i] = c[i] + hi;
        }
        let corners = (0..8)
            .map(|bits| {
                frame
                    .point_at(std::array::from_fn(|i| {
                        if bits & (1 << i) == 0 { min[i] } else { max[i] }
                    }))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let (value, _) = run(f, tolerance).unwrap();
        let actual = value["objects"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|o| o["points"].as_array().unwrap())
            .map(|p| {
                Point3::try_new(
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        for p in &actual {
            assert!(
                corners.iter().any(|q| p.distance_to(*q).unwrap() < 1e-8),
                "{id}: {p:?}"
            );
        }
        for p in &corners {
            assert!(
                actual.iter().any(|q| p.distance_to(*q).unwrap() < 1e-8),
                "{id}: {p:?}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 18);
}

#[test]
fn malformed_options_and_vertex_sources_are_rejected() {
    let request: Value = serde_json::from_str(ORDINARY).unwrap();
    let f = request["operations"][0].clone();
    for (key, value) in [
        ("coordinate_system", json!("World _Delete")),
        ("output", json!("SubD")),
        ("cumulative", json!("No")),
    ] {
        let mut invalid = f.clone();
        invalid[key] = value;
        assert!(serde_json::from_value::<BoundingBoxFixture>(invalid).is_err());
    }
    for sources in [
        json!([]),
        json!([{"type":"point_cloud","points":[]}]),
        json!([{"type":"mesh","vertices":[[0,0,0]],"faces":[[0,1,2]]}]),
    ] {
        let mut invalid = f.clone();
        invalid["sources"] = sources;
        let fixture: BoundingBoxFixture = serde_json::from_value(invalid).unwrap();
        assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    }
}
