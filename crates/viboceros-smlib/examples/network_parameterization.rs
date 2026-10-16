//! Compare experimental parameter choices with retained public SDK output.
//! This probe qualifies neither NetworkSrf fitting nor continuous surface parity.
use serde_json::{Value, json};
use viboceros_geometry::{
    CurveFitConstraint, CurveFitStation, CurveRef, NurbsCurve, NurbsSurface, Point3, Tolerance,
    WeightedPoint3,
};
use viboceros_smlib::{
    fit_cubic_curve_at_stations, fit_cubic_curve_with_constraints, gordon_network_surface,
    prepared_gordon_surface,
};

fn sampled_gordon_residual(
    surface: &NurbsSurface,
    center: [f64; 2],
) -> Result<Value, Box<dyn std::error::Error>> {
    let domains = [surface.domain_u(), surface.domain_v()];
    let stations = [
        [*domains[0].start(), center[0], *domains[0].end()],
        [*domains[1].start(), center[1], *domains[1].end()],
    ];
    let u_curves = stations[1]
        .map(|v| surface.isocurve_u(v))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let v_curves = stations[0]
        .map(|u| surface.isocurve_v(u))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut grid = [[[0.; 3]; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            grid[i][j] = surface.evaluate(stations[0][i], stations[1][j])?.to_array();
        }
    }
    let basis = |axis: usize, t: f64| {
        std::array::from_fn::<_, 3, _>(|i| {
            (0..3)
                .filter(|&j| j != i)
                .map(|j| (t - stations[axis][j]) / (stations[axis][i] - stations[axis][j]))
                .product::<f64>()
        })
    };
    let anchor = grid[1][1];
    let mut maximum = 0_f64;
    for i in 0..=64 {
        for j in 0..=64 {
            let u = (i as f64 / 64.)
                .mul_add(domains[0].end() - domains[0].start(), *domains[0].start());
            let v = (j as f64 / 64.)
                .mul_add(domains[1].end() - domains[1].start(), *domains[1].start());
            let bu = basis(0, u);
            let bv = basis(1, v);
            let mut actual = anchor;
            for k in 0..3 {
                let along_u = u_curves[k].evaluate(u)?.to_array();
                let along_v = v_curves[k].evaluate(v)?.to_array();
                for axis in 0..3 {
                    actual[axis] += bv[k] * (along_u[axis] - anchor[axis])
                        + bu[k] * (along_v[axis] - anchor[axis]);
                    for l in 0..3 {
                        actual[axis] -= bu[k] * bv[l] * (grid[k][l][axis] - anchor[axis]);
                    }
                }
            }
            maximum = maximum.max(Point3::try_from(actual)?.distance_to(surface.evaluate(u, v)?)?);
        }
    }
    Ok(
        json!({"grid":[65,65],"parameter_pairs":4225,"maximum_sampled_distance":maximum,
        "comparison":"Quadratic Gordon reconstruction from the saved tensor's own isocurves; sampled diagnostic, not a continuous proof or an independent NetworkSrf implementation."}),
    )
}

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

