//! Replay fixed input recipes; native arc measurements are expected data only.
use super::{inputs, native, reference_inputs};
use serde_json::Value;
use viboceros_command::arc_blend::Input;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{CurveRef, CurveSegment3};
pub const FAMILIES: [&str; 3] = [
    "arc_blend_radius_targets",
    "arc_blend_radius_parallel",
    "arc_blend_radius_recall",
];
pub fn diagnostic(id: &str) -> Option<&'static str> {
    if ["rd_corner_", "rd_skew_", "rd_spatial_"]
        .iter()
        .any(|p| id.starts_with(p))
    {
        Some("general tangent radius solver remains unsupported")
    } else if ["rd_parallel_end_zero", "rd_parallel_typed_zero"].contains(&id) {
        Some("target inherited from a previous command remains unsupported")
    } else {
        None
    }
}
pub fn replay(input: &mut Input, doc: &Document, v: &Value) {
    let mut radius = false;
    for (index, a) in v["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if let Some(i) = a["curve"].as_u64() {
            input
                .pick_curve(
                    doc,
                    reference_inputs::guide(doc, i as usize),
                    None,
                    native::point(&a["pick"]),
                )
                .unwrap();
        } else if a == "RadiusDifference" {
            radius = true;
        } else if let Some(p) = a.get("click") {
            if input.active_handle().is_some() {
                input
                    .adjust_parameter(inputs::parameter(v, index, input))
                    .unwrap();
                input.finish_adjustment();
            } else {
                input
                    .select_handle(inputs::handle(input, native::point(p)))
                    .unwrap();
            }
        } else if a.is_array() {
            input.adjust_point(native::point(a)).unwrap();
            input.finish_adjustment();
        } else if a.is_number() {
            assert!(
                input
                    .option(&format!(
                        "{}={a}",
                        if radius { "RadiusDifference" } else { "Ratio" }
                    ))
                    .unwrap()
            );
            radius = false;
            input.finish_adjustment();
        } else if a != "Enter" {
            assert!(input.option(a.as_str().unwrap()).unwrap());
        }
    }
    assert!(!radius);
}
pub fn independent_samples(doc: &Document, v: &Value) -> f64 {
    let object = doc
        .objects()
        .find(|o| o.attributes().name().is_none())
        .unwrap();
    let Geometry::PolyCurve(curve) = object.geometry() else {
        panic!("expected analytic biarc")
    };
    let target = v["after"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("arc_segments").is_some())
        .unwrap();
    let mut error = samples(CurveRef::PolyCurve(curve), target);
    assert_eq!(curve.segments().len(), 2);
    for (actual, expected) in curve
        .segments()
        .iter()
        .zip(target["arc_segments"].as_array().unwrap())
    {
        let CurveSegment3::Arc(arc) = actual else {
            panic!("expected circular arc")
        };
        error = error.max((arc.radius() - expected["radius"].as_f64().unwrap()).abs());
        error = error.max(samples(CurveRef::Arc(arc), expected));
    }
    error
}
pub fn samples(curve: CurveRef<'_>, expected: &Value) -> f64 {
    let rows = &expected["samples"];
    let domain = &expected["curve"]["domain"];
    let mut error = 0_f64;
    assert_eq!(rows.as_array().unwrap().len(), 33);
    for r in rows.as_array().unwrap() {
        let t = r["parameter"].as_f64().unwrap();
        // Native SegmentCurve domains use the outer PolyCurve interval, while
        // our analytic leaves retain natural domains. Compare the same local
        // station; full NURBS domains are checked independently.
        let a = ((t - domain[0].as_f64().unwrap())
            / (domain[1].as_f64().unwrap() - domain[0].as_f64().unwrap()))
        .clamp(0., 1.);
        let t = *curve.domain().start() + a * (*curve.domain().end() - *curve.domain().start());
        let jet = curve.evaluate_with_tangent(t).unwrap();
        error = error.max(jet.point().distance_to(native::point(&r["point"])).unwrap());
        for (a, b) in jet
            .tangent()
            .as_vector()
            .to_array()
            .iter()
            .zip(r["tangent"].as_array().unwrap())
        {
            error = error.max((a - b.as_f64().unwrap()).abs());
        }
    }
    error
}
