//! Surface-distance shots compared with independently captured OffsetOnSurface.
use serde_json::Value;
use viboceros_geometry::{
    NurbsCurve, NurbsSurface, Point2, Point3, PreparedSurfaceGeodesic, Tolerance, WeightedPoint3,
};
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                Point3::try_from(serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap())
                    .unwrap(),
                p["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
#[test]
fn offset_qualification_keeps_known_native_differences_explicit() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_curve_offset.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_curve_offset.json"
    ))
    .unwrap();
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-9).unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 24);
    assert_eq!(r["results"].as_array().unwrap().len(), 24);
    let mut max: f64 = 0.;
    let mut failures = Vec::new();
    let mut shots = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        assert!(row["error"].is_null());
        let value = &row["value"];
        assert_eq!(value["source_unchanged"], true);
        let s = surface(&value["surface"]);
        let c = curve(&value["curve"]);
        let prepared = PreparedSurfaceGeodesic::new(&s).unwrap();
        let outputs = value["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| curve(&o["definition"]))
            .collect::<Vec<_>>();
        let mut error: f64 = 0.;
        let mut local_failures = Vec::new();
        let mut candidates = Vec::new();
        let mut boundary_rejections = 0;
        for i in 1..8 {
            let domain = c.domain();
            let t = domain.start() + i as f64 / 8. * (domain.end() - domain.start());
            let sample = viboceros_geometry::CurveRef::NurbsCurve(&c)
                .evaluate_with_tangent(t)
                .unwrap();
            let uv = match op["case"].as_str().unwrap() {
                "cylinder_axis" => (0.6, sample.point().z()),
                "cylinder_ring" => (t, 3.),
                "sphere_latitude" => (t, 0.2),
                "warped_iso" => (t, 0.5),
                _ => s.closest_parameters(sample.point(), tolerance).unwrap(),
            };
            assert!(
                s.evaluate(uv.0, uv.1)
                    .unwrap()
                    .distance_to(sample.point())
                    .unwrap()
                    < 1e-10
            );
            let direction = s
                .normal_at(uv.0, uv.1)
                .unwrap()
                .as_vector()
                .cross(sample.tangent().as_vector())
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            let result = prepared.point(
                Point2::try_new(uv.0, uv.1).unwrap(),
                direction,
                op["distance"].as_f64().unwrap(),
                tolerance,
            );
            shots += 1;
            match result {
                Ok(p) => {
                    if outputs.is_empty() {
                        local_failures.push(format!("unexpected station {i}"));
                        continue;
                    }
                    let distance = outputs
                        .iter()
                        .map(|c| {
                            let t = c.closest_parameter(p.point, tolerance).unwrap();
                            c.evaluate(t).unwrap().distance_to(p.point).unwrap()
                        })
                        .fold(f64::INFINITY, f64::min);
                    error = error.max(distance);
                    candidates.push(serde_json::json!({"point":p.point.to_array(),"nearest_distance":distance,"station":i}));
                }
                Err(e) => {
                    if outputs.is_empty()
                        && matches!(
                            e,
                            viboceros_geometry::GeometryError::ParameterCurveOutsideSurfaceDomain
                        )
                    {
                        boundary_rejections += 1;
                    } else {
                        local_failures.push(format!("station {i}: {e}"));
                    }
                }
            }
        }
        println!(
            "SURFACE_GEODESIC {}",
            serde_json::json!({"id":op["id"],"maximum_error":error,"qualified":error<1e-6 && local_failures.is_empty(),"boundary_rejections":boundary_rejections,"errors":local_failures,"candidates":candidates})
        );
        assert!(local_failures.is_empty(), "{local_failures:?}");
        assert_eq!(boundary_rejections, if outputs.is_empty() { 7 } else { 0 });
        // These are bounded diagnostic ceilings, not a relaxed parity gate.
        // The five cases remain unqualified against the unchanged 1e-6 gate.
        let diagnostic_ceiling = match op["id"].as_str().unwrap() {
            "plane_curve_2" | "plane_curve_3" => 3.2e-6,
            "warped_iso_0" | "warped_iso_1" => 2.7e-5,
            "warped_iso_2" => 1.05e-4,
            _ => 1e-6,
        };
        assert!(
            error <= diagnostic_ceiling,
            "unexpected deterioration for {}: {error}",
            op["id"]
        );
        if error >= 1e-6 || !local_failures.is_empty() {
            failures.push(op["id"].clone());
        }
        max = max.max(error);
    }
    println!("surface geodesic shots={shots} maximum_error={max}");
    assert_eq!(shots, 168);
    assert_eq!(
        failures,
        vec![
            "plane_curve_2",
            "plane_curve_3",
            "warped_iso_0",
            "warped_iso_1",
            "warped_iso_2"
        ]
    );
}

#[test]
fn independent_sdk_closest_points_confirm_the_measured_differences() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_curve_offset_verify.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_curve_offset_verify.json"
    ))
    .unwrap();
    let tolerance = Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        assert!(row["error"].is_null());
        let value = &row["value"];
        let outputs = value["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| curve(&o["definition"]))
            .collect::<Vec<_>>();
        assert!(!outputs.is_empty());
        for (point, expected) in op["samples"]
            .as_array()
            .unwrap()
            .iter()
            .zip(value["sample_distances"].as_array().unwrap())
        {
            let point =
                Point3::try_from(serde_json::from_value::<[f64; 3]>(point.clone()).unwrap())
                    .unwrap();
            let distance = outputs
                .iter()
                .map(|c| {
                    let t = c.closest_parameter(point, tolerance).unwrap();
                    c.evaluate(t).unwrap().distance_to(point).unwrap()
                })
                .fold(f64::INFINITY, f64::min);
            assert!((distance - expected.as_f64().unwrap()).abs() < 1e-10);
            count += 1;
        }
    }
    assert_eq!(count, 42);
}

