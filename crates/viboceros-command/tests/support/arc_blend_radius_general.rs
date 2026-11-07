use super::{native, radius};
use serde_json::Value;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{CurveRef, CurveSegment3, Vector3};
pub fn representation_diagnostic(id: &str) -> bool {
    ["rg_cocircular_1", "rg_cocircular_2"].contains(&id)
}
pub fn coincident_circle_error(doc: &Document, value: &Value) -> f64 {
    let object = doc
        .objects()
        .find(|o| o.attributes().name().is_none())
        .unwrap();
    let Geometry::PolyCurve(curve) = object.geometry() else {
        panic!("expected analytic biarc")
    };
    let target = value["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("arc_segments").is_some())
        .unwrap();
    let arcs = target["arc_segments"].as_array().unwrap();
    let center = native::point(&arcs[0]["center"]);
    let r = arcs[0]["radius"].as_f64().unwrap();
    let start = native::point(&arcs[0]["samples"][0]["point"]);
    let x = center.direction_to(start).unwrap();
    let t = Vector3::try_from(native::point(&arcs[0]["samples"][0]["tangent"]).to_array()).unwrap();
    let normal = x
        .as_vector()
        .cross(t)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let mut error = radius::samples(CurveRef::PolyCurve(curve), target);
    let mut sweep = 0.;
    let mut expected_sweep = 0.;
    for a in arcs {
        let native_center = native::point(&a["center"]);
        let radial = native_center
            .direction_to(native::point(&a["samples"][0]["point"]))
            .unwrap();
        let tangent =
            Vector3::try_from(native::point(&a["samples"][0]["tangent"]).to_array()).unwrap();
        let n = radial
            .as_vector()
            .cross(tangent)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let normal_error = n
            .as_vector()
            .to_array()
            .iter()
            .zip(normal.as_vector().to_array())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        let support_error = center.distance_to(native_center).unwrap()
            + (r - a["radius"].as_f64().unwrap()).abs()
            + r * normal_error;
        assert!(support_error < 1e-9);
        error = error.max(support_error);
        expected_sweep += a["angles"][1].as_f64().unwrap() - a["angles"][0].as_f64().unwrap();
    }
    for segment in curve.segments() {
        let CurveSegment3::Arc(arc) = segment else {
            panic!("expected circle pieces")
        };
        let normal_error = arc
            .normal()
            .unwrap()
            .as_vector()
            .to_array()
            .iter()
            .zip(normal.as_vector().to_array())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        // Circle support error bounds every point, rather than only the finite
        // samples. The two native arcs must share that same supporting circle.
        error = error.max(
            arc.center().distance_to(center).unwrap() + (arc.radius() - r).abs() + r * normal_error,
        );
        sweep += arc.sweep_radians();
    }
    error = error.max(r * (sweep - expected_sweep).abs());
    assert!(sweep < std::f64::consts::PI && expected_sweep < std::f64::consts::PI);
    let actual_start = curve.evaluate(*curve.domain().start()).unwrap();
    // Four circle pieces, start phase, sweep and plane errors contribute to
    // the bound. Reserve a conservative budget rather than taking their max
    // as if the independent errors could not add.
    16. * error.max(actual_start.distance_to(start).unwrap())
}
pub const FAMILIES: [&str; 4] = [
    "arc_blend_radius_targets",
    "arc_blend_radius_general",
    "arc_blend_radius_general_holdouts",
    "arc_blend_radius_cocircular",
];
pub fn final_warning(value: &Value) -> bool {
    // A contact edit can resolve an earlier failed target. Native history
    // retains that earlier warning, so inspect the final acknowledged getter.
    let lines = value["history"]
        .as_str()
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    let start = lines
        .iter()
        .rposition(|s| {
            s.starts_with("Enter target radius difference")
                || s.starts_with("Select start point location:")
                || s.starts_with("Select end point location:")
        })
        .unwrap();
    lines[start..]
        .iter()
        .any(|s| s.contains("Unable to achieve requested radius difference"))
}
