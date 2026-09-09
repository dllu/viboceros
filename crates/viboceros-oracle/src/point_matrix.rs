//! PointGrid command oracle; independent of interpolated surface grids.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointMatrixFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub points: Vec<[f64; 3]>,
    #[serde(default)]
    pub three_point: bool,
    #[serde(default)]
    pub centered: bool,
    pub count: [usize; 3],
    pub height: Option<f64>,
    pub height_point: Option<[f64; 3]>,
}

pub(super) fn run(
    f: &PointMatrixFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_mode(f, tolerance, false)
}

pub(super) fn run_diagonal(
    f: &PointMatrixFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_mode(f, tolerance, true)
}

fn run_mode(
    f: &PointMatrixFixture,
    tolerance: Tolerance,
    diagonal: bool,
) -> Result<(Value, u64), ProbeError> {
    if (diagonal && (f.three_point || f.centered || f.height.is_some()))
        || (!diagonal && f.height_point.is_some())
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid PointGrid mode or height input",
        ));
    }
    if (f.centered && f.three_point) || f.points.len() != if f.three_point { 3 } else { 2 } {
        return Err(ProbeError::FixtureInvariant(
            "incorrect PointGrid base point count",
        ));
    }
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
    if f.three_point {
        command.push_str(" 3Point");
    }
    if f.centered {
        command.push_str(" Center");
    }
    if diagonal {
        command.push_str(" Diagonal");
    }
    for p in &f.points {
        command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
    }
    if let Some(height) = f.height {
        command.push_str(&format!(" {height}"));
    }
    if let Some(p) = f.height_point {
        command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
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
    if diagonal {
        return Ok((
            json!({"points": cloud.points().iter().map(|p| p.to_array()).collect::<Vec<_>>()}),
            0,
        ));
    }
    let frame = if f.three_point {
        Frame3::try_from_points(
            Point3::try_from(f.points[0])?,
            Point3::try_from(f.points[1])?,
            Point3::try_from(f.points[2])?,
            tolerance,
        )?
    } else {
        context
            .construction_plane
            .with_origin(Point3::try_from(f.points[0])?)
    };
    let mut size = frame.coordinates_of(Point3::try_from(f.points[1])?)?;
    if f.three_point {
        size[1] = frame.coordinates_of(Point3::try_from(f.points[2])?)?[1];
    }
    size[2] = f
        .height
        .unwrap_or(size[1].abs() * if f.centered { 2.0 } else { 1.0 });
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
    fn diagonal_grid_fixture_matches_rhino_in_source_order() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_matrix_diagonal.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_matrix_diagonal_prompt.json"
        ))
        .unwrap();
        assert_eq!(response.results.len(), 4);
        for result in response.results {
            let expected = reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == result.id)
                .unwrap();
            let actual: Vec<[f64; 3]> =
                serde_json::from_value(result.value["points"].clone()).unwrap();
            let expected: Vec<[f64; 3]> =
                serde_json::from_value(expected["value"]["points"].clone()).unwrap();
            assert_eq!(actual, expected, "{}", result.id);
        }
    }

    #[test]
    fn point_grid_command_fixture_runs_all_cases() {
        check(
            include_str!("../../../tools/rhino_oracle/fixtures/point_matrix_command.json"),
            include_str!("../../../tools/rhino_oracle/observations/point_matrix_command.json"),
        );
    }

    #[test]
    fn three_point_grid_command_fixture_matches_rhino_point_sets() {
        check(
            include_str!("../../../tools/rhino_oracle/fixtures/point_matrix_three_point.json"),
            include_str!("../../../tools/rhino_oracle/observations/point_matrix_three_point.json"),
        );
    }

    #[test]
    fn center_grid_command_fixture_matches_rhino_point_sets() {
        check(
            include_str!("../../../tools/rhino_oracle/fixtures/point_matrix_center.json"),
            include_str!("../../../tools/rhino_oracle/observations/point_matrix_center.json"),
        );
    }

    fn check(request: &str, reference: &str) {
        let request: crate::ProbeRequest = serde_json::from_str(request).unwrap();
        let result = crate::run_request(&request).unwrap();
        assert_eq!(result.results.len(), request.operations.len());
        assert!(
            result
                .results
                .iter()
                .all(|r| !r.value["points"].as_array().unwrap().is_empty())
        );
        let reference: serde_json::Value = serde_json::from_str(reference).unwrap();
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
