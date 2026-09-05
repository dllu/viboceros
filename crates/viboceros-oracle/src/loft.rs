use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoftFixture {
    pub curves: Vec<NurbsCurveDefinition>,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub sample_geometry: bool,
}

pub(super) fn run(
    fixture: &LoftFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    use viboceros_geometry::try_loft_nurbs_curves;
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let style = parse_style(&fixture.style)?;
    if fixture.command {
        return command(fixture, &curves, iterations, tolerance);
    }
    let mut surface = try_loft_nurbs_curves(&curves, style, fixture.closed)?;
    let started = Instant::now();
    for _ in 0..iterations {
        surface = black_box(try_loft_nurbs_curves(&curves, style, fixture.closed)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!([surface_record(&surface, fixture.sample_geometry)?]),
        elapsed,
    ))
}

fn surface_record(surface: &NurbsSurface, samples: bool) -> Result<Value, ProbeError> {
    let mut record = nurbs_surface_definition_value(surface);
    if samples {
        let mut points = Vec::with_capacity(289);
        for u in 0..=16 {
            for v in 0..=16 {
                points.push(
                    surface
                        .evaluate(
                            surface.parameter_at_u(f64::from(u) / 16.0)?,
                            surface.parameter_at_v(f64::from(v) / 16.0)?,
                        )?
                        .to_array(),
                );
            }
        }
        record["samples"] = json!(points);
    }
    Ok(record)
}

fn parse_style(value: &str) -> Result<viboceros_geometry::LoftStyle, ProbeError> {
    use viboceros_geometry::LoftStyle;
    Ok(match value {
        "" | "normal" => LoftStyle::Normal,
        "loose" => LoftStyle::Loose,
        "tight" => LoftStyle::Tight,
        "straight" => LoftStyle::Straight,
        "uniform" => LoftStyle::Uniform,
        _ => return Err(ProbeError::FixtureInvariant("unknown loft style")),
    })
}

pub(super) fn build_brep(fixture: &LoftFixture, tolerance: Tolerance) -> Result<Brep, ProbeError> {
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Brep::try_loft(
        &curves,
        parse_style(&fixture.style)?,
        fixture.closed,
        tolerance,
    )?)
}

fn command(
    fixture: &LoftFixture,
    curves: &[NurbsCurve],
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "Loft Type={} Closed={}",
        if fixture.style.is_empty() {
            "normal"
        } else {
            &fixture.style
        },
        if fixture.closed { "Yes" } else { "No" }
    );
    let mut elapsed = 0_u128;
    let mut result = Value::Null;
    for iteration in 0..=iterations {
        let started = Instant::now();
        let mut document = Document::new(tolerance);
        let ids = curves
            .iter()
            .enumerate()
            .map(|(i, c)| {
                document.add_geometry_with_attributes(
                    Geometry::NurbsCurve(c.clone()),
                    ObjectAttributes::on_layer(document.current_layer_id())
                        .with_name(format!("loft-source-{i}")),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        registry.execute(&mut document, &command)?;
        let outputs = document.objects().filter(|o| !ids.contains(&o.id())).map(|o| {
            let Geometry::Brep(brep) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant("loft command output must be a BRep"));
            };
            let mut faces = brep.faces().iter().collect::<Vec<_>>();
            faces.sort_by(|a, b| {
                let a = a.surface(); let b = b.surface();
                [(*a.domain_v().start(), *b.domain_v().start()),
                 (*a.domain_v().end(), *b.domain_v().end()),
                 (*a.domain_u().start(), *b.domain_u().start()),
                 (*a.domain_u().end(), *b.domain_u().end())]
                 .into_iter().map(|(a,b)| a.total_cmp(&b))
                 .find(|c| !c.is_eq()).unwrap_or(std::cmp::Ordering::Equal)
            });
            Ok(json!({
                "surfaces": faces.iter().map(|f| surface_record(f.surface(), fixture.sample_geometry)).collect::<Result<Vec<_>, _>>()?,
                "face_reversed": faces.iter().map(|f| f.is_reversed()).collect::<Vec<_>>(),
                "valid": true, "vertices": brep.vertices().len(), "edges": brep.edges().len(),
                "selected": document.is_selected(o.id()), "name": o.attributes().name(),
                "group_count": document.groups().filter(|g| g.members().any(|id|id==o.id())).count(),
            }))
        }).collect::<Result<Vec<_>, ProbeError>>()?;
        result = json!({"succeeded":true,"originals_present":ids.iter().map(|id|document.object(*id).is_some()).collect::<Vec<_>>(),"originals_selected":ids.iter().map(|id|document.is_selected(*id)).collect::<Vec<_>>(),"outputs":outputs});
        drop(document);
        if iteration > 0 {
            elapsed += started.elapsed().as_nanos();
        }
    }
    Ok((
        result,
        u64::try_from(elapsed).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn end_weight_fixtures_preserve_the_original_ruled_profiles() {
        for fixture in [
            include_str!("../../../tools/rhino_oracle/fixtures/loft_end_weights.json"),
            include_str!("../../../tools/rhino_oracle/fixtures/loft_end_weights_diagnostics.json"),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = crate::run_request(&request).unwrap();
            for (operation, result) in request.operations.iter().zip(&response.results) {
                let crate::Operation::Loft { fixture, .. } = operation else {
                    panic!("expected loft");
                };
                let source = super::nurbs_curve_from_definition(&fixture.curves[0]).unwrap();
                let controls = source.control_points();
                let c = (controls.last().unwrap().weight() / controls[0].weight()).sqrt();
                let height = fixture.curves[1].control_points[0].point[2];
                let samples = result.value[0]["samples"].as_array().unwrap();
                assert_eq!(samples.len(), 289);
                for u in 0..=16 {
                    for v in 0..=16 {
                        let s = f64::from(v) / 16.0;
                        let t = s / (c * (1.0 - s) + s);
                        let mut expected = source.evaluate(t).unwrap().to_array();
                        expected[2] = height * f64::from(u) / 16.0;
                        for (axis, expected) in expected.into_iter().enumerate() {
                            let actual = samples[(u * 17 + v) as usize][axis].as_f64().unwrap();
                            let epsilon = if height > 1e6 { 5e-7 } else { 5e-14 };
                            assert!(
                                (actual - expected).abs() < epsilon,
                                "{}: {actual} != {expected}",
                                result.id
                            );
                        }
                    }
                }
                for (sample, curve, control) in [(0, 0, 0), (16, 0, 3), (272, 1, 0), (288, 1, 3)] {
                    assert_eq!(
                        samples[sample],
                        serde_json::json!(fixture.curves[curve].control_points[control].point)
                    );
                }
            }
        }
    }

    #[test]
    fn all_permanent_loft_and_document_fixtures_run() {
        for (fixture, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/loft.json"),
                34,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/loft_command.json"),
                33,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/loft_3dm_interchange.json"),
                4,
            ),
        ] {
            let request: crate::ProbeRequest = serde_json::from_str(fixture).unwrap();
            let response = crate::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            for (operation, result) in request.operations.iter().zip(&response.results) {
                if let crate::Operation::Loft { fixture, .. } = operation {
                    assert!(fixture.sample_geometry);
                    let surfaces = if fixture.command {
                        result.value["outputs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .flat_map(|o| o["surfaces"].as_array().unwrap())
                            .collect::<Vec<_>>()
                    } else {
                        result.value.as_array().unwrap().iter().collect::<Vec<_>>()
                    };
                    for surface in surfaces {
                        assert_eq!(surface["samples"].as_array().unwrap().len(), 289);
                    }
                }
            }
        }
    }
}
