//! Compare typed coordinate resolution with Rhino's actual Polyline prompt.

use super::*;
use viboceros_drafting::PointInput;

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_fixture_checks_world_and_rotated_plane_point_sequences() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_input.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 19);
        for (operation, result) in request.operations.iter().zip(&response.results) {
            let crate::Operation::PointInput { fixture, .. } = operation else {
                panic!("point input");
            };
            assert_eq!(
                result.value["points"].as_array().unwrap().len(),
                fixture.points.len()
            );
            assert_eq!(result.value["points"][0], serde_json::json!(fixture.origin));
            assert_eq!(result.elapsed_ns, 0);
        }
        let front = &response
            .results
            .iter()
            .find(|r| r.id == "point-input-front-cartesian")
            .unwrap()
            .value["points"];
        assert_eq!(front[1], serde_json::json!([1.0, 0.0, 2.0]));
        assert_eq!(front[2], serde_json::json!([4.0, -5.0, 6.0]));
        assert_eq!(front[7], serde_json::json!([-4.0, 5.0, 9.0]));
        let diagnostic: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_input_diagnostics.json"
        ))
        .unwrap();
        let diagnostic = crate::run_request(&diagnostic).unwrap();
        assert_eq!(diagnostic.results.len(), 1);
        assert_eq!(
            diagnostic.results[0].value["points"][2],
            serde_json::json!([0.0, 2e16, 0.0])
        );
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointInputFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub points: Vec<String>,
}

pub(super) fn run(
    fixture: &PointInputFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(2..=256).contains(&fixture.points.len()) {
        return Err(ProbeError::FixtureInvariant(
            "point input requires 2–256 points",
        ));
    }
    let plane = Frame3::try_from_directions(
        Point3::try_from(fixture.origin)?,
        Vector3::try_from(fixture.x_axis)?,
        Vector3::try_from(fixture.y_axis)?,
        tolerance,
    )?;
    let mut points = Vec::with_capacity(fixture.points.len());
    for token in &fixture.points {
        let input = PointInput::parse(token)
            .ok_or(ProbeError::FixtureInvariant("expected a point token"))?
            .map_err(|_| ProbeError::FixtureInvariant("invalid point syntax"))?;
        let point = input
            .resolve(plane, points.last().copied())
            .map_err(|_| ProbeError::FixtureInvariant("point could not be resolved"))?;
        points.push(point);
    }
    let polyline = Polyline3::try_new(points, tolerance)?;
    Ok((
        json!({"points": polyline.vertices().iter().map(|p| p.to_array()).collect::<Vec<_>>()}),
        0,
    ))
}