fn compare_queries(
    surface: &NurbsSurface,
    queries: &[Value],
    expected: &[Value],
    tolerance: Tolerance,
) -> Result<(f64, Vec<Value>), Box<dyn std::error::Error>> {
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
    Ok((maximum, rows))
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
    for (name, curves, mids, prepared) in [
        ("raw_gordon", &original, [0.5, 0.5], false),
        (
            "crossing_constrained_arc_length",
            &arc_curves,
            common,
            false,
        ),
        ("global_smooth_station_map", &smooth_curves, common, false),
        (
            "prepared_global_smooth_station_map",
            &smooth_curves,
            common,
            true,
        ),
    ] {
        let surface = if prepared {
            prepared_gordon_surface(
                &curves[..3],
                &curves[3..],
                &[0., mids[0], 1.],
                &[0., mids[1], 1.],
                tolerance,
                tolerance,
            )?
        } else {
            gordon_network_surface(curves, tolerance, tolerance)?
        };
        let mut original_bounds = vec![];
        for (index, source) in original.iter().enumerate() {
            let fixed = if index < 3 {
                [0., mids[1], 1.][index]
            } else {
                [0., mids[0], 1.][index - 3]
            };
            let endpoints = if index < 3 {
                [[0., fixed], [1., fixed]]
            } else {
                [[fixed, 0.], [fixed, 1.]]
            };
            let endpoints =
                endpoints.map(|p| viboceros_geometry::Point2::try_new(p[0], p[1]).unwrap());
            let result =
                surface.try_pullback_curve_certified_with_bound(source, Some(endpoints), tolerance);
            original_bounds.push(match result {
                Ok((uv,bound))=>json!({"source_curve_index":index,"certified":true,"complete_distance_bound":bound,"uv_control_count":uv.control_points().len()}),
                Err(error)=>json!({"source_curve_index":index,"certified":false,"error":error.to_string()}),
            });
        }
        let original_images = original_bounds.iter().all(|r| r["certified"] == true);
        let (maximum, rows) = compare_queries(&surface, queries, expected, tolerance)?;
        candidates.push(json!({"candidate":name,
            "curve_control_counts":curves.iter().map(|c|c.control_points().len()).collect::<Vec<_>>(),
            "surface_control_counts":[surface.control_point_count_u(),surface.control_point_count_v()],
            "surface_degree":[surface.degree_u(),surface.degree_v()],
            "maximum_interior_knot_multiplicities":[
                surface.knots_u().chunk_by(|a,b|a==b).filter(|k|k[0]>*surface.domain_u().start()&&k[0]<*surface.domain_u().end()).map(|k|k.len()).max().unwrap_or(0),
                surface.knots_v().chunk_by(|a,b|a==b).filter(|k|k[0]>*surface.domain_v().start()&&k[0]<*surface.domain_v().end()).map(|k|k.len()).max().unwrap_or(0)],
            "supplied_curve_images_certified":true,"pre_fit_original_curve_images_certified":original_images,
            "original_source_curve_bounds":original_bounds,
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
    let center = original[1].evaluate(original[1].parameter_at(0.5)?)?;
    let center = surface.closest_parameters(center, Tolerance::DEFAULT)?;
    let residual = sampled_gordon_residual(&surface, [center.0, center.1])?;
    // Reference-derived curves isolate construction from source-family fitting.
    // They never feed any of the independent candidates above.
    let u_stations = [0., center.0 / surface.domain_u().end(), 1.];
    let v_stations = [0., center.1 / surface.domain_v().end(), 1.];
    let u_curves = v_stations
        .map(|s| {
            surface
                .isocurve_u(s * surface.domain_v().end())?
                .try_reparameterized(0. ..=1.)
        })
        .into_iter()
        .collect::<Result<Vec<_>, viboceros_geometry::GeometryError>>()?;
    let v_curves = u_stations
        .map(|s| {
            surface
                .isocurve_v(s * surface.domain_u().end())?
                .try_reparameterized(0. ..=1.)
        })
        .into_iter()
        .collect::<Result<Vec<_>, viboceros_geometry::GeometryError>>()?;
    let reconstructed = prepared_gordon_surface(
        &u_curves,
        &v_curves,
        &u_stations,
        &v_stations,
        tolerance,
        tolerance,
    )?;
    let (reconstruction_maximum, reconstruction_queries) =
        compare_queries(&reconstructed, queries, expected, tolerance)?;
    let reconstruction = json!({"reference_derived_inputs":true,"qualifies_independent_networksrf":false,
        "supplied_curve_images_certified":true,"source_curve_count":6,
        "surface_control_counts":[reconstructed.control_point_count_u(),reconstructed.control_point_count_v()],
        "maximum_rhino_query_output_distance":reconstruction_maximum,"query_outputs_within_1e_minus_6":reconstruction_maximum<=1e-6,
        "queries":reconstruction_queries});
    let fitted_reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/network_curve_fit_rhino_20261016.json"
    ))?;
    let fitted_request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/network_curve_fit.json"
    ))?;
    let mut fitted_stations = vec![];
    for (i, row) in fitted_reference["results"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(row["id"], fitted_request["operations"][i]["id"]);
        let definition = &row["value"];
        let fitted = NurbsCurve::try_new_rational(
            definition["degree"].as_u64().unwrap() as usize,
            controls(definition),
            numbers(&definition["knots"]),
        )?;
        let source = &original[i % 6];
        let point = source.evaluate(source.parameter_at(0.5)?)?;
        let parameter = fitted.closest_parameter(point, Tolerance::DEFAULT)?;
        let domain = fitted.domain();
        fitted_stations.push(json!({"source_curve_index":i%6,"fit_tolerance":fitted_request["operations"][i]["fit_tolerance"],"fitted_control_count":fitted.control_points().len(),
            "normalized_midpoint_parameter":(parameter-domain.start())/(domain.end()-domain.start()),"midpoint_distance":fitted.evaluate(parameter)?.distance_to(point)?}));
    }
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
    let output = json!({"schema_version":3,"engine":"viboceros","engine_version":env!("CARGO_PKG_VERSION"),"fixture":"biquadratic_network",
        "qualification":"Diagnostic only; none of the candidates qualifies Rhino NetworkSrf interior fitting.",
        "comparison":"World-space returned closest-point outputs; not continuous surface separation or Hausdorff bounds.",
        "source_fit_error_policy":"Adaptive sampled fitting at 1e-7; exact stored-curve position constraints at 1e-9.",
        "new_licensed_rhino_captures":0,"licensed_curve_fit_reference_records":12,"reference_observation":"gordon_network_rhino_20261015.json",
        "curve_fit_reference_observation":"network_curve_fit_rhino_20261016.json",
        "saved_rhino_gordon_sampled_residual":residual,"saved_rhino_fit_midpoint_parameters":fitted_stations,
        "saved_reference_isocurve_reconstruction":reconstruction,
        "common_curve_fit_station_parameters":common,"candidates":candidates,"saved_rhino_tensor_parameter_traces":traces});
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    println!();
    Ok(())
}
