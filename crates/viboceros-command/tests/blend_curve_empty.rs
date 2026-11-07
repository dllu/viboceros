#[path = "support/blend_curve.rs"]
#[allow(dead_code)]
mod blend;
#[path = "support/blend_curve_empty.rs"]
mod empty;
#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/blend_curve_measurements.rs"]
#[allow(dead_code)]
mod measurements;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/blend_curve_points.rs"]
#[allow(dead_code)]
mod points;
#[path = "support/blend_curve_portions.rs"]
#[allow(dead_code)]
mod portions;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;
use viboceros_command::{CommandRegistry, blend_curve::Input};

#[test]
fn empty_retained_intervals_match_native_geometry_ownership_and_history() {
    let mut count = 0;
    for family in empty::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let mut doc = reference_inputs::setup(value, &value["prescribed_case"]);
            doc.clear_selection();
            let before = empty::State::capture(&doc, value);
            let mut input = Input::with_preferences(doc.tolerance(), points::preferences(value));
            portions::replay(&mut input, &doc, value);
            if blend::accepted(value) {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            let mut metrics = before.verify(&mut doc, value);
            metrics["id"] = serde_json::json!(format!("{family}/{}", row["id"].as_str().unwrap()));
            println!("BLEND_CURVE_EMPTY_COMMAND {metrics}");
            count += 1;
        }
    }
    assert_eq!(count, empty::CASES);
}
