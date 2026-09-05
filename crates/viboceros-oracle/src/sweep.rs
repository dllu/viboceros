use super::curve_join_close::CurveInput;
use super::*;
use viboceros_geometry::{Sweep1, SweepBlend, SweepFrameStyle, SweepSection};

#[cfg(test)]
mod tests {
    #[test]
    fn permanent_sweep_fixtures_execute_with_finite_geometry() {
        for (source, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1.json"),
                11,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1_curved_blend.json"),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1_diagnostics.json"),
                2,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1_command.json"),
                6,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1_multisection.json"),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/sweep1_basis_diagnostics.json"),
                8,
            ),
        ] {
            let request = serde_json::from_str(source).unwrap();
            let response = super::run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            for result in response.results {
                let surfaces = result.value.as_array().unwrap();
                assert!(!surfaces.is_empty());
                for surface in surfaces {
                    assert_eq!(surface["samples"].as_array().unwrap().len(), 135);
                    assert!(surface["samples"].as_array().unwrap().iter().all(|p| {
                        p.as_array()
                            .unwrap()
                            .iter()
                            .all(|c| c.as_f64().unwrap().is_finite())
                    }));
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SweepFixture {
    pub rail: CurveInput,
    pub sections: Vec<CurveInput>,
    pub parameters: Vec<f64>,
    #[serde(default)]
    pub roadlike_axis: Option<[f64; 3]>,
    #[serde(default)]
    pub blend: u8,
    #[serde(default)]
    pub queries: Option<Vec<[f64; 3]>>,
    #[serde(default)]
    pub inspect_definition: bool,
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub refit_rail: bool,
}

pub(super) fn run(
    fixture: &SweepFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let rail = fixture.rail.geometry()?;
    if fixture.closed {
        return Err(ProbeError::FixtureInvariant(
            "closed sweep closure is not implemented",
        ));
    }
    if fixture.parameters.len() != fixture.sections.len() {
        return Err(ProbeError::FixtureInvariant(
            "sweep section/parameter count mismatch",
        ));
    }
    let sections = fixture
        .sections
        .iter()
        .zip(&fixture.parameters)
        .map(|(curve, &parameter)| {
            Ok(SweepSection {
                parameter,
                curve: curve.geometry()?.as_ref().to_nurbs()?,
            })
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let style = match fixture.roadlike_axis {
        Some(axis) => SweepFrameStyle::Roadlike(Vector3::try_from(axis)?.normalized_nonzero()?),
        None => SweepFrameStyle::Freeform,
    };
    let blend = match fixture.blend {
        0 => SweepBlend::Local,
        1 => SweepBlend::Global,
        _ => return Err(ProbeError::FixtureInvariant("invalid sweep blend")),
    };
    let build = || {
        if fixture.command {
            command_surfaces(&rail, &sections, fixture, tolerance)
        } else {
            Ok(vec![
                Sweep1::try_new(rail.as_ref(), &sections, style, blend, tolerance)?.to_surface()?,
            ])
        }
    };
    let mut surfaces = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        surfaces = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let mut records = Vec::new();
    for surface in surfaces {
        let mut samples = Vec::new();
        if let Some(queries) = &fixture.queries {
            for query in queries {
                let (u, v) = surface.closest_parameters(Point3::try_from(*query)?, tolerance)?;
                samples.push(surface.evaluate(u, v)?.to_array());
            }
        } else {
            for j in 0..=8 {
                for i in 0..=8 {
                    let u = *surface.domain_u().start() * (1.0 - i as f64 / 8.0)
                        + *surface.domain_u().end() * i as f64 / 8.0;
                    samples.push(surface.evaluate(u, j as f64 / 8.0)?.to_array());
                }
            }
        }
        let mut record = json!({"samples":samples});
        if fixture.inspect_definition {
            record["definition"] = nurbs_surface_definition_value(&surface);
        }
        records.push(record);
    }
    Ok((json!(records), if fixture.command { 0 } else { elapsed }))
}

fn command_surfaces(
    rail: &viboceros_geometry::Curve3,
    sections: &[SweepSection],
    fixture: &SweepFixture,
    tolerance: Tolerance,
) -> Result<Vec<NurbsSurface>, ProbeError> {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    let rail_id = document.add_geometry_with_attributes(
        Geometry::from(rail.clone()),
        ObjectAttributes::on_layer(document.current_layer_id()).with_name("Rail"),
    )?;
    let mut ids = vec![rail_id];
    for section in sections {
        ids.push(document.add_geometry(Geometry::NurbsCurve(section.curve.clone()))?);
    }
    document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let mut text = format!(
        "Sweep1 RailName=Rail Parameters={} GlobalShapeBlending={} RefitRail={}",
        fixture
            .parameters
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        if fixture.blend == 1 { "Yes" } else { "No" },
        if fixture.refit_rail { "Yes" } else { "No" }
    );
    if let Some([x, y, z]) = fixture.roadlike_axis {
        text.push_str(&format!(" FrameStyle=Roadlike Axis={x},{y},{z}"));
    }
    registry.execute(&mut document, &text)?;
    let mut surfaces = Vec::new();
    for object in document.objects().filter(|o| !ids.contains(&o.id())) {
        let Geometry::Brep(brep) = object.geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "sweep command must create a BRep",
            ));
        };
        surfaces.extend(brep.faces().iter().map(|f| f.surface().clone()));
    }
    Ok(surfaces)
}
