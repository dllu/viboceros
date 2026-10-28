use super::*;
use serde_json::Value;

fn curve(definition: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        definition["degree"].as_u64().unwrap() as usize,
        definition["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                crate::WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[Real; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(definition["knots"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn single_region_native_rebuilds_choose_intervals_after_retiming() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_join_modes.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let expected = curve(&row["value"]["refit_sources"][1]["curves"][0]);
        if expected
            .knots()
            .chunk_by(|a, b| a == b)
            .any(|g| g.len() == 3)
        {
            continue;
        }
        let source = curve(&op["curves"][0]);
        let before = source.clone();
        let tolerance = op["refit_tolerance"].as_f64().unwrap();
        let candidate = super::super::super::pieces::construct(
            &source,
            tolerance,
            1_f64.to_radians(),
            super::super::super::pieces::SplitClock::SourceLookup,
        )
        .unwrap()
        .unwrap();
        let rebuilt =
            super::super::construct_refined(&candidate.curve, 1_f64.to_radians(), tolerance)
                .unwrap()
                .unwrap();
        assert_eq!(rebuilt.knots(), expected.knots(), "{}", op["id"]);
        assert_eq!(
            rebuilt.control_points().len(),
            expected.control_points().len()
        );
        for (actual, reference) in rebuilt
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            maximum = maximum.max(actual.point().distance_to(reference.point()).unwrap());
        }
        // Compare complete curve images as well as the rebuilt knot chart.
        // Strict 1e-9 surface-record comparisons remain separate and retain
        // these small control differences as explicit counterexamples.
        assert!(
            expected
                .parameter_curve_deviation_bound(&rebuilt, tolerance * 0.05)
                .unwrap()
                .is_some(),
            "{}",
            op["id"]
        );
        assert_eq!(source, before);
        count += 1;
    }
    assert_eq!(count, 10);
    // Knot agreement does not establish complete Native control-net agreement.
    println!("C1_REBUILD_INTERVALS cases={count} maximum_control_difference={maximum}");
}

fn hidden_feature() -> NurbsCurve {
    let cuts = [0., 0.03125, 0.0390625, 0.046875, 1.];
    let heights = [0., 0., 0.01, 0., 0.];
    let mut controls = vec![Point3::try_new(0., 0., 0.).unwrap()];
    let mut knots = vec![0.; 4];
    for (i, pair) in cuts.windows(2).enumerate() {
        let width = (pair[1] - pair[0]) / 3.;
        controls.extend([
            Point3::try_new(pair[0] + width, heights[i], 0.).unwrap(),
            Point3::try_new(pair[1] - width, heights[i + 1], 0.).unwrap(),
            Point3::try_new(pair[1], heights[i + 1], 0.).unwrap(),
        ]);
        knots.extend([pair[1]; 3]);
    }
    knots.push(1.);
    NurbsCurve::try_new(3, controls, knots).unwrap()
}

#[test]
fn complete_interval_bound_refines_a_feature_missed_by_all_seven_stations() {
    let source = hidden_feature();
    let before = source.clone();
    for i in 0..=8 {
        assert_eq!(source.evaluate(i as Real / 8.).unwrap().y(), 0.);
    }
    let rebuilt = propose(&source, 1e-4).unwrap().unwrap();
    assert!(rebuilt.control_points().len() > 4);
    assert!(
        source
            .parameter_curve_deviation_bound(&rebuilt, 1e-4)
            .unwrap()
            .is_some()
    );
    assert!(rebuilt.evaluate(0.0390625).unwrap().y() > 0.0099);
    assert_eq!(source, before);

    let image = propose_image(&source, &source, 1e-4).unwrap().unwrap();
    assert!(image.control_points().len() > 4);
    assert!(
        source
            .parameter_curve_deviation_bound(&image, 1e-4)
            .unwrap()
            .is_some()
    );
    assert!(image.evaluate(0.0390625).unwrap().y() > 0.0099);
    assert_eq!(source, before);
}

#[test]
fn image_reconstruction_keeps_the_original_input_budget() {
    let line = |height| {
        NurbsCurve::try_new(
            1,
            [0., 1.]
                .map(|x| Point3::try_new(x, height, 0.).unwrap())
                .to_vec(),
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let original = line(0.);
    let proxy = line(0.00075);
    let before = (original.clone(), proxy.clone());
    assert!(propose(&proxy, 0.001).unwrap().is_some());
    assert!(propose_image(&proxy, &original, 0.001).unwrap().is_none());
    assert_eq!((original, proxy), before);
}

#[test]
fn unresolvable_rebuild_returns_no_partial_curve() {
    let source = hidden_feature();
    let before = source.clone();
    assert!(propose(&source, 1e-100).unwrap().is_none());
    assert_eq!(source, before);
}
