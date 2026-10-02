//! Independent command replay from explicit source geometry, never snap/output targets.
use super::*;
use crate::object_source::ObjectSource;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UntrimFixture {
    sources: Vec<Source>,
    keep_trim_objects: bool,
    #[serde(default)]
    preselect: bool,
    #[serde(default)]
    source_layer: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Source {
    Object(ObjectSource),
    Box(BoxSource),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BoxSource {
    Box,
}

pub(super) fn run(
    f: &UntrimFixture,
    tolerance: Tolerance,
    command: &str,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len()) {
        return Err(ProbeError::FixtureInvariant(
            "untrim commands require 1 to 8 sources",
        ));
    }
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    let layer = if f.source_layer {
        document.add_layer("Sources", ColorRgb::new(0, 0, 0))?
    } else {
        document.current_layer_id()
    };
    for (i, source) in f.sources.iter().enumerate() {
        let geometry = match source {
            Source::Object(object) => object.geometry(tolerance)?,
            Source::Box(_) => Geometry::Brep(Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 10.]; 3],
                tolerance,
            )?),
        };
        constructed.push(geometry_record(&geometry, tolerance)?);
        let id = document.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(layer)
                .with_name(format!("source-{i}"))
                .with_object_color(ColorRgb::new(10 + i as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(document.add_group(Some(format!("source-{i}")), [id])?);
    }
    if f.preselect {
        document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    }
    let before = snapshot(&document, &ids, &groups)?;
    if !f.preselect {
        // Command-first SelID uses the surface filter before accepting picks.
        let eligible = ids
            .iter()
            .copied()
            .filter(|id| {
                viboceros_command::ObjectSelectionFilter::Surfaces
                    .accepts_object(document.object(*id).unwrap())
            })
            .collect::<Vec<_>>();
        document.select_objects_direct(eligible, SelectionMode::Replace)?;
    }
    let input = format!(
        "{command} KeepTrimObjects={}",
        if f.keep_trim_objects { "Yes" } else { "No" }
    );
    let result = if f.preselect {
        registry.execute(&mut document, &input)
    } else {
        registry.execute_postselected(
            &mut document,
            &input,
            viboceros_command::CommandContext::default(),
        )
    };
    Ok((
        json!({"constructed":constructed,"before":before,
        "after":snapshot(&document,&ids,&groups)?,"succeeded":result.is_ok()}),
        0,
    ))
}

pub(super) fn snapshot(
    document: &Document,
    ids: &[ObjectId],
    groups: &[viboceros_document::GroupId],
) -> Result<Vec<Value>, ProbeError> {
    document.objects().map(|object| {
        let color=object.attributes().object_color();
        Ok(json!({
        "source":ids.iter().position(|id| *id == object.id()),
                "selected":document.is_selected(object.id()), "name":object.attributes().name(),
                "color": [color.red,color.green,color.blue],
                "color_source": match object.attributes().color_source() {
                    viboceros_document::ObjectColorSource::Layer => "ColorFromLayer",
                    viboceros_document::ObjectColorSource::Object => "ColorFromObject",
                    viboceros_document::ObjectColorSource::Material => "ColorFromMaterial",
                    viboceros_document::ObjectColorSource::Parent => "ColorFromParent",
                },
        "current_layer":object.attributes().layer_id() == document.current_layer_id(),
        "groups":object.group_ids().iter().filter_map(|id| groups.iter().position(|g| g == id)).collect::<Vec<_>>(),
                "geometry": match object.geometry() {
                    // Rhino stores AddSurface as a natural one-face B-rep.
                    // Normalize that wrapper only for document snapshots;
                    // constructed records retain the original surface input.
                    Geometry::NurbsSurface(surface) => geometry_record(&Geometry::Brep(
                        Brep::try_surface_face_with_native_edge_parameters(surface.clone(),document.tolerance())?),document.tolerance())?,
                    geometry => geometry_record(geometry, document.tolerance())?,
                },
    }))}).collect()
}

pub(super) fn geometry_record(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<Value, ProbeError> {
    Ok(match geometry {
        Geometry::Point(point) => json!({"type":"point", "point":point.to_array()}),
        Geometry::NurbsSurface(surface) => {
            json!({"type":"surface", "definition":nurbs_surface_definition_value(surface)})
        }
        Geometry::Brep(brep) => {
            json!({"type":"brep", "definition":brep_interchange::definition_record(brep)?,
            "untrimmed":brep.faces().iter().map(|face| face.is_untrimmed(tolerance)).collect::<Result<Vec<_>,_>>()?})
        }
        _ => json!({"type":"curve", "definition":nurbs_curve_definition_value(&geometry.curve_ref()
            .ok_or(ProbeError::FixtureInvariant("unexpected UntrimAll source geometry"))?.to_nurbs()?)}),
    })
}
