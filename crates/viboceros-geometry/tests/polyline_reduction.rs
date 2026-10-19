use serde_json::Value;
use viboceros_geometry::{GeometryError, Point3, Polyline3, Tolerance};

#[test]
fn independent_native_sdk_vertex_decisions_match_and_collapsed_loops_fail() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reduce_polyline.json"
    ))
    .unwrap();
    let mut checked = 0;
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["sdk"].is_null() {
            continue;
        }
        let points = serde_json::from_value::<Vec<[f64; 3]>>(v["recipe"]["points"].clone())
            .unwrap()
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect();
        let source = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
        let original = source.clone();
        let result = source.try_reduced(v["recipe"]["reduction_tolerance"].as_f64().unwrap());
        assert_eq!(source, original);
        let expected = serde_json::from_value::<Vec<[f64; 3]>>(v["sdk"]["points"].clone()).unwrap();
        if expected.first() == expected.last() && expected.len() < 4 {
            assert_eq!(
                result,
                Err(GeometryError::CollapsedPolylineReduction),
                "{}",
                row["id"]
            );
        } else {
            let result = result.unwrap();
            assert_eq!(result.vertices().len(), expected.len(), "{}", row["id"]);
            for (a, b) in result.vertices().iter().zip(expected) {
                // Native degree elevation can round a source endpoint before
                // reduction; vertex decisions still compare by count and order.
                for (a, b) in a.to_array().into_iter().zip(b) {
                    assert!((a - b).abs() <= 1e-12, "{}: {a} vs {b}", row["id"]);
                }
            }
        }
        checked += 1;
    }
    assert!(checked >= 40);
}

#[test]
fn large_flat_inputs_extreme_scales_and_finite_segment_distance_are_stable() {
    let points = (0..100_000)
        .map(|i| Point3::try_new(i as f64, (i % 2) as f64 * 1e-5, 0.).unwrap())
        .collect();
    let source = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
    let reduced = source.try_reduced(0.001).unwrap();
    assert_eq!(
        reduced.vertices(),
        &[source.vertices()[0], *source.vertices().last().unwrap()]
    );
    for scale in [1e-150, 1., 1e150] {
        let points = [[0., 0., 0.], [3., 0., 0.], [1., 0., 0.]]
            .map(|p| Point3::try_from(p.map(|x| x * scale)).unwrap())
            .to_vec();
        let source = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
        assert_eq!(
            source.try_reduced(scale * 0.1).unwrap().vertices(),
            source.vertices()
        );
        for tolerance in [f64::NAN, f64::INFINITY, -1.] {
            assert!(source.try_reduced(tolerance).is_err());
        }
    }
}

#[test]
fn triangle_exception_uses_public_coincidence_and_distinct_point_rules() {
    let near = [
        [1., 1., 0.],
        [4., 1., 0.],
        [1., 4., 0.],
        [1. + f64::EPSILON, 1., 0.],
    ];
    let source = Polyline3::try_new(
        near.map(|p| Point3::try_from(p).unwrap()).to_vec(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    assert_eq!(
        source.try_reduced(10.).unwrap().vertices(),
        source.vertices()
    );
    let tiny = [
        [0., 0., 0.],
        [1e-150, 0., 0.],
        [0., 1e-150, 0.],
        [0., 0., 0.],
    ];
    let source = Polyline3::try_new(
        tiny.map(|p| Point3::try_from(p).unwrap()).to_vec(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    assert_eq!(
        source.try_reduced(1e-149),
        Err(GeometryError::CollapsedPolylineReduction)
    );
}

#[test]
fn a_subnormal_closing_chord_does_not_reject_regular_source_segments() {
    let points = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [1e-320, 0., 0.],
    ];
    let source = Polyline3::try_new(
        points.map(|p| Point3::try_from(p).unwrap()).to_vec(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    assert_eq!(
        source.try_reduced(0.01).unwrap().vertices(),
        source.vertices()
    );
}

#[test]
fn kernel_and_captured_sdk_timings_use_the_same_flat_input() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reduce_polyline.json"
    ))
    .unwrap();
    let v = &capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "flat_1024")
        .unwrap()["value"];
    let points = serde_json::from_value::<Vec<[f64; 3]>>(v["recipe"]["points"].clone())
        .unwrap()
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect();
    let source = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION).unwrap();
    let tolerance = v["recipe"]["reduction_tolerance"].as_f64().unwrap();
    let mut durations = Vec::new();
    for i in 0..9 {
        let started = std::time::Instant::now();
        let result = std::hint::black_box(&source)
            .try_reduced(tolerance)
            .unwrap();
        let ns = started.elapsed().as_nanos();
        assert_eq!(result.vertices().len(), 2);
        std::hint::black_box(result);
        if i >= 2 {
            durations.push(ns);
        }
    }
    assert_eq!(v["sdk"]["elapsed_ns"].as_array().unwrap().len(), 3);
    eprintln!(
        "REDUCE_POLYLINE_TIMING {}",
        serde_json::json!({"points":source.vertices().len(),"debug":cfg!(debug_assertions),"rust_ns":durations,"rhino_fex_sdk_ns":v["sdk"]["elapsed_ns"]})
    );
}
