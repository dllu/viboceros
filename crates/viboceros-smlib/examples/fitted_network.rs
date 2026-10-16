//! Independent automatic Gordon fitting; saved SDK data supplies comparisons only.
use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};
use viboceros_smlib::{GordonCurveFitClock, fitted_gordon_network_surface};
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/network_surface.json"
    ))?;
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/gordon_network_rhino_20261015.json"
    ))?;
    let mut cases = std::env::args().skip(1).collect::<Vec<_>>();
    if cases.is_empty() {
        cases = vec!["biquadratic_network".into(), "rational_cylinder".into()];
    }
    let numerical = Tolerance::DEFAULT;
    let mut rows = vec![];
    for id in cases {
        let op = request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["id"] == id)
            .ok_or("unknown network fixture")?;
        let expected = &reference["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["id"] == id)
            .ok_or("missing reference fixture")?["value"];
        let curves = op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let curve = NurbsCurve::try_new_rational(
                    c["degree"].as_u64().unwrap() as usize,
                    c["control_points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            WeightedPoint3::try_new(
                                point(&p["point"]),
                                p["weight"].as_f64().unwrap(),
                            )
                            .unwrap()
                        })
                        .collect(),
                    serde_json::from_value(c["knots"].clone()).unwrap(),
                )?;
                Ok::<_, viboceros_geometry::GeometryError>(if let Some(d) = c.get("domain") {
                    curve.try_reparameterized(d[0].as_f64().unwrap()..=d[1].as_f64().unwrap())?
                } else {
                    curve
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let before = curves.clone();
        let edge = Tolerance::try_new(op["edge_tolerance"].as_f64().unwrap(), 1e-12, 1e-8)?;
        let interior = Tolerance::try_new(op["interior_tolerance"].as_f64().unwrap(), 1e-12, 1e-8)?;
        let result = fitted_gordon_network_surface(
            &curves,
            edge,
            interior,
            numerical,
            GordonCurveFitClock::Local {
                transition_fraction: 0.5,
            },
        )?;
        let mut maximum = 0_f64;
        let mut queries = vec![];
        for (index, (q, p)) in op["queries"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["closest_points"].as_array().unwrap())
            .enumerate()
        {
            let q = point(q);
            let expected = point(p);
            let (u, v) = result.surface.closest_parameters(q, numerical)?;
            let actual = result.surface.evaluate(u, v)?;
            let distance = actual.distance_to(expected)?;
            maximum = maximum.max(distance);
            queries.push(json!({"query_index":index,"query":q.to_array(),"actual":actual.to_array(),"reference":expected.to_array(),"output_distance":distance}));
        }
        let fits=result.fitted_curves.iter().map(|family|family.iter().map(|c|json!({"degree":c.degree(),"knots":c.knots(),
            "control_points":c.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()).collect::<Vec<_>>();
        let images=result.source_images.iter().map(|image|json!({"source_curve_index":image.source_index,
            "complete_distance_bound":image.distance_bound,"certified":true,"uv_degree":image.uv.degree(),"uv_knots":image.uv.knots(),
            "uv_control_points":image.uv.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()
        })).collect::<Vec<_>>();
        assert_eq!(curves, before);
        rows.push(json!({"fixture":id,"reference_derived_inputs":false,"families":result.families,"common_stations":result.stations,
            "spans_per_interval":result.spans_per_interval,"family_fit_attempts":result.family_fit_attempts,
            "maximum_sampled_point_distance":result.maximum_sampled_point_distance,
            "fitted_curves":fits,"original_source_curve_bounds":images,"original_curve_images_certified":true,
            "surface_control_counts":[result.surface.control_point_count_u(),result.surface.control_point_count_v()],"surface_degree":[result.surface.degree_u(),result.surface.degree_v()],
            "maximum_interior_knot_multiplicities":[result.surface.knots_u().chunk_by(|a,b|a==b).filter(|k|k[0]>0.&&k[0]<1.).map(|k|k.len()).max().unwrap_or(0),result.surface.knots_v().chunk_by(|a,b|a==b).filter(|k|k[0]>0.&&k[0]<1.).map(|k|k.len()).max().unwrap_or(0)],
            "maximum_rhino_query_output_distance":maximum,"query_outputs_within_1e_minus_6":maximum<=1e-6,"queries":queries}));
    }
    serde_json::to_writer_pretty(
        std::io::stdout().lock(),
        &json!({"schema_version":1,"engine":"viboceros","new_licensed_rhino_captures":0,
        "qualification":"Experimental automatic fitting and strict original-image admission; not a qualified NetworkSrf command.",
        "comparison":"Returned closest-point outputs, not continuous surface separation.","query_numerical_absolute_tolerance":numerical.absolute(),"results":rows}),
    )?;
    println!();
    Ok(())
}
