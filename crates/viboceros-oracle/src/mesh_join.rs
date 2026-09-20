//! Actual mesh Join commands with exact raw-index and document-state records.
use super::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MeshJoinFixture {
    sources: Vec<Source>,
    selected: Option<Vec<usize>>,
    #[serde(default)]
    join_disjoint: bool,
    #[serde(default)]
    preselect: bool,
    absolute_tolerance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    vertices: Vec<[f64; 3]>,
    faces: Vec<Vec<u32>>,
}

pub(super) fn run(f: &MeshJoinFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let tolerance = if let Some(absolute) = f.absolute_tolerance {
        Tolerance::try_new(absolute, tolerance.relative(), tolerance.angular())?
    } else {
        tolerance
    };
    let invalid = || ProbeError::FixtureInvariant("invalid mesh join fixture");
    let order = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..f.sources.len()).collect());
    if f.sources.is_empty()
        || f.sources.len() > 32
        || order.is_empty()
        || order.iter().any(|i| *i >= f.sources.len())
        || order.iter().collect::<BTreeSet<_>>().len() != order.len()
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    let mut groups = Vec::new();
    for (index, source) in f.sources.iter().enumerate() {
        let layer = document.add_layer(format!("Source {index}"), ColorRgb::BLACK)?;
        layers.push(layer);
        let source = crate::object_source::ObjectSource::Vertices(
            crate::object_source::VertexSource::Mesh {
                vertices: source.vertices.clone(),
                faces: source.faces.clone(),
            },
        )
        .geometry(tolerance)?;
        ids.push(
            document.add_geometry_with_attributes(
                source,
                ObjectAttributes::on_layer(layer)
                    .with_name(format!("source-{index}"))
                    .with_object_color(ColorRgb::new(10 + index as u8, 30, 50)),
            )?,
        );
    }
    for (index, &id) in ids.iter().enumerate() {
        groups.push(document.add_group(Some(format!("Source {index}")), [id])?);
    }
    groups.push(document.add_group(Some("Shared".into()), ids.iter().copied())?);
    for &index in &order {
        document.select_objects_direct([ids[index]], SelectionMode::Add)?;
    }
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "Join JoinDisjointMeshes={}",
        if f.join_disjoint { "Yes" } else { "No" }
    );
    if f.preselect {
        registry.execute(&mut document, &command)?;
    } else {
        registry.execute_postselected(&mut document, &command, Default::default())?;
    }
    let objects = document.objects().map(|object| {
        let Geometry::Mesh(mesh) = object.geometry() else { return Err(invalid()); };
        let attributes = object.attributes();
        let color = attributes.object_color();
        let mut memberships = object.group_ids().iter().map(|g| groups.iter().position(|i|i==g).unwrap()).collect::<Vec<_>>();
        memberships.sort_unstable();
        Ok(json!({"mesh":polygon_mesh_value(mesh),"source":ids.iter().position(|id|*id==object.id()),
            "selected":document.is_selected(object.id()),"name":attributes.name(),
            "layer":layers.iter().position(|l|*l==attributes.layer_id()),"color":[color.red,color.green,color.blue],
            "groups":memberships}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    let mut result = json!({"succeeded":true,"objects":objects});
    if f.absolute_tolerance.is_some() {
        result["absolute_tolerance"] = json!(tolerance.absolute());
    }
    Ok((result, 0))
}
