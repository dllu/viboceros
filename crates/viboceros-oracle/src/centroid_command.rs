//! Shared owned centroid fixtures; native geometry never reads observed targets.
use super::*;
use crate::object_source::ObjectSource;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod volume_tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CentroidFixture {
    pub sources: Vec<ObjectSource>,
    #[serde(default)]
    pub groups: Vec<Vec<usize>>,
    pub selected: Option<Vec<usize>>,
    #[serde(default = "preselect_default")]
    pub preselect: bool,
}
fn preselect_default() -> bool {
    true
}

#[derive(Clone, Copy)]
pub(super) enum Measure {
    Area,
    Volume,
}

pub(super) fn run(
    f: &CentroidFixture,
    tolerance: Tolerance,
    measure: Measure,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid area centroid fixture");
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
            Measure::Volume => {
                let mass = match &geometry {
                    Geometry::Mesh(m) => Some(m.volume_mass_properties()?),
                    Geometry::Brep(b) => Some(b.volume_mass_properties(tolerance)?),
                    Geometry::NurbsSurface(s) => Some(s.volume_mass_properties(tolerance)?),
                    _ => None,
                };
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
    let r = CommandRegistry::with_builtins();
    let command = match measure {
        Measure::Area => "AreaCentroid",
        Measure::Volume => "VolumeCentroid",
    };
    let succeeded = if f.preselect {
        r.execute(&mut d, command)
    } else {
        r.execute_postselected(
            &mut d,
            command,
            viboceros_command::CommandContext::default(),
        )
    }
    .is_ok();
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
    Ok((
        json!({"properties":properties,"succeeded":succeeded,"points":points,"selected":selected}),
        0,
    ))
}
