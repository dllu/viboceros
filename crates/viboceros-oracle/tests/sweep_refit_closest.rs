//! Public closest-point calibration on independent fitting candidates.
use serde_json::Value;
use viboceros_geometry::Point3;
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn all_candidate_closest_points_match_the_public_native_queries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let input: ProbeRequest = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/sweep1_refit_closest.json")).unwrap(),
    )
    .unwrap();
    let native: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/sweep1_refit_closest.json")).unwrap(),
    )
    .unwrap();
    let actual = run_request(&input).unwrap();
    assert_eq!(actual.results.len(), 140);
    assert_eq!(native["results"].as_array().unwrap().len(), 140);
    let mut maximum = 0_f64;
    for (a, b) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"].as_str().unwrap());
        let point = |v: &Value| {
            Point3::try_from(serde_json::from_value::<[f64; 3]>(v["point"].clone()).unwrap())
                .unwrap()
        };
        let distance = point(&a.value).distance_to(point(&b["value"])).unwrap();
        maximum = maximum.max(distance);
        assert!(distance <= 1e-7, "{}: {distance}", a.id);
        assert!(
            (a.value["distance"].as_f64().unwrap() - b["value"]["distance"].as_f64().unwrap())
                .abs()
                <= 1e-7
        );
    }
    println!("140 public closest-point queries; maximum point difference {maximum}");
}
