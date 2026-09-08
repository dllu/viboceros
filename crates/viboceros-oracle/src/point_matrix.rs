//! PointGrid command oracle; independent of interpolated surface grids.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointMatrixFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub points: [[f64; 3]; 2],
    pub count: [usize; 3],
    pub height: Option<f64>,
}

pub(super) fn run(
    f: &PointMatrixFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let context = viboceros_command::CommandContext {
        construction_plane: Frame3::try_from_directions(
            Point3::try_from(f.origin)?,
            Vector3::try_from(f.x_axis)?,
            Vector3::try_from(f.y_axis)?,
            tolerance,
        )?,
    };
    let mut command = format!(
        "PointGrid XCount={} YCount={} ZCount={}",
        f.count[0], f.count[1], f.count[2]
    );
    for p in f.points {
        command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
    }
    if let Some(height) = f.height {
        command.push_str(&format!(" {height}"));
    }
    let mut document = Document::new(tolerance);
    CommandRegistry::with_builtins().execute_in_context(&mut document, &command, context)?;
    let Geometry::PointCloud(cloud) = document
        .objects()
        .next()
        .ok_or(ProbeError::FixtureInvariant("missing grid"))?
        .geometry()
    else {
        return Err(ProbeError::FixtureInvariant("expected a point cloud"));
    };
    let frame = context
        .construction_plane
        .with_origin(Point3::try_from(f.points[0])?);
    let mut size = frame.coordinates_of(Point3::try_from(f.points[1])?)?;
    size[2] = f.height.unwrap_or(size[1].abs());
    let count = [f.count[0].max(2), f.count[1].max(2), f.count[2]];
    let mut points = cloud
        .points()
        .iter()
        .map(|p| {
            let local = frame.coordinates_of(*p)?;
            let key: [i64; 3] = [2, 1, 0]
                .map(|axis| (local[axis] / size[axis] * (count[axis] - 1) as f64).round() as i64);
            Ok((key, p.to_array()))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    points.sort_by_key(|(key, _)| *key);
    Ok((
        json!({"points": points.into_iter().map(|(_, point)| point).collect::<Vec<_>>()}),
        0,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn point_grid_command_fixture_runs_all_cases() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_matrix_command.json"
        ))
        .unwrap();
        let result = crate::run_request(&request).unwrap();
        assert_eq!(result.results.len(), request.operations.len());
        assert!(
            result
                .results
                .iter()
                .all(|r| !r.value["points"].as_array().unwrap().is_empty())
        );
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_matrix_command.json"
        ))
        .unwrap();
        assert_eq!(
            result.results.len(),
            reference["results"].as_array().unwrap().len()
        );
        for actual in result.results {
            let expected = reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == actual.id)
                .unwrap();
            let mut unmatched = expected["value"]["points"].as_array().unwrap().clone();
            let actual_points = actual.value["points"].as_array().unwrap();
            assert_eq!(actual_points.len(), unmatched.len(), "{}", actual.id);
            // Independent one-to-one set matching, without the live probe's
            // lattice sort keys. Removing matches retains duplicate multiplicity.
            for p in actual_points {
                let index = unmatched
                    .iter()
                    .position(|q| {
                        (0..3).all(|axis| {
                            (p[axis].as_f64().unwrap() - q[axis].as_f64().unwrap()).abs() <= 1e-10
                        })
                    })
                    .unwrap_or_else(|| panic!("{}: unmatched point {p}", actual.id));
                unmatched.swap_remove(index);
            }
            assert!(unmatched.is_empty());
        }
    }
}
