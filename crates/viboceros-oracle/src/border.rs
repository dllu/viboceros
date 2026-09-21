//! Actual border commands and representation-preserving output records.
use super::*;
use crate::brep_source::{BrepCommandSource, reorder_edges, write_shared_artifact};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BorderFixture {
    command: String,
    source: BrepCommandSource,
    output_layer: Option<String>,
    faces: Option<Vec<usize>>,
    #[serde(default)]
    preselect: bool,
    artifact_path: Option<String>,
    /// New edge order, expressed as old indices; geometry is unchanged.
    edge_order: Option<Vec<usize>>,
}

pub(super) fn run(f: &BorderFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid border fixture");
    if !matches!(f.command.as_str(), "DupBorder" | "DupFaceBorder") {
        return Err(invalid());
    }
    let layer = f.output_layer.as_deref().unwrap_or("Current");
    if !matches!(layer, "Current" | "Input") {
        return Err(invalid());
    }
    if let Some(faces) = &f.faces
        && (!f.preselect
            || faces.is_empty()
            || faces.iter().collect::<BTreeSet<_>>().len() != faces.len())
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let input_layer = document.add_layer("Source", ColorRgb::BLACK)?;
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let mut geometry = f.source.geometry(tolerance)?;
    if let Some(order) = &f.edge_order {
        let Geometry::Brep(brep) = &geometry else {
            return Err(invalid());
        };
        geometry = Geometry::Brep(reorder_edges(brep, order, tolerance)?);
    }
    if let Some(path) = &f.artifact_path {
        write_shared_artifact(&geometry, path, tolerance)?;
    }

    let source = document.add_geometry_with_attributes(
        geometry,
        ObjectAttributes::on_layer(input_layer)
            .with_name("Source")
            .with_object_color(ColorRgb::new(11, 22, 33)),
    )?;
    document.add_group(Some("Source group".into()), [source])?;
    document.select_objects_direct([source], SelectionMode::Replace)?;
    let mut command = format!("{} OutputLayer={layer}", f.command);
    if let Some(faces) = &f.faces {
        command.push_str(&format!(
            " Faces={}",
            faces
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    } else if f.command == "DupFaceBorder" {
        command.push_str(" Faces=All");
    }
    let registry = CommandRegistry::with_builtins();
    let result = if f.preselect {
        registry.execute(&mut document, &command)
    } else {
        registry.execute_postselected(&mut document, &command, Default::default())
    };
    result?;
    let outputs=document.objects().filter(|o|o.id()!=source).map(|o| {
        let attrs=o.attributes();let color=attrs.object_color();
        Ok(json!({"curve":crate::curve_interchange::curve_record(o.geometry().curve_ref().ok_or_else(invalid)?)?,
            "selected":document.is_selected(o.id()),"name":attrs.name(),
            "layer":if attrs.layer_id()==input_layer {"Source"} else if attrs.layer_id()==current_layer {"Current"} else {"Unexpected"},
            "color":[color.red,color.green,color.blue],"color_source":format!("ColorFrom{:?}",attrs.color_source()),
            "group_count":o.group_ids().len()}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok((
        json!({"succeeded":true,"source_retained":document.object(source).is_some(),"source_selected":document.is_selected(source),"outputs":outputs,"new_groups":document.groups().len()-1}),
        0,
    ))
}
