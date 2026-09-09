//! Multiple point placement and command-local Undo observations.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointsFixture {
    pub events: Vec<Event>,
    #[serde(default)]
    pub cancel: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Event {
    Point([f64; 3]),
    Action(String),
}

pub(super) fn run(f: &PointsFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if f.events.len() > 100 {
        return Err(ProbeError::FixtureInvariant("too many Points events"));
    }
    let mut command = String::from("Points");
    for event in &f.events {
        match event {
            Event::Point(p) => {
                Point3::try_from(*p)?;
                command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
            }
            Event::Action(action) if action == "undo" => command.push_str(" Undo"),
            _ => return Err(ProbeError::FixtureInvariant("invalid Points event")),
        }
    }
    // Both Enter and Escape retain accepted points in the measured Rhino path.
    // App tests separately verify interactive cancellation; this probe executes
    // the same final accepted point sequence through the typed command.
    let mut document = Document::new(tolerance);
    CommandRegistry::with_builtins().execute(&mut document, &command)?;
    let after = document
        .objects()
        .map(|o| {
            let Geometry::Point(p) = o.geometry() else {
                unreachable!("Points output")
            };
            json!({"point":p.to_array(),"selected":document.is_selected(o.id())})
        })
        .collect::<Vec<_>>();
    Ok((json!({"after":after}), 0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn points_fixture_matches_measured_accepted_points() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/points_command.json"
        ))
        .unwrap();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/points_command.json"
        ))
        .unwrap();
        let actual = crate::run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 7);
        for result in actual.results {
            let expected = reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == result.id)
                .unwrap();
            // The initial raw probe also attempted outer Undo/Redo inside
            // RunPythonScript. Those did not undo Points and are not parity
            // evidence for normal document history; compare accepted points.
            assert_eq!(
                result.value["after"], expected["value"]["after"],
                "{}",
                result.id
            );
        }
    }
}
