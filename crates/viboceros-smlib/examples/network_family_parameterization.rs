//! Independent shared-basis family fitting on one retained network fixture.
use serde_json::{Value, json};
use viboceros_geometry::{CurveFitStation, NurbsCurve, Point2, Point3, Tolerance, WeightedPoint3};
use viboceros_smlib::{
    GordonCurveFitClock, GordonCurveFitInput, fit_gordon_curve_family_with_clock,
    prepared_gordon_surface,
};
fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
fn point(value: &Value) -> Point3 {
    Point3::try_from(<[f64; 3]>::try_from(numbers(value)).unwrap()).unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spans = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(60);
    let transition = std::env::args()
        .nth(2)
        .map(|s| s.parse::<f64>())
        .transpose()?;
    let policy = transition.map_or(GordonCurveFitClock::WholeInterval, |transition_fraction| {
        GordonCurveFitClock::Local {
            transition_fraction,
        }
    });
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
        .find(|o| o["id"] == "biquadratic_network")
        .unwrap();
    let reference = reference["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == operation["id"])
        .unwrap();
    let original = operation["curves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                c["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap())
                            .unwrap()
                    })
                    .collect(),
                numbers(&c["knots"]),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(original.len(), 6);
    let before = original.clone();
    let numerical = Tolerance::DEFAULT;
    let common = [&original[..3], &original[3..]].map(|family| {
        family
            .iter()
            .map(|c| {
                let first = c
                    .try_trimmed(*c.domain().start()..=c.parameter_at(0.5).unwrap())
                    .unwrap();
                first.length(numerical).unwrap() / c.length(numerical).unwrap()
            })
            .sum::<f64>()
            / family.len() as f64
    });
    let mut fitted = vec![];
    let mut fit_distances = vec![];
    for (index, family) in [&original[..3], &original[3..]].into_iter().enumerate() {
        let stations = family
            .iter()
            .map(|c| {
                [0., 0.5, 1.]
                    .into_iter()
                    .zip([0., common[index], 1.])
                    .map(|(s, t)| {
                        let source_parameter = c.parameter_at(s).unwrap();
                        CurveFitStation {
                            source_parameter,
                            fit_parameter: t,
                            point: c.evaluate(source_parameter).unwrap(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let inputs = family
            .iter()
            .zip(&stations)
            .map(|(curve, stations)| GordonCurveFitInput { curve, stations })
            .collect::<Vec<_>>();
        let result =
            fit_gordon_curve_family_with_clock(&inputs, &[spans, spans], 1e-7, numerical, policy)?;
        fit_distances.push(result.maximum_sampled_point_distance);
        fitted.push(result.curves);
    }
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8)?;
    let surface = prepared_gordon_surface(
        &fitted[0],
        &fitted[1],
        &[0., common[0], 1.],
        &[0., common[1], 1.],
        tolerance,
        tolerance,
    )?;
    let mut original_bounds = vec![];
    for (i, curve) in original.iter().enumerate() {
        let fixed = if i < 3 {
            [0., common[1], 1.][i]
        } else {
            [0., common[0], 1.][i - 3]
        };
        let endpoints = if i < 3 {
            [[0., fixed], [1., fixed]]
        } else {
            [[fixed, 0.], [fixed, 1.]]
        };
        let endpoints = endpoints.map(|p| Point2::try_new(p[0], p[1]).unwrap());
        original_bounds.push(match surface.try_pullback_curve_certified_with_bound(curve,Some(endpoints),tolerance) {
            Ok((uv,bound))=>json!({"source_curve_index":i,"certified":true,"complete_distance_bound":bound,"uv_control_count":uv.control_points().len()}),
            Err(error)=>json!({"source_curve_index":i,"certified":false,"error":error.to_string()}),
        });
    }
    let mut maximum = 0_f64;
    let mut rows = vec![];
    for (i, (q, expected)) in operation["queries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["value"]["closest_points"].as_array().unwrap())
        .enumerate()
    {
        let query = point(q);
        let (u, v) = surface.closest_parameters(query, tolerance)?;
        let actual = surface.evaluate(u, v)?;
        let expected = point(expected);
        let distance = actual.distance_to(expected)?;
        maximum = maximum.max(distance);
        rows.push(json!({"query_index":i,"query":query.to_array(),"actual":actual.to_array(),"reference":expected.to_array(),"output_distance":distance}));
    }
    let definitions=fitted.iter().map(|family|family.iter().map(|c|json!({"degree":c.degree(),"knots":c.knots(),
        "control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()
    })).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(original, before);
    let output = json!({"schema_version":2,"engine":"viboceros","fixture":"biquadratic_network",
        "reference_derived_inputs":false,"new_licensed_rhino_captures":0,
        "qualification":"Experimental shared-basis family fitting; not a qualified NetworkSrf command.",
        "comparison":"Returned closest-point outputs, not continuous surface separation.",
        "uniform_spans_per_network_interval":spans,"common_stations":common,
        "clock_transition_fraction":transition,"clock_policy":if transition.is_some(){"local"}else{"whole_interval"},
        "curve_fit_tolerance":1e-7,"maximum_sampled_point_distances":fit_distances,
        "fitted_curves":definitions,"surface_degree":[surface.degree_u(),surface.degree_v()],
        "surface_control_counts":[surface.control_point_count_u(),surface.control_point_count_v()],
        "maximum_interior_knot_multiplicities":[surface.knots_u().chunk_by(|a,b|a==b).filter(|k|k[0]>0.&&k[0]<1.).map(|k|k.len()).max().unwrap_or(0),surface.knots_v().chunk_by(|a,b|a==b).filter(|k|k[0]>0.&&k[0]<1.).map(|k|k.len()).max().unwrap_or(0)],
        "supplied_fitted_curve_images_certified":true,"original_curve_images_certified":original_bounds.iter().all(|r|r["certified"]==true),
        "original_source_curve_bounds":original_bounds,"maximum_rhino_query_output_distance":maximum,"query_outputs_within_1e_minus_6":maximum<=1e-6,"queries":rows});
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    println!();
    Ok(())
}
