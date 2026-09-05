use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointGridFixture {
    /// Rhino's public API order: V varies fastest. Converted at the boundary.
    pub points: Vec<[f64; 3]>,
    pub count: [usize; 2],
    #[serde(default = "default_degree")]
    pub degree: [usize; 2],
    #[serde(default)]
    pub closed: [bool; 2],
    #[serde(default)]
    pub control: bool,
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub keep_points: bool,
}

fn default_degree() -> [usize; 2] {
    [3; 2]
}

pub(super) fn run(
    fixture: &PointGridFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if fixture.command {
        return command(fixture, iterations, tolerance);
    }
    let points = input_points(fixture)?;
    let mut result = build(fixture, &points);
    let start = Instant::now();
    for _ in 0..iterations {
        result = black_box(build(fixture, &points));
    }
    let elapsed =
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((record(&result?, fixture, true)?, elapsed))
}

fn input_points(fixture: &PointGridFixture) -> Result<Vec<Point3>, ProbeError> {
    if fixture.count[0].checked_mul(fixture.count[1]) != Some(fixture.points.len()) {
        return Err(ProbeError::FixtureInvariant(
            "point grid dimensions do not match the point array",
        ));
    }
    Ok((0..fixture.count[1])
        .flat_map(|v| {
            (0..fixture.count[0])
                .map(move |u| Point3::try_from(fixture.points[u * fixture.count[1] + v]))
        })
        .collect::<Result<Vec<_>, _>>()?)
}

fn build(fixture: &PointGridFixture, points: &[Point3]) -> Result<NurbsSurface, GeometryError> {
    if fixture.control {
        NurbsSurface::try_control_point_grid(points, fixture.count, fixture.degree)
    } else {
        NurbsSurface::try_through_point_grid(points, fixture.count, fixture.degree, fixture.closed)
    }
}

pub(super) fn build_brep(
    fixture: &PointGridFixture,
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
    let points = input_points(fixture)?;
    Ok(if fixture.control {
        Brep::try_control_point_grid(&points, fixture.count, fixture.degree, tolerance)?
    } else {
        Brep::try_through_point_grid(
            &points,
            fixture.count,
            fixture.degree,
            fixture.closed,
            tolerance,
        )?
    })
}

