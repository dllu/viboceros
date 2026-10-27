//! Native source representation affects numerical length-query semantics.
use serde_json::Value;
use viboceros_geometry::{ArcLengthSampler, CurveRef, Polyline3, Tolerance};
#[allow(dead_code)]
#[path = "support/loft_parameters.rs"]
mod definitions;

#[test]
fn original_nurbs_and_simplified_polyline_clocks_match_native() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_component_clocks.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), 6);
    assert_eq!(native["results"].as_array().unwrap().len(), 6);
    let (mut scalar_count, mut batch_count, mut nurbs_batch_count, mut maximum) = (0, 0, 0, 0_f64);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let source = definitions::curve(&op["curves"][0]);
        let before = source.clone();
        let points = source.control_points().iter().map(|c| c.point()).collect();
        let parameters = source.knots()[1..source.knots().len() - 1].to_vec();
        let polyline =
            Polyline3::try_with_parameters(points, parameters, Tolerance::DEFAULT).unwrap();
        for reference in row["value"]["refit_sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["source_count"] == 1)
        {
            let simplified = reference["simplified"] == true;
            assert_eq!(
                reference["source_types"][0],
                if simplified {
                    "PolylineCurve"
                } else {
                    "NurbsCurve"
                }
            );
            let curve = if simplified {
                CurveRef::Polyline(&polyline)
            } else {
                CurveRef::NurbsCurve(&source)
            };
            let sampler = ArcLengthSampler::try_new(curve, Tolerance::DEFAULT).unwrap();
            for clock in reference["numerical_clocks"].as_array().unwrap() {
                let accuracy = clock["fractional_tolerance"].as_f64().unwrap();
                for station in clock["scalars"].as_array().unwrap() {
                    let parameter = sampler
                        .parameter_at_quadrature_normalized_length(
                            station["fraction"].as_f64().unwrap(),
                            accuracy,
                        )
                        .unwrap();
                    let mut d = (parameter - station["parameter"].as_f64().unwrap()).abs();
                    for (a, b) in curve
                        .evaluate(parameter)
                        .unwrap()
                        .to_array()
                        .into_iter()
                        .zip(station["point"].as_array().unwrap())
                    {
                        d = d.max((a - b.as_f64().unwrap()).abs());
                    }
                    assert!(
                        d <= 1e-9,
                        "{} simplified={simplified} {accuracy}: {d}",
                        op["id"]
                    );
                    maximum = maximum.max(d);
                    scalar_count += 1;
                }
                let fractions = (0..=8).map(|i| i as f64 / 8.).collect::<Vec<_>>();
                let values = sampler
                    .parameters_at_incremental_normalized_lengths(&fractions, accuracy)
                    .unwrap();
                for (a, b) in values
                    .iter()
                    .zip(clock["batch_parameters"].as_array().unwrap())
                {
                    let native = b.as_f64().unwrap();
                    let d = (a - native).abs();
                    assert!(
                        d <= 1e-9,
                        "{} simplified={simplified} {accuracy}: {d}",
                        op["id"]
                    );
                    // Exact natural-knot selection matters beyond coordinate
                    // accuracy: adjacent spans may have different tangents.
                    if !simplified && source.knots().contains(&native) {
                        assert_eq!(*a, native, "{} {accuracy}: natural boundary", op["id"]);
                    }
                    maximum = maximum.max(d);
                    if simplified {
                        batch_count += 1;
                    } else {
                        nurbs_batch_count += 1;
                    }
                }
            }
        }
        assert_eq!(source, before);
    }
    println!(
        "COMPONENT_CLOCK scalars={scalar_count} polyline_batch={batch_count} maximum={maximum} nurbs_batch={nurbs_batch_count}"
    );
    assert_eq!(
        (scalar_count, batch_count, nurbs_batch_count),
        (144, 216, 216)
    );
}
