//! Compare experimental parameter choices with retained public SDK output.
//! This probe qualifies neither NetworkSrf fitting nor continuous surface parity.
use serde_json::{Value, json};
use viboceros_geometry::{
    CurveFitConstraint, CurveFitStation, CurveRef, NurbsCurve, NurbsSurface, Point3, Tolerance,
    WeightedPoint3,
};
use viboceros_smlib::{
    fit_cubic_curve_at_stations, fit_cubic_curve_with_constraints, gordon_network_surface,
};

fn point(value: &Value) -> Point3 {
    Point3::try_new(
        value[0].as_f64().unwrap(),
        value[1].as_f64().unwrap(),
        value[2].as_f64().unwrap(),
    )
    .unwrap()
}

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

fn controls(definition: &Value) -> Vec<WeightedPoint3> {
    definition["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This bounded investigation intentionally uses one retained fixture.
    // Its three source stations per curve are known from the fixture recipe.
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/network_surface.json"
    ))?;
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/gordon_network_rhino_20261015.json"
    ))?;
    let operation = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "biquadratic_network")
        .unwrap();
    let reference = reference["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == operation["id"])
        .unwrap();
    let original = operation["curves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                controls(c),
                numbers(&c["knots"]),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(original.len(), 6);
    let before = original.clone();
    let constraints = |curve: &NurbsCurve| {
        [0., 0.5, 1.].map(|t| {
            let source_parameter = curve.parameter_at(t).unwrap();
            CurveFitConstraint {
                source_parameter,
                point: curve.evaluate(source_parameter).unwrap(),
            }
        })
    };
    let arc = original
        .iter()
        .map(|curve| {
            fit_cubic_curve_with_constraints(
                CurveRef::NurbsCurve(curve),
                &constraints(curve),
                1e-7,
                0.,
                Tolerance::DEFAULT,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let common = [&arc[..3], &arc[3..]].map(|family| {
        family
            .iter()
            .map(|fit| fit.constraint_parameters[1] / fit.curve.domain().end())
            .sum::<f64>()
            / family.len() as f64
    });
    let smooth = original
        .iter()
        .enumerate()
        .map(|(i, curve)| {
            let stations = constraints(curve)
                .into_iter()
                .zip([0., common[i / 3], 1.])
                .map(|(constraint, fit_parameter)| CurveFitStation {
                    source_parameter: constraint.source_parameter,
                    fit_parameter,
                    point: constraint.point,
                })
                .collect::<Vec<_>>();
            fit_cubic_curve_at_stations(
                CurveRef::NurbsCurve(curve),
                &stations,
                1e-7,
                0.,
                Tolerance::DEFAULT,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arc_curves = arc.iter().map(|f| f.curve.clone()).collect::<Vec<_>>();
    let smooth_curves = smooth.iter().map(|f| f.curve.clone()).collect::<Vec<_>>();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8)?;
    let queries = operation["queries"].as_array().unwrap();
    let expected = reference["value"]["closest_points"].as_array().unwrap();
    assert_eq!(queries.len(), expected.len());
    let mut candidates = vec![];
    for (name, curves, original_images) in [
        ("raw_gordon", &original, true),
        ("crossing_constrained_arc_length", &arc_curves, false),
        ("global_smooth_station_map", &smooth_curves, false),
    ] {
        let surface = gordon_network_surface(curves, tolerance, tolerance)?;
        let mut maximum = 0_f64;
        let mut rows = vec![];
        for (index, (query, reference)) in queries.iter().zip(expected).enumerate() {
            let q = point(query);
            let (u, v) = surface.closest_parameters(q, tolerance)?;
            let actual = surface.evaluate(u, v)?;
            let reference = point(reference);
            let distance = actual.distance_to(reference)?;
            maximum = maximum.max(distance);
            rows.push(json!({"query_index":index,"query":q.to_array(),"actual":actual.to_array(),"reference":reference.to_array(),"output_distance":distance}));
        }
        candidates.push(json!({"candidate":name,
            "curve_control_counts":curves.iter().map(|c|c.control_points().len()).collect::<Vec<_>>(),
            "surface_control_counts":[surface.control_point_count_u(),surface.control_point_count_v()],
            "supplied_curve_images_certified":true,"pre_fit_original_curve_images_certified":original_images,
            "maximum_rhino_query_output_distance":maximum,"query_outputs_within_1e_minus_6":maximum<=1e-6,
            "queries":rows}));
    }
    let d = &reference["value"]["definition"];
    let surface = NurbsSurface::try_new_rational(
        d["degree"][0].as_u64().unwrap() as usize,
        d["degree"][1].as_u64().unwrap() as usize,
        d["control_count"][0].as_u64().unwrap() as usize,
        d["control_count"][1].as_u64().unwrap() as usize,
        controls(d),
        numbers(&d["knots_u"]),
        numbers(&d["knots_v"]),
    )?;
    let mut traces = vec![];
    for (index, curve) in original.iter().enumerate() {
        let mut rows = vec![];
        for i in 0..=16 {
            let source_parameter = curve.parameter_at(i as f64 / 16.)?;
            let q = curve.evaluate(source_parameter)?;
            let (u, v) = surface.closest_parameters(q, Tolerance::DEFAULT)?;
            rows.push(json!({"source_parameter":source_parameter,"point":q.to_array(),
                "normalized_surface_parameters":[u/surface.domain_u().end(),v/surface.domain_v().end()],
                "rust_closest_point_distance":surface.evaluate(u,v)?.distance_to(q)?}));
        }
        traces.push(json!({"source_curve_index":index,"samples":rows}));
    }
    assert_eq!(original, before);
    let output = json!({"schema_version":1,"engine":"viboceros","engine_version":env!("CARGO_PKG_VERSION"),"fixture":"biquadratic_network",
        "qualification":"Diagnostic only; none of the candidates qualifies Rhino NetworkSrf interior fitting.",
        "comparison":"World-space returned closest-point outputs; not continuous surface separation or Hausdorff bounds.",
        "source_fit_error_policy":"Adaptive sampled fitting at 1e-7; exact stored-curve position constraints at 1e-9.",
        "new_licensed_rhino_captures":0,"reference_observation":"gordon_network_rhino_20261015.json",
        "common_curve_fit_station_parameters":common,"candidates":candidates,"saved_rhino_tensor_parameter_traces":traces});
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    println!();
    Ok(())
}
