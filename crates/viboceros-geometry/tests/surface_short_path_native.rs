//! Native captures are immutable evidence, including Rhino's nonminimal pole
//! path and approximation error. Geometry correctness uses independent gates.
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point2, Point3, Tolerance, WeightedPoint3};
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
fn captured_short_paths_keep_native_discrepancies_visible() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_short_path.json"
    ))
    .unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap();
    for row in capture["results"].as_array().unwrap() {
        let v = &row["value"];
        let source = surface(&v["surface"]);
        let before = source.clone();
        let start =
            Point2::try_from(serde_json::from_value::<[f64; 2]>(v["start"].clone()).unwrap())
                .unwrap();
        let end = Point2::try_from(serde_json::from_value::<[f64; 2]>(v["end"].clone()).unwrap())
            .unwrap();
        let result =
            viboceros_geometry::try_short_path_on_surface(&source, start, end, tolerance, None)
                .unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
        assert_eq!(source, before);
        assert!(result.surface_image_bound <= tolerance.absolute() * 0.5);
        for (uv, t) in [
            (start, *result.curve.domain().start()),
            (end, *result.curve.domain().end()),
        ] {
            assert!(
                source
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .distance_to(result.curve.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-8,
                "{} endpoint",
                row["id"]
            );
        }
        let native = curve(&v["curve"]);
        let mut maximum = 0_f64;
        for (a, b) in [(&result.curve, &native), (&native, &result.curve)] {
            for i in 0..=64 {
                let p = a.evaluate(a.parameter_at(i as f64 / 64.).unwrap()).unwrap();
                let t = b
                    .closest_parameter(p, Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap())
                    .unwrap();
                maximum = maximum.max(p.distance_to(b.evaluate(t).unwrap()).unwrap());
            }
        }
        println!(
            "SURFACE_SHORT_PATH {}",
            serde_json::json!({"id":row["id"],"distance":result.distance,"native_length":v["length"],"surface_image_bound":result.surface_image_bound,"maximum_bidirectional_sample_error":maximum,"geodesic_queries":result.geodesic_queries,"qualified":maximum<=1e-6})
        );
        let expected = matches!(
            row["id"].as_str().unwrap(),
            "plane" | "skew_plane" | "sphere_antipodal" | "coincident"
        );
        assert_eq!(
            maximum < 1e-6,
            expected,
            "{} native qualification changed",
            row["id"]
        );
        assert!(maximum < 0.03);
    }
}

#[test]
fn native_cylinder_discrepancy_has_an_independent_unrolled_witness() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/surface_short_path.json"
    ))
    .unwrap();
    let v = &capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "cylinder")
        .unwrap()["value"];
    let native = curve(&v["curve"]);
    let a = v["points"][0].as_array().unwrap();
    let b = v["points"][1].as_array().unwrap();
    let theta = a[1].as_f64().unwrap().atan2(a[0].as_f64().unwrap());
    let end = b[1].as_f64().unwrap().atan2(b[0].as_f64().unwrap());
    let mut maximum = 0_f64;
    for i in 0..=256 {
        let p = native
            .evaluate(native.parameter_at(i as f64 / 256.).unwrap())
            .unwrap();
        let t =
            (p.z() - a[2].as_f64().unwrap()) / (b[2].as_f64().unwrap() - a[2].as_f64().unwrap());
        let phi = theta + t * (end - theta);
        maximum = maximum.max(
            p.distance_to(Point3::try_new(3. * phi.cos(), 3. * phi.sin(), p.z()).unwrap())
                .unwrap(),
        );
    }
    println!("SHORT_PATH_CYLINDER_UNROLLED_WITNESS {maximum:e}");
    assert!(maximum > 5e-4 && maximum < 6e-4);
}
