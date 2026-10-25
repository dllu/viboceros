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
    #[serde(default)]
    pub start_point: Option<[f64; 3]>,
    #[serde(default)]
    pub end_point: Option<[f64; 3]>,
    #[serde(default)]
    pub inspect_inputs: bool,
    #[serde(default)]
    pub rebuild_point_count: usize,
    #[serde(default)]
    pub refit_tolerance: Option<f64>,
}

impl LoftFixture {
    fn validate_simplification(&self) -> Result<(), ProbeError> {
        if let Some(tolerance) = self.refit_tolerance {
            if !tolerance.is_finite() || tolerance < 0. {
                return Err(ProbeError::FixtureInvariant(
                    "Loft Refit requires a nonnegative finite tolerance",
                ));
            }
            if self.rebuild_point_count != 0 {
                return Err(ProbeError::FixtureInvariant(
                    "Loft Refit and Rebuild are mutually exclusive",
                ));
            }
        }
        Ok(())
    }

    fn end_points(&self) -> Result<viboceros_geometry::LoftEndPoints, GeometryError> {
        Ok(viboceros_geometry::LoftEndPoints {
            start: self.start_point.map(Point3::try_from).transpose()?,
            end: self.end_point.map(Point3::try_from).transpose()?,
        })
    }
}

pub(super) fn run(
    fixture: &LoftFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    fixture.validate_simplification()?;
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let style = parse_style(&fixture.style)?;
    let ends = fixture.end_points()?;
    if fixture.command {
        return command(fixture, &curves, iterations, tolerance);
    }
    let before = fixture.inspect_inputs.then(|| {
        curves
            .iter()
            .map(nurbs_curve_definition_value)
            .collect::<Vec<_>>()
    });
    let mut surface = fixture_surface(
        &curves,
        style,
        fixture.closed,
        ends,
        fixture.rebuild_point_count,
        fixture.refit_tolerance,
        tolerance,
    )?;
    let started = Instant::now();
    for _ in 0..iterations {
        surface = black_box(fixture_surface(
            &curves,
            style,
            fixture.closed,
            ends,
            fixture.rebuild_point_count,
            fixture.refit_tolerance,
            tolerance,
        )?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let surfaces = json!([surface_record(&surface, fixture.sample_geometry)?]);
    Ok((
        if let Some(before) = before {
            json!({"before":before,"after":curves.iter().map(nurbs_curve_definition_value).collect::<Vec<_>>(),"surfaces":surfaces})
        } else {
            surfaces
        },
        elapsed,
    ))
}

fn fixture_surface(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
    ends: viboceros_geometry::LoftEndPoints,
    rebuild: usize,
    refit: Option<f64>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, ProbeError> {
    if let Some(fit_tolerance) = refit.filter(|t| *t > 0.) {
        #[cfg(feature = "native-smlib")]
        {
            return viboceros_smlib::loft_surface_refitted(
                curves,
                style,
                closed,
                ends,
                fit_tolerance,
            )
            .map_err(CommandError::from)
            .map_err(ProbeError::from);
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            return Ok(viboceros_geometry::try_loft_nurbs_curves_refitted(
                curves,
                style,
                closed,
                ends,
                fit_tolerance,
            )?);
        }
    }
    if rebuild != 0 {
        #[cfg(feature = "native-smlib")]
        {
            return viboceros_smlib::loft_surface_rebuilt(
                curves, style, closed, ends, rebuild, tolerance,
            )
            .map_err(CommandError::from)
            .map_err(ProbeError::from);
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            return Ok(viboceros_geometry::try_loft_nurbs_curves_rebuilt(
                curves, style, closed, ends, rebuild, tolerance,
            )?);
        }
    }
    if ends.is_empty() {
        return surface(curves, style, closed);
    }
    if closed {
        return Err(ProbeError::FixtureInvariant(
            "closed Loft cannot have point ends",
        ));
    }
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface_with_end_points(curves, style, ends)
            .map_err(CommandError::from)
            .map_err(ProbeError::from)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves_with_end_points(
            curves, style, ends,
        )?)
    }
}

fn surface(
    curves: &[NurbsCurve],
    style: viboceros_geometry::LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, ProbeError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface(curves, style, closed)
            .map_err(CommandError::from)
            .map_err(ProbeError::from)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves(
            curves, style, closed,
        )?)
    }
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
    fixture.validate_simplification()?;
    let curves = fixture
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let ends = fixture.end_points()?;
    if !ends.is_empty() {
        if fixture.closed {
            return Err(ProbeError::FixtureInvariant(
                "closed Loft cannot have point ends",
            ));
        }
        return Brep::try_loft_with_surface_end_points(
            &curves,
            parse_style(&fixture.style)?,
            ends,
            viboceros_geometry::LoftOptions::default(),
            tolerance,
            &mut |c, s, e| {
                fixture_surface(
                    c,
                    s,
                    false,
                    e,
                    fixture.rebuild_point_count,
                    fixture.refit_tolerance,
                    tolerance,
                )
            },
        );
    }
    Brep::try_loft_with_surface(
        &curves,
        parse_style(&fixture.style)?,
        fixture.closed,
        tolerance,
        &mut |c, s, closed| {
            fixture_surface(
                c,
                s,
                closed,
                viboceros_geometry::LoftEndPoints::default(),
                fixture.rebuild_point_count,
                fixture.refit_tolerance,
                tolerance,
            )
        },
    )
}

fn command(
    fixture: &LoftFixture,
    curves: &[NurbsCurve],
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut command = format!(
        "Loft Type={} Closed={}",
        if fixture.style.is_empty() {
            "normal"
        } else {
            &fixture.style
        },
        if fixture.closed { "Yes" } else { "No" }
    );
    for (name, point) in [
        ("StartPoint", fixture.start_point),
        ("EndPoint", fixture.end_point),
    ] {
        if let Some([x, y, z]) = point {
            command.push_str(&format!(" {name}={x},{y},{z}"));
        }
    }
    if fixture.rebuild_point_count != 0 {
        command.push_str(&format!(" Rebuild={}", fixture.rebuild_point_count));
    }
    if let Some(tolerance) = fixture.refit_tolerance {
        command.push_str(&format!(" Refit={tolerance}"));
    }
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
