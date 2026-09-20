use super::*;
use viboceros_geometry::PointProjection3;

fn fixtures(diagnostics: bool) -> (ProbeRequest, Value) {
    let (request, response) = if diagnostics {
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/align_fit_diagnostics.json"),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/align_fit_diagnostics.json"
            ),
        )
    } else {
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/align_fit.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/align_fit.json"),
        )
    };
    (
        serde_json::from_str(request).unwrap(),
        serde_json::from_str(response).unwrap(),
    )
}

#[test]
fn fitted_alignment_matches_all_regular_records_including_terminal_failures() {
    let (request, rhino) = fixtures(false);
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 39);
    assert_eq!(rhino["results"].as_array().unwrap().len(), 39);
    assert_eq!(
        actual
            .results
            .iter()
            .filter(|r| r.value["succeeded"] == false)
            .count(),
        6
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(rhino["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn ambiguous_and_mesh_fit_diagnostics_keep_raw_differences_and_check_their_causes() {
    let (request, rhino) = fixtures(true);
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 6);
    assert_eq!(rhino["results"].as_array().unwrap().len(), 6);
    for (mut a, b) in actual
        .results
        .into_iter()
        .zip(rhino["results"].as_array().unwrap())
        .take(5)
    {
        assert_eq!(a.id, b["id"]);
        if a.id.starts_with("mixed") {
            let mut max_error = 0_f64;
            for (p, q) in a.value["objects"][3]["points"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(b["value"]["objects"][3]["points"].as_array().unwrap())
            {
                for i in 0..3 {
                    let raw = p[i].as_f64().unwrap();
                    let expected = q[i].as_f64().unwrap();
                    max_error = max_error.max((raw - expected).abs());
                    assert_eq!(f64::from(raw as f32), expected);
                    p[i] = json!(f64::from(raw as f32));
                }
            }
            assert!(max_error > 1e-8);
        } else {
            let original = if a.id == "fit-tetra" {
                vec![[1., 1., 1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.]]
            } else {
                vec![
                    [1., 0., 0.],
                    [-1., 0., 0.],
                    [0., 1., 0.],
                    [0., -1., 0.],
                    [0., 0., 1.],
                    [0., 0., -1.],
                ]
            };
            let points = |value: &Value| {
                value["objects"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|o| {
                        Point3::try_from(std::array::from_fn(|i| {
                            o["points"][0][i].as_f64().unwrap()
                        }))
                        .unwrap()
                    })
                    .collect::<Vec<_>>()
            };
            for values in [&a.value, &b["value"]] {
                let output = points(values);
                let mut normal = None;
                for p in &output {
                    for q in &output {
                        let n = Vector3::try_from(p.to_array())
                            .unwrap()
                            .cross(Vector3::try_from(q.to_array()).unwrap())
                            .unwrap();
                        if n.length().unwrap() > 1e-8 {
                            normal = Some(n.normalized_nonzero().unwrap().as_vector());
                        }
                    }
                }
                let n = normal.unwrap();
                let mut residual = 0.;
                for (input, output) in original.iter().zip(output) {
                    assert!(
                        n.dot(Vector3::try_from(output.to_array()).unwrap())
                            .unwrap()
                            .abs()
                            < 1e-12
                    );
                    let movement = Point3::try_from(*input).unwrap().vector_to(output).unwrap();
                    assert!(movement.cross(n).unwrap().length().unwrap() < 1e-12);
                    residual += movement.dot(movement).unwrap();
                }
                // Both point sets have isotropic scatter: all normals attain
                // the same minimum squared-distance sum (4 or 2 respectively).
                assert!((residual - if a.id == "fit-tetra" { 4. } else { 2. }).abs() < 1e-12);
            }
            assert_ne!(points(&a.value), points(&b["value"]));
            for (actual, reference) in a.value["objects"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(b["value"]["objects"].as_array().unwrap())
            {
                actual["points"] = reference["points"].clone();
            }
        }
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn curved_bound_fit_is_checked_against_analytic_bottom_center_anchors() {
    let (request, rhino) = fixtures(true);
    let actual = run_request(&request).unwrap();
    let Operation::Align { fixture, .. } = request.operations.last().unwrap() else {
        panic!("align")
    };
    let result = actual.results.last().unwrap();
    assert_eq!(result.id, "disk-fit");
    let basis = [[1., 2., 3.], [-2., 1., 0.], [-3., -6., 5.]];
    let dot = |p: [f64; 3], axis: usize| (0..3).map(|i| p[i] * basis[axis][i]).sum::<f64>();
    let world = |local: [f64; 3]| {
        Point3::try_from(std::array::from_fn(|i| {
            local[0] * basis[0][i] / 14.
                + local[1] * basis[1][i] / 5.
                + local[2] * basis[2][i] / 70.
        }))
        .unwrap()
    };
    let tolerance = request.tolerance.geometry().unwrap();
    let sources = fixture
        .layout
        .sources
        .iter()
        .map(|s| s.geometry(tolerance).unwrap())
        .collect::<Vec<_>>();
    let mut anchors = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        if i == 1 {
            // z=u²+v² on radius .8. X=u+2v+3z has min -5/12,
            // max 3r²+r√5; Y=-2u+v is symmetric. Z=-3u-6v+5z
            // attains -9/4 at (u,v)=(3/10,3/5), inside the disk.
            anchors.push(world([
                (-5_f64 / 12.).midpoint(3. * 0.8_f64.powi(2) + 0.8 * 5_f64.sqrt()),
                0.,
                -9. / 4.,
            ]));
        } else {
            let (_, points) = crate::object_layout::sample(source).unwrap();
            anchors.push(world(std::array::from_fn(|axis| {
                let low = points
                    .iter()
                    .map(|p| dot(*p, axis))
                    .fold(f64::INFINITY, f64::min);
                let high = points
                    .iter()
                    .map(|p| dot(*p, axis))
                    .fold(f64::NEG_INFINITY, f64::max);
                if axis == 2 { low } else { low.midpoint(high) }
            })));
        }
    }
    let plane = PointProjection3::onto_best_fit_plane(&anchors).unwrap();
    let mut discrepancy = 0_f64;
    for (i, (source, anchor)) in sources.iter().zip(anchors).enumerate() {
        let movement = anchor.vector_to(plane.project(anchor).unwrap()).unwrap();
        let (_, points) = crate::object_layout::sample(source).unwrap();
        for (j, point) in points.into_iter().enumerate() {
            let expected = Point3::try_from(point)
                .unwrap()
                .translated(movement)
                .unwrap()
                .to_array();
            for axis in 0..3 {
                let value = result.value["objects"][i]["points"][j][axis]
                    .as_f64()
                    .unwrap();
                assert!((value - expected[axis]).abs() < 1e-8);
                discrepancy = discrepancy.max(
                    (value
                        - rhino["results"][5]["value"]["objects"][i]["points"][j][axis]
                            .as_f64()
                            .unwrap())
                    .abs(),
                );
            }
        }
    }
    assert!(discrepancy > 1e-6);
}
