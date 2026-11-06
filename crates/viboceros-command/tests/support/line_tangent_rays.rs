//! Independently prescribed guides and public viewing-ray input calibration.
#[allow(unused_imports)]
pub use super::tangent_inputs::{click_point, command, guide, pick, setup, token};
use serde_json::Value;
pub const FAMILIES: [&str; 2] = ["line_tangent_rays", "line_tangent_mouse"];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_tangent_rays_sources.json"
    ))
    .unwrap()
}
pub fn excluded(id: &str) -> bool {
    [
        "ray_fixed_elevated",
        "ray_elevated_top",
        "ray_elevated_far",
        "ray_vertical_front",
        "ray_tilted_perspective",
        "ray_cubic_top",
        "ray_cubic_perspective",
        "ray_signed_length",
        "ray_bothsides",
    ]
    .contains(&id)
}

/// A viewing line parallel to the CPlane has no free 3D endpoint to submit.
pub fn mouse_point(
    v: &Value,
    index: usize,
    input: &viboceros_command::line::curve_input::Input,
) -> Option<viboceros_geometry::Point3> {
    let click = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"].as_u64() == Some(index as u64))
        .unwrap();
    if click["projected_point"].is_null() {
        let origin = super::native::point(&click["ray"][0]);
        let direction = origin
            .vector_to(super::native::point(&click["ray"][1]))
            .unwrap();
        assert!(
            direction
                .dot(input.plane().z_axis().as_vector())
                .unwrap()
                .abs()
                <= 1e-12 * direction.length().unwrap()
        );
        None
    } else {
        Some(click_point(v, index, input))
    }
}
