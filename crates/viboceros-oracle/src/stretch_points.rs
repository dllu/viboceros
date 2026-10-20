//! Public StretchSpaceMorph point-map protocol, shared with the Python oracle.
use super::*;
use viboceros_geometry::StretchPointMorph;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StretchPointFixture {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub length: Option<f64>,
    pub target: Option<[f64; 3]>,
    pub points: Vec<[f64; 3]>,
}
pub(super) fn run(f: &StretchPointFixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    if f.length.is_some() == f.target.is_some()
        || f.points.is_empty()
        || f.points.len() > 256
        || f.start
            .into_iter()
            .chain(f.end)
            .chain(f.length)
            .chain(f.target.into_iter().flatten())
            .chain(f.points.iter().flatten().copied())
            .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid bounded Stretch SDK recipe",
        ));
    }
    let start = Point3::try_from(f.start)?;
    let end = Point3::try_from(f.end)?;
    let morph = if let Some(length) = f.length {
        StretchPointMorph::try_new(start, end, length)
    } else {
        StretchPointMorph::try_with_target(start, end, Point3::try_from(f.target.unwrap())?)
    };
    let points = f
        .points
        .iter()
        .copied()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let (mapped, elapsed) = measure(iterations, || {
        points
            .iter()
            .map(|p| match &morph {
                Ok(m) => m.morph_point(*p).map(Point3::to_array),
                Err(_) => Ok(p.to_array()),
            })
            .collect::<Result<Vec<_>, GeometryError>>()
    })?;
    Ok((json!({"valid":morph.is_ok(),"points":mapped}), elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_point_witnesses_match_lengths_targets_reversed_axes_and_validity_boundaries() {
        let input: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/stretch.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/stretch.json"
        ))
        .unwrap();
        let mut count = 0;
        let mut worst = 0_f64;
        for (op, row) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            if op["op"] != "stretch_sdk" {
                continue;
            }
            assert_eq!(op["id"], row["id"]);
            count += 1;
            let fixture: StretchPointFixture = serde_json::from_value(op.clone()).unwrap();
            let actual = run(&fixture, 1).unwrap().0;
            assert_eq!(actual["valid"], row["value"]["valid"], "{}", op["id"]);
            for (a, e) in actual["points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(row["value"]["points"].as_array().unwrap())
            {
                let a = Point3::try_from(serde_json::from_value::<[f64; 3]>(a.clone()).unwrap())
                    .unwrap();
                let e = Point3::try_from(serde_json::from_value::<[f64; 3]>(e.clone()).unwrap())
                    .unwrap();
                let gap = a.distance_to(e).unwrap();
                worst = worst.max(gap);
                assert!(gap <= 1e-11, "{}: {gap}", op["id"]);
            }
        }
        assert_eq!(count, 23);
        eprintln!(
            "STRETCH_SDK_AUDIT {}",
            json!({"cases":count,"maximum_error":worst})
        );
    }
}