#[test]
fn clipped_offset_curves_keep_native_locus_differences_explicit() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_curve_offset.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_curve_offset.json"
    ))
    .unwrap();
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-9).unwrap();
    let mut unqualified = Vec::new();
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let source = curve(&row["value"]["curve"]);
        let surface = surface(&row["value"]["surface"]);
        let native = row["value"]["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| curve(&o["definition"]))
            .collect::<Vec<_>>();
        let fitted = viboceros_geometry::try_offset_curves_on_surface(
            &surface,
            &source,
            op["distance"].as_f64().unwrap(),
            tolerance,
        )
        .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert_eq!(fitted.curves.len(), native.len(), "{}", op["id"]);
        let mut maximum: f64 = 0.;
        for (sources, targets) in [(&fitted.curves, &native), (&native, &fitted.curves)] {
            for c in sources {
                for i in 0..=64 {
                    let p = c.evaluate(c.parameter_at(i as f64 / 64.).unwrap()).unwrap();
                    let error = targets
                        .iter()
                        .map(|t| {
                            let u = t.closest_parameter(p, tolerance).unwrap();
                            t.evaluate(u).unwrap().distance_to(p).unwrap()
                        })
                        .fold(f64::INFINITY, f64::min);
                    maximum = maximum.max(error);
                }
            }
        }
        if op["id"] == "warped_iso_1" {
            assert!(fitted.extension_attempts > 0);
            let c = &fitted.curves[0];
            assert!(
                c.domain().start() > source.domain().start()
                    && c.domain().end() < source.domain().end()
            );
            assert!(
                (c.evaluate(*c.domain().start()).unwrap().x() + 5.).abs() < tolerance.absolute()
            );
            assert!((c.evaluate(*c.domain().end()).unwrap().x() - 5.).abs() < tolerance.absolute());
        }
        println!(
            "SURFACE_CURVE_OFFSET {}",
            serde_json::json!({"id":op["id"],"status":if fitted.curves.is_empty(){"empty"}else{"curve"},"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"native_outputs":native.len(),"curves":fitted.curves.len(),"queries":fitted.geodesic_queries,"controls":fitted.curves.iter().map(|c|c.control_points().len()).sum::<usize>(),"extension_attempts":fitted.extension_attempts,"source_intervals":fitted.curves.iter().map(|c|[*c.domain().start(),*c.domain().end()]).collect::<Vec<_>>()})
        );
        if maximum >= 1e-6 {
            unqualified.push(op["id"].clone());
        }
        let ceiling = match op["id"].as_str().unwrap() {
            "plane_curve_2" | "plane_curve_3" => 4e-6,
            "warped_iso_0" | "warped_iso_1" => 2.7e-5,
            "warped_iso_2" => 1.05e-4,
            _ => 1e-6,
        };
        assert!(maximum < ceiling, "{}: {maximum}", op["id"]);
    }
    // Boundary clipping now agrees structurally, while the original five
    // numerical gaps remain unqualified against the unchanged 1e-6 gate.
    assert_eq!(
        unqualified,
        vec![
            "plane_curve_2",
            "plane_curve_3",
            "warped_iso_0",
            "warped_iso_1",
            "warped_iso_2"
        ]
    );
}

#[test]
fn independently_captured_clipping_and_closed_gap_policies_match_the_sdk() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/surface_curve_offset_clipping.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_curve_offset_clipping.json"
    ))
    .unwrap();
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-9).unwrap();
    let expected_open = [4, 1, 1, 1, 0];
    let expected_connectors = [4, 0, 1, 0, 0];
    let mut count = 0;
    for (index, (op, row)) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .enumerate()
    {
        assert_eq!(op["id"], row["id"]);
        assert!(row["error"].is_null());
        assert_eq!(row["value"]["source_unchanged"], true);
        let source = curve(&row["value"]["curve"]);
        let surface = surface(&row["value"]["surface"]);
        let native = row["value"]["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| curve(&o["definition"]))
            .collect::<Vec<_>>();
        let distance = op["distance"].as_f64().unwrap();
        let open = viboceros_geometry::try_offset_curves_on_surface(
            &surface, &source, distance, tolerance,
        )
        .unwrap();
        assert_eq!(open.curves.len(), expected_open[index]);
        let connected = viboceros_geometry::try_offset_curves_on_surface_with_options(
            &surface,
            &source,
            distance,
            tolerance,
            viboceros_geometry::SurfaceCurveOffsetOptions {
                closed_gaps: viboceros_geometry::SurfaceCurveOffsetGapStyle::ConnectClosed,
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert_eq!(connected.curves.len(), native.len());
        assert_eq!(connected.connectors, expected_connectors[index]);
        let mut maximum: f64 = 0.;
        for (sources, targets) in [(&connected.curves, &native), (&native, &connected.curves)] {
            for c in sources {
                for i in 0..=128 {
                    let p = c
                        .evaluate(c.parameter_at(i as f64 / 128.).unwrap())
                        .unwrap();
                    let error = targets
                        .iter()
                        .map(|c| {
                            let t = c.closest_parameter(p, tolerance).unwrap();
                            p.distance_to(c.evaluate(t).unwrap()).unwrap()
                        })
                        .fold(f64::INFINITY, f64::min);
                    maximum = maximum.max(error);
                }
            }
        }
        println!(
            "SURFACE_OFFSET_CLIPPING {}",
            serde_json::json!({"id":op["id"],"open_pieces":open.curves.len(),"sdk_curves":connected.curves.len(),"connectors":connected.connectors,"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6})
        );
        assert!(maximum < 1e-6, "{}: {maximum}", op["id"]);
        count += 1;
    }
    assert_eq!(count, 5);
}
