//! Shared owned centroid fixtures; native geometry never reads observed targets.
use super::*;
use crate::object_source::ObjectSource;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod volume_tests;
#[cfg(test)]
mod volume_unit_tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CentroidFixture {
    pub sources: Vec<ObjectSource>,
    #[serde(default)]
    pub groups: Vec<Vec<usize>>,
    pub selected: Option<Vec<usize>>,
    #[serde(default = "preselect_default")]
    pub preselect: bool,
    /// Literal input to the conditional non-closed warning, not a command result.
    pub open_confirmation: Option<String>,
    pub display_units: Option<String>,
    pub unit_setup: Option<Vec<String>>,
    pub model_units: Option<u32>,
}
fn preselect_default() -> bool {
    true
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Measure {
    Area,
    Volume,
    ScalarVolume,
}

pub(super) fn run(
    f: &CentroidFixture,
    tolerance: Tolerance,
    measure: Measure,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid area centroid fixture");
    if (measure != Measure::ScalarVolume
        && (f.display_units.is_some() || f.unit_setup.is_some() || f.model_units.is_some()))
        || (f.preselect && f.display_units.is_some())
        || f.unit_setup
            .as_ref()
            .is_some_and(|steps| steps.is_empty() || steps.len() > 4)
    {
        return Err(invalid());
    }
    if let Some(answer) = &f.open_confirmation
        && (measure == Measure::Area || !matches!(answer.as_str(), "yes" | "no" | "escape"))
    {
        return Err(invalid());
    }
    if f.sources.is_empty() || f.sources.len() > 32 || f.groups.len() > 16 {
        return Err(invalid());
    }
    let selected = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..f.sources.len()).collect());
    let valid = |indices: &[usize]| {
        !indices.is_empty()
            && indices.iter().all(|i| *i < f.sources.len())
            && indices.iter().collect::<BTreeSet<_>>().len() == indices.len()
    };
    if !valid(&selected) || f.groups.iter().any(|g| !valid(g)) {
        return Err(invalid());
    }
    let mut d = Document::new(tolerance);
    if let Some(code) = f.model_units {
        use viboceros_geometry::LengthUnitSystem;
        d.set_units(
            match code {
                0 => LengthUnitSystem::None,
                2 => LengthUnitSystem::Millimeters,
                4 => LengthUnitSystem::Meters,
                8 => LengthUnitSystem::Inches,
                _ => return Err(invalid()),
            },
            false,
        )?;
    }
    let r = CommandRegistry::with_builtins();
    if measure == Measure::ScalarVolume {
        let units = r.object_selection_prompt("Volume")?.ok_or_else(invalid)?;
        if f.display_units
            .iter()
            .chain(f.unit_setup.iter().flatten())
            .any(|unit| !units.choices[0].choices.contains(&unit.as_str()))
        {
            return Err(invalid());
        }
    }
    let mut unit_setup = Vec::new();
    for unit in f.unit_setup.iter().flatten() {
        let input = format!("Volume Units={unit}");
        r.accept_object_selection_input(&input)?;
        unit_setup.push(
            r.execute_postselected(&mut d, &input, viboceros_command::CommandContext::default())
                .is_ok(),
        );
    }
    let mut ids = Vec::new();
    let mut properties = Vec::new();
    for source in &f.sources {
        let geometry = source.geometry(tolerance)?;
        let value = match measure {
            Measure::Area => {
                let mass = match &geometry {
                    Geometry::Mesh(mesh) => Some(mesh.area_mass_properties()?),
                    Geometry::NurbsSurface(s) => Some(s.area_mass_properties(tolerance)?),
                    Geometry::Brep(b) => Some(b.area_mass_properties(tolerance)?),
                    _ => match geometry.curve_ref() {
                        Some(c) if c.is_closed()? && c.is_planar(tolerance)? => {
                            Some(c.planar_area_mass_properties(tolerance)?)
                        }
                        _ => None,
                    },
                };
                match mass {
                    Some(m) => json!({"area":m.area()?,"centroid":m.centroid()?.to_array()}),
                    None => Value::Null,
                }
            }
            Measure::Volume | Measure::ScalarVolume => {
                let mass = geometry
                    .volume_boundary()
                    .map(|boundary| {
                        viboceros_geometry::VolumeMassProperties::from_boundaries(
                            &[boundary],
                            tolerance,
                        )
                    })
                    .transpose()?;
                match mass {
                    Some(m) => {
                        json!({"volume":m.signed_volume()?,"centroid": if m.is_zero() { None } else { Some(m.centroid()?.to_array()) }})
                    }
                    None => Value::Null,
                }
            }
        };
        properties.push(value);
        ids.push(d.add_geometry(geometry)?);
    }
    for (i, group) in f.groups.iter().enumerate() {
        d.add_group(Some(format!("sources-{i}")), group.iter().map(|i| ids[*i]))?;
    }
    d.select_objects_direct(selected.iter().map(|i| ids[*i]), SelectionMode::Replace)?;
    let before = d.objects().cloned().collect::<Vec<_>>();
    let command = match measure {
        Measure::Area => "AreaCentroid",
        Measure::Volume => "VolumeCentroid",
        Measure::ScalarVolume => "Volume",
    };
    let mut invocation = command.to_owned();
    if let Some(unit) = &f.display_units {
        invocation.push_str(&format!(" Units={unit}"));
        r.accept_object_selection_input(&invocation)?;
    }
    let mut confirmation = None;
    if let Some(answer) = &f.open_confirmation {
        let prompt = r
            .object_selection_prompt(&invocation)?
            .ok_or_else(invalid)?;
        let question = if d
            .selected_objects()
            .any(|o| prompt.filter.accepts_object(o))
        {
            r.object_selection_confirmation(&d, &prompt)?
        } else {
            None
        };
        confirmation = Some(question.is_some());
        if let Some(mut question) = question {
            // The captured warning treats Escape as Yes. The requested input
            // is ignored when this actual source selection needs no warning.
            question.update_options(if answer == "no" { "No" } else { "Yes" })?;
            invocation = question.command_line();
        }
    }
    let result = if f.preselect {
        r.execute(&mut d, &invocation)
    } else {
        r.execute_postselected(
            &mut d,
            &invocation,
            viboceros_command::CommandContext::default(),
        )
    };
    let succeeded = result.is_ok();
    if d.objects().take(before.len()).ne(before.iter()) {
        return Err(invalid());
    }
    let points = d
        .objects()
        .skip(ids.len())
        .map(|o| {
            let Geometry::Point(p) = o.geometry() else {
                return Err(invalid());
            };
            Ok(json!({
                "point": p.to_array(),
                "selected": d.is_selected(o.id()),
                "current_layer": o.attributes().layer_id() == d.current_layer_id(),
                "groups": o.group_ids().len(),
                "name": o.attributes().name().unwrap_or("")
            }))
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    let selected = ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| d.is_selected(*id).then_some(i))
        .collect::<Vec<_>>();
    let mut value =
        json!({"properties":properties,"succeeded":succeeded,"points":points,"selected":selected});
    if let Some(asked) = confirmation {
        value["confirmation"] = json!(asked);
    }
    if f.unit_setup.is_some() {
        value["unit_setup"] = json!(unit_setup);
    }
    if measure == Measure::ScalarVolume {
        let volume = match result {
            Ok(message) => Some(
                message
                    .rsplit_once("total volume ")
                    .ok_or_else(invalid)?
                    .1
                    .split_whitespace()
                    .next()
                    .ok_or_else(invalid)?
                    .parse::<f64>()
                    .map_err(|_| invalid())?,
            ),
            Err(_) => None,
        };
        value["volume"] = json!(volume);
    }
    Ok((value, 0))
}
