//! CPlane history and basis probes through the application's command parser.
use super::*;
use viboceros_command::construction_plane::{self as cplane, ConstructionPlaneState, PlaneAction};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionPlaneFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub steps: Vec<PlaneStep>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaneStep {
    World { view: String },
    Origin { point: [f64; 3] },
    ThreePoint { points: [[f64; 3]; 3] },
    ThreePointInput { points: [String; 3] },
    OriginInput { point: String },
    Elevation { distance: f64 },
    Through { point: [f64; 3] },
    Rotate { axis: [[f64; 3]; 2], angle: f64 },
    Undo,
    Redo,
}

pub(super) fn run(
    f: &ConstructionPlaneFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.steps.is_empty() || f.steps.len() > 128 {
        return Err(ProbeError::FixtureInvariant(
            "expected 1 to 128 CPlane steps",
        ));
    }
    let frame = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let mut state = ConstructionPlaneState::new(frame);
    let record = |state: &ConstructionPlaneState| json!({"origin":state.frame().origin().to_array(), "axes":state.frame().axes().map(|a| a.as_vector().to_array())});
    let mut records = vec![record(&state)];
    for step in &f.steps {
        apply_step(&mut state, step, None, tolerance)?;
        records.push(record(&state));
    }
    Ok((json!({"states":records}), 0))
}

fn apply_step(
    state: &mut ConstructionPlaneState,
    step: &PlaneStep,
    previous: Option<Point3>,
    tolerance: Tolerance,
) -> Result<(), ProbeError> {
    let point = |p: &[f64; 3]| format!("w{},{},{}", p[0], p[1], p[2]);
    let command = match step {
        PlaneStep::World { view } => format!("CPlane World {view}"),
        PlaneStep::Origin { point: p } => format!("CPlane {}", point(p)),
        PlaneStep::ThreePoint { points: [a, b, c] } => {
            format!("CPlane 3Point {} {} {}", point(a), point(b), point(c))
        }
        PlaneStep::ThreePointInput { points: [a, b, c] } => format!("CPlane 3Point {a} {b} {c}"),
        PlaneStep::OriginInput { point } => format!("CPlane {point}"),
        PlaneStep::Elevation { distance } => format!("CPlane Elevation {distance}"),
        PlaneStep::Through { point: p } => format!("CPlane Through {}", point(p)),
        PlaneStep::Rotate {
            axis: [a, b],
            angle,
        } => format!("CPlane Rotate {} {} {angle}", point(a), point(b)),
        PlaneStep::Undo => "CPlane Undo".into(),
        PlaneStep::Redo => "CPlane Redo".into(),
    };
    let action = cplane::parse(&command, state.frame(), previous, tolerance)
        .unwrap()
        .map_err(|_| ProbeError::FixtureInvariant("invalid CPlane command fixture"))?;
    match action {
        PlaneAction::Set(frame) => {
            state.set(frame);
        }
        PlaneAction::Undo => {
            state.undo();
        }
        PlaneAction::Redo => {
            state.redo();
        }
        PlaneAction::Prompt(_) => {
            return Err(ProbeError::FixtureInvariant("incomplete CPlane fixture"));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionPlaneInputFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub before: Vec<String>,
    pub step: PlaneStep,
    pub after: Vec<String>,
}

pub(super) fn run_input(
    f: &ConstructionPlaneInputFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.before.is_empty() || f.after.is_empty() || f.before.len() + f.after.len() > 256 {
        return Err(ProbeError::FixtureInvariant(
            "expected before/after points for nested CPlane",
        ));
    }
    let frame = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let mut state = ConstructionPlaneState::new(frame);
    let mut points = Vec::new();
    let append =
        |tokens: &[String], plane: Frame3, points: &mut Vec<Point3>| -> Result<(), ProbeError> {
            for token in tokens {
                let point = viboceros_drafting::PointInput::parse(token)
                    .ok_or(ProbeError::FixtureInvariant("expected point token"))?
                    .and_then(|p| p.resolve(plane, points.last().copied()))
                    .map_err(|_| ProbeError::FixtureInvariant("invalid nested point"))?;
                points.push(point);
            }
            Ok(())
        };
    append(&f.before, frame, &mut points)?;
    apply_step(&mut state, &f.step, points.last().copied(), tolerance)?;
    append(&f.after, state.frame(), &mut points)?;
    let polyline = Polyline3::try_new(points, tolerance)?;
    Ok((
        json!({"points":polyline.vertices().iter().map(|p| p.to_array()).collect::<Vec<_>>()}),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_cplane_fixtures_preserve_prior_polyline_points() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_plane_input.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 24);
        for result in response.results {
            assert_eq!(result.elapsed_ns, 0);
            assert_eq!(result.value["points"].as_array().unwrap().len(), 3);
            assert_eq!(result.value["points"][0], json!([1., 2., 3.]));
            assert_eq!(result.value["points"][1], json!([4., 5., 6.]));
        }
    }
    #[test]
    fn permanent_construction_plane_fixtures_remain_orthonormal_and_right_handed() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/construction_planes.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        for result in response.results {
            assert_eq!(result.elapsed_ns, 0);
            for frame in result.value["states"].as_array().unwrap() {
                let axes: [[f64; 3]; 3] = serde_json::from_value(frame["axes"].clone()).unwrap();
                let axes = axes.map(|a| Vector3::try_from(a).unwrap());
                for axis in axes {
                    assert!((axis.length().unwrap() - 1.0).abs() < 1e-12);
                }
                assert!(axes[0].dot(axes[1]).unwrap().abs() < 1e-12);
                assert!(
                    (axes[0].cross(axes[1]).unwrap().dot(axes[2]).unwrap() - 1.0).abs() < 1e-12
                );
            }
        }
    }
}
