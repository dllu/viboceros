//! Independently captured UV interpolation, including fixed failure cases.
use serde_json::Value;
use viboceros_geometry::{
    NurbsCurve, NurbsSurface, Point2, Point3, Tolerance, WeightedPoint3,
    try_interpolate_curve_on_surface,
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
#[test]
fn surface_interpolation_native_images_and_locations() {
    replay(include_str!(
        "../../../tools/rhino_oracle/observations/surface_interpolate.json"
    ));
}
#[test]
fn surface_interpolation_closed_winding_and_open_boundary_overshoot() {
    replay(include_str!(
        "../../../tools/rhino_oracle/observations/surface_interpolate_closed.json"
    ));
}
fn replay(data: &str) {
    let q: Value = serde_json::from_str(data).unwrap();
    let tol = Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap();
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let s = &v["surface"];
        let surface = NurbsSurface::try_new_rational(
            s["degree"][0].as_u64().unwrap() as usize,
            s["degree"][1].as_u64().unwrap() as usize,
            s["control_count"][0].as_u64().unwrap() as usize,
            s["control_count"][1].as_u64().unwrap() as usize,
            controls(&s["control_points"]),
            serde_json::from_value(s["knots_u"].clone()).unwrap(),
            serde_json::from_value(s["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        let before = surface.clone();
        let points = v["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                Point2::try_from(serde_json::from_value::<[f64; 2]>(p.clone()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        let result = try_interpolate_curve_on_surface(
            &surface,
            &points,
            v["closed"].as_bool().unwrap(),
            tol,
        );
        assert_eq!(surface, before);
        if v["interpolants"]["uv"]["curve"].is_null() {
            assert!(result.is_err());
            println!(
                "SURFACE_INTERPOLATION {}",
                serde_json::json!({"id":row["id"],"expected_failure":true,"qualified":true})
            );
            continue;
        }
        let result = result.unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
        assert!(result.surface_image_bound <= tol.absolute() * 0.5);
        let native = curve(&v["interpolants"]["uv"]["curve"]);
        let mut maximum = 0_f64;
        for (from, to) in [(&result.curve, &native), (&native, &result.curve)] {
            for i in 0..=64 {
                let p = from
                    .evaluate(from.parameter_at(i as f64 / 64.).unwrap())
                    .unwrap();
                let t = to
                    .closest_parameter(p, Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap())
                    .unwrap();
                maximum = maximum.max(p.distance_to(to.evaluate(t).unwrap()).unwrap());
            }
        }
        let mut node_error = 0_f64;
        for point in &points {
            let p = surface.evaluate(point.x(), point.y()).unwrap();
            let t = result.curve.closest_parameter(p, tol).unwrap();
            node_error = node_error.max(p.distance_to(result.curve.evaluate(t).unwrap()).unwrap());
        }
        println!(
            "SURFACE_INTERPOLATION {}",
            serde_json::json!({"id":row["id"],"maximum_bidirectional_sample_error":maximum,"surface_image_bound":result.surface_image_bound,"maximum_node_error":node_error,"seams":result.seam_count,"degree":result.curve.degree(),"controls":result.curve.control_points().len(),"extended_support":result.extended_support.is_some(),"qualified":maximum<1e-6})
        );
        assert!(maximum < 1e-6, "{} native error {maximum:e}", row["id"]);
        assert!(
            node_error < 1e-6,
            "{} interpolation locations {node_error:e}",
            row["id"]
        );
    }
}
