//! The public closest query does not explain the retained subdivision gaps.
use super::*;

#[test]
fn saved_raw_fitting_stations_match_native_closest_points_and_pass_half_tolerance() {
    let inputs = read("loft_refit_closest_stations_inputs", "fixtures");
    let query = read("loft_refit_closest_stations", "fixtures");
    let native = read("loft_refit_closest_stations", "observations");
    let contexts = read("loft_refit_join_modes", "observations");
    let cases = inputs["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 21);
    let mut maximum_parameter = 0_f64;
    let mut maximum_point = 0_f64;
    let mut maximum_distance = 0_f64;
    let mut count = 0;
    for case in cases {
        let source = curve(&case["source"]);
        let candidate = curve(&case["candidate"]);
        let before = (source.clone(), candidate.clone());
        let context = contexts["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == case["source_id"])
            .unwrap();
        let lookup =
            context["value"]["refit_sources"][1]["numerical_clocks"][3]["batch_parameters"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_f64().unwrap())
                .collect::<Vec<_>>();
        let parameter = |fraction: Real| {
            let station = fraction * 8.;
            let i = (station.floor() as usize).min(7);
            let blend = station - i as Real;
            lookup[i].mul_add(1. - blend, lookup[i + 1] * blend)
        };
        let interval = case["interval"].as_array().unwrap();
        let initial = super::super::super::initial_cubic_from_frames(
            source
                .evaluate(parameter(interval[0].as_f64().unwrap()))
                .unwrap(),
            source
                .evaluate(parameter(interval[1].as_f64().unwrap()))
                .unwrap(),
            source
                .tangent_at_on_side(
                    parameter(interval[0].as_f64().unwrap()),
                    crate::ParameterSide::Right,
                )
                .unwrap(),
            source
                .tangent_at_on_side(
                    parameter(interval[1].as_f64().unwrap()),
                    crate::ParameterSide::Right,
                )
                .unwrap(),
        )
        .unwrap();
        for (actual, saved) in initial
            .control_points()
            .iter()
            .zip(candidate.control_points())
        {
            assert!(actual.point().distance_to(saved.point()).unwrap() < 1e-13);
        }
        for (i, target) in case["targets"].as_array().unwrap().iter().enumerate() {
            let target =
                Point3::try_from(serde_json::from_value::<[Real; 3]>(target.clone()).unwrap())
                    .unwrap();
            let t = case["parameters"][i].as_f64().unwrap();
            assert!(source.evaluate(t).unwrap().distance_to(target).unwrap() < 1e-13);
            let op = &query["operations"][count];
            let row = &native["results"][count];
            assert_eq!(op["id"], row["id"]);
            let native = &row["value"];
            let actual = candidate
                .closest_parameter(target, Tolerance::DEFAULT)
                .unwrap();
            let point = candidate.evaluate(actual).unwrap();
            let reference = Point3::try_from(
                serde_json::from_value::<[Real; 3]>(native["point"].clone()).unwrap(),
            )
            .unwrap();
            maximum_parameter =
                maximum_parameter.max((actual - native["parameter"].as_f64().unwrap()).abs());
            maximum_point = maximum_point.max(point.distance_to(reference).unwrap());
            maximum_distance = maximum_distance.max(
                (point.distance_to(target).unwrap() - native["distance"].as_f64().unwrap()).abs(),
            );
            let limit = case["refit_tolerance"].as_f64().unwrap() * 0.5;
            assert!(point.distance_to(target).unwrap() < limit);
            assert!(native["distance"].as_f64().unwrap() < limit);
            count += 1;
        }
        assert_eq!((source, candidate), before);
    }
    assert_eq!(count, 147);
    assert!(maximum_parameter < 1e-9, "{maximum_parameter}");
    assert!(maximum_point < 1e-9, "{maximum_point}");
    assert!(maximum_distance < 1e-12, "{maximum_distance}");
    println!(
        "RAW_STATION_CLOSEST queries={count} maximum_parameter={maximum_parameter} maximum_point={maximum_point} maximum_distance={maximum_distance}"
    );
}