fn command(
    fixture: &PointGridFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut text = format!(
        "{} KeepPoints={}",
        if fixture.control {
            "SrfControlPtGrid"
        } else {
            "SrfPtGrid"
        },
        if fixture.keep_points { "Yes" } else { "No" }
    );
    if fixture.control {
        text.push_str(&format!(
            " Degree={} {} Degree={} {}",
            fixture.degree[0], fixture.count[0], fixture.degree[1], fixture.count[1]
        ));
    } else {
        text.push_str(&format!(
            " DegreeU={} ClosedU={} {} DegreeV={} ClosedV={} {}",
            fixture.degree[0],
            if fixture.closed[0] { "Yes" } else { "No" },
            fixture.count[0],
            fixture.degree[1],
            if fixture.closed[1] { "Yes" } else { "No" },
            fixture.count[1]
        ));
    }
    for p in &fixture.points {
        text.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
    }
    let mut elapsed = 0_u128;
    let mut value = Value::Null;
    for iteration in 0..=iterations {
        let start = Instant::now();
        let mut doc = Document::new(tolerance);
        let marker = doc.add_geometry(Geometry::Point(Point3::try_new(93.0, 97.0, 101.0)?))?;
        doc.select_objects_direct([marker], SelectionMode::Replace)?;
        registry.execute(&mut doc, &text)?;
        let mut surfaces = vec![];
        let mut points = vec![];
        for obj in doc.objects().filter(|o| o.id() != marker) {
            let mut output = match obj.geometry() {
                Geometry::Brep(brep) => record_brep(brep, fixture)?,
                Geometry::PointCloud(c) => {
                    json!({"kind":"point_cloud","points":c.points().iter().map(|p|p.to_array()).collect::<Vec<_>>()})
                }
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "point grid output must be a surface or point cloud",
                    ));
                }
            };
            output["selected"] = json!(doc.is_selected(obj.id()));
            output["name"] = json!(obj.attributes().name());
            output["group_count"] = json!(
                doc.groups()
                    .filter(|g| g.members().any(|id| id == obj.id()))
                    .count()
            );
            if matches!(obj.geometry(), Geometry::Brep(_)) {
                surfaces.push(output);
            } else {
                points.push(output);
            }
        }
        let sentinel_point = match doc.object(marker).map(|o| o.geometry()) {
            Some(Geometry::Point(p)) => Some(p.to_array()),
            _ => None,
        };
        value = json!({"outputs":surfaces,"points":points,"sentinel_present":doc.object(marker).is_some(),"sentinel_selected":doc.is_selected(marker),"sentinel_point":sentinel_point});
        drop(doc);
        if iteration > 0 {
            elapsed += start.elapsed().as_nanos();
        }
    }
    Ok((
        value,
        u64::try_from(elapsed).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

fn record_brep(brep: &Brep, fixture: &PointGridFixture) -> Result<Value, ProbeError> {
    let mut faces = brep.faces().iter().collect::<Vec<_>>();
    faces.sort_by(|a, b| {
        let a = a.surface();
        let b = b.surface();
        [
            *a.domain_v().start(),
            *a.domain_v().end(),
            *a.domain_u().start(),
            *a.domain_u().end(),
        ]
        .into_iter()
        .zip([
            *b.domain_v().start(),
            *b.domain_v().end(),
            *b.domain_u().start(),
            *b.domain_u().end(),
        ])
        .map(|(a, b)| a.total_cmp(&b))
        .find(|o| o.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
    });
    let values = faces
        .iter()
        .map(|f| record(f.surface(), fixture, false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"faces":values,"face_reversed":faces.iter().map(|f|f.is_reversed()).collect::<Vec<_>>(),
        "valid":true,"vertices":brep.vertices().len(),"edges":brep.edges().len()}),
    )
}

pub(super) fn record(
    s: &NurbsSurface,
    fixture: &PointGridFixture,
    include_constraints: bool,
) -> Result<Value, ProbeError> {
    let mut samples = Vec::new();
    for j in 0..=12 {
        for i in 0..=12 {
            samples.push(
                s.evaluate(
                    s.parameter_at_u(i as f64 / 12.0)?,
                    s.parameter_at_v(j as f64 / 12.0)?,
                )?
                .to_array(),
            );
        }
    }
    Ok(json!({"surface":nurbs_surface_definition_value(s),
        "closed":[s.is_closed_u()?,s.is_closed_v()?],
        "periodic":[s.is_periodic_u(),s.is_periodic_v()],"valid":true,"samples":samples,
        "constraints":if include_constraints {constraints(s,fixture)?} else {Value::Null}}))
}

fn constraints(s: &NurbsSurface, fixture: &PointGridFixture) -> Result<Value, ProbeError> {
    if fixture.control {
        return Ok(Value::Null);
    }
    let mut parameters: [Vec<f64>; 2] = [vec![], vec![]];
    let count = fixture.count;
    for axis in 0..2 {
        let (degree, knots) = if axis == 0 {
            (s.degree_u(), s.knots_u())
        } else {
            (s.degree_v(), s.knots_v())
        };
        if fixture.closed[axis] && degree > 1 {
            parameters[axis] = (0..count[axis])
                .map(|i| knots[i + 1..=i + degree].iter().sum::<f64>() / degree as f64)
                .collect();
        } else {
            let point = |i: usize, j: usize| {
                Point3::try_from(
                    fixture.points[if axis == 0 {
                        i * count[1] + j
                    } else {
                        j * count[1] + i
                    }],
                )
            };
            parameters[axis].push(0.0);
            for i in 0..count[axis] - 1 {
                let mut delta = 0.0;
                for j in 0..count[1 - axis] {
                    delta += point(i, j)?.distance_to(point(i + 1, j)?)?;
                }
                parameters[axis].push(parameters[axis][i] + delta / count[1 - axis] as f64);
            }
        }
    }
    let mut samples = vec![];
    for &v in &parameters[1] {
        for &u in &parameters[0] {
            samples.push(s.evaluate_extended(u, v)?.to_array());
        }
    }
    let domains = [s.domain_u(), s.domain_v()];
    Ok(
        json!({"parameters":parameters,"samples":samples,"outside_domain":
        [parameters[0].iter().any(|t|!domains[0].contains(t)),parameters[1].iter().any(|t|!domains[1].contains(t))]}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_point_grid_fixtures_exercise_geometry_commands_and_interchange() {
        for (text, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/point_grid.json"),
                53,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/point_grid_command.json"),
                18,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/point_grid_high_degree.json"),
                8,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/point_grid_high_degree_diagnostics.json"
                ),
                4,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/point_grid_3dm_interchange.json"
                ),
                4,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(text).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
        }
    }
}
