//! Bounded public TaperSpaceMorph point-map protocol.
use super::*;
use viboceros_geometry::TaperPointMorph;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TaperPointFixture {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub start_radius: f64,
    pub end_radius: f64,
    pub flat: bool,
    pub infinite: bool,
    pub points: Vec<[f64; 3]>,
}

pub(super) fn run(
    fixture: &TaperPointFixture,
    iterations: u32,
    _tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if fixture.points.is_empty()
        || fixture.points.len() > 256
        || [fixture.start_radius, fixture.end_radius]
            .into_iter()
            .chain([fixture.start, fixture.end].into_iter().flatten())
            .chain(fixture.points.iter().flatten().copied())
            .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid bounded Taper point recipe",
        ));
    }
    let morph = TaperPointMorph::try_new(
        Point3::try_from(fixture.start)?,
        Point3::try_from(fixture.end)?,
        fixture.start_radius,
        fixture.end_radius,
        fixture.flat,
        fixture.infinite,
    );
    let points = fixture
        .points
        .iter()
        .copied()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let (mapped, elapsed) = measure(iterations, || {
        points
            .iter()
            .map(|point| match &morph {
                Ok(morph) => morph.morph_point(*point).map(Point3::to_array),
                // Native invalid SDK definitions return each original point.
                Err(_) => Ok(point.to_array()),
            })
            .collect::<Result<Vec<_>, GeometryError>>()
    })?;
    Ok((json!({"valid": morph.is_ok(), "points": mapped}), elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taper_protocol_rejects_unbounded_samples_and_replays_native_results() {
        let input: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/taper_points.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/taper_points.json"
        ))
        .unwrap();
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let fixture: TaperPointFixture = serde_json::from_value(op.clone()).unwrap();
            let value = run(&fixture, 1, Tolerance::DEFAULT).unwrap().0;
            assert_eq!(value["valid"], row["value"]["valid"], "{}", op["id"]);
            for (actual, expected) in value["points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(row["value"]["points"].as_array().unwrap())
            {
                for (a, b) in actual
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(expected.as_array().unwrap())
                {
                    let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                    assert!(
                        (a - b).abs() <= 1e-10 + 1e-12 * b.abs(),
                        "{}: {a} != {b}",
                        op["id"]
                    );
                }
            }
        }
        let fixture: TaperPointFixture =
            serde_json::from_value(input["operations"][0].clone()).unwrap();
        for bad in [
            TaperPointFixture {
                points: vec![],
                ..fixture.clone()
            },
            TaperPointFixture {
                points: vec![[0.; 3]; 257],
                ..fixture.clone()
            },
            TaperPointFixture {
                end_radius: f64::NAN,
                ..fixture.clone()
            },
            TaperPointFixture {
                start: [1e7, 0., 0.],
                ..fixture.clone()
            },
            TaperPointFixture {
                points: vec![[0., 0., f64::INFINITY]],
                ..fixture.clone()
            },
        ] {
            assert!(run(&bad, 1, Tolerance::DEFAULT).is_err());
        }
    }
}
