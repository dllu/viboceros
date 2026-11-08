//! Replay accepted values from authored actions, never native output coefficients.
use super::*;
use viboceros_command::offset_surface;
use viboceros_geometry::LengthUnitSystem;

pub fn initial(v: &Value) -> Options {
    let prompt = v["input_trace"][0]["prompts"][0].as_str().unwrap();
    let mut options = Options::default();
    let value = |key: &str| {
        prompt
            .split(&format!("{key}="))
            .nth(1)
            .map(|s| s.split_whitespace().next().unwrap().trim_end_matches(')'))
    };
    for key in ["Distance", "Solid", "BothSides", "DeleteInput"] {
        if let Some(value) = value(key) {
            options = offset_surface::parse(
                &[&format!("{key}={value}")],
                options,
                &LengthUnitSystem::Millimeters,
            )
            .unwrap()
            .options;
        }
    }
    options
}
pub fn command(v: &Value) -> String {
    let mut options = initial(v);
    let mut through = false;
    let mut through_point = None;
    let mut start: Option<Point3> = None;
    let mut distance_menu = false;
    for action in v["prescribed_case"]["actions"].as_array().unwrap() {
        if action == "Enter" {
            if distance_menu && options.distance >= 0. {
                if options.distance == 0. {
                    options.distance = 1.;
                }
                distance_menu = false;
            }
            continue;
        }
        if action == "Cancel" {
            distance_menu = false;
            continue;
        }
        if action == "Distance" {
            distance_menu = true;
            start = None;
            continue;
        }
        if action == "ThroughPoint" {
            through = true;
            continue;
        }
        if action.get("curve").is_some() {
            continue;
        }
        let word = if action.get("click").is_some() {
            "FlipAll".to_owned()
        } else if action.is_array() {
            let point = native::point(action);
            if through {
                through_point = Some(point);
                continue;
            }
            if let Some(first) = start.take() {
                first.distance_to(point).unwrap().to_string()
            } else {
                start = Some(point);
                continue;
            }
        } else if let Some(number) = action.get("number").and_then(Value::as_str) {
            number.to_owned()
        } else if let Some(number) = action.as_f64() {
            number.to_string()
        } else {
            action.as_str().unwrap().to_owned()
        };
        if distance_menu
            && let Ok(value) = offset_surface::distance(&word, &LengthUnitSystem::Millimeters)
        {
            if value < 0. {
                continue;
            }
            if value == 0. {
                distance_menu = false;
                continue;
            }
        }
        options = offset_surface::parse(&[&word], options, &LengthUnitSystem::Millimeters)
            .unwrap()
            .options;
        if offset_surface::distance(&word, &LengthUnitSystem::Millimeters).is_ok() {
            distance_menu = false;
        }
    }
    let mut text = format!(
        "OffsetSrf {} FlipAll={}",
        options.arguments(),
        if options.flip_all { "Yes" } else { "No" }
    );
    if let Some(point) = through_point {
        text.push_str(&format!(
            " ThroughPoint={}",
            point.to_array().map(|v| v.to_string()).join(",")
        ));
    }
    text
}
