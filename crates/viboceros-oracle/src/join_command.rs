//! Actual Join commands with native geometry and document-state records.
use super::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct JoinFixture {
    #[serde(default)]
    command: JoinAction,
    sources: Vec<Source>,
    selected: Option<Vec<usize>>,
    #[serde(default)]
    join_disjoint: bool,
    #[serde(default)]
    preselect: bool,
    absolute_tolerance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Source {
    Brep {
        brep: crate::brep_source::BrepSourceFixture,
    },
    Mesh {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    Curve(crate::curve_join_close::CurveInput),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
enum JoinAction {
    #[default]
    Join,
    JoinCopy,
}

impl JoinAction {
    fn name(self) -> &'static str {
        match self {
            Self::Join => "Join",
            Self::JoinCopy => "JoinCopy",
        }
    }
}

impl Source {
    fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        match self {
            Self::Brep { brep } => {
                let geometry = Geometry::Brep(brep.build(tolerance)?);
                if let Some(path) = &brep.artifact_path {
                    crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
                }
                Ok(geometry)
            }
            Self::Mesh { vertices, faces } => crate::object_source::ObjectSource::Vertices(
                crate::object_source::VertexSource::Mesh {
                    vertices: vertices.clone(),
                    faces: faces.clone(),
                },
            )
            .geometry(tolerance),
            Self::Curve(curve) => Ok(Geometry::from(curve.geometry()?)),
        }
    }
}

pub(super) fn run(f: &JoinFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let tolerance = if let Some(absolute) = f.absolute_tolerance {
        Tolerance::try_new(absolute, tolerance.relative(), tolerance.angular())?
    } else {
        tolerance
    };
    let invalid = || ProbeError::FixtureInvariant("invalid join fixture");
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
        let source = source.geometry(tolerance)?;
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
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "{} JoinDisjointMeshes={}",
        f.command.name(),
        if f.join_disjoint { "Yes" } else { "No" }
    );
    let prompt = registry
        .object_selection_prompt(&command)?
        .ok_or_else(invalid)?;
    for &index in &order {
        document.select_objects_direct([ids[index]], SelectionMode::Add)?;
        if !f.preselect && registry.object_selection_complete(&document, &prompt)? {
            break;
        }
    }
    let result = if f.preselect {
        registry.execute(&mut document, &command)
    } else {
        registry.execute_postselected(&mut document, &command, Default::default())
    };
    let succeeded = match result {
        Ok(_) => true,
        Err(
            CommandError::NoOpenCurvesToJoin
            | CommandError::NoOpenSurfacesToJoin
            | CommandError::NothingJoined,
        ) => false,
        Err(error) => return Err(error.into()),
    };
    let objects = document.objects().map(|object| {
        let attributes = object.attributes();
        let color = attributes.object_color();
        let mut memberships = object.group_ids().iter().map(|g| groups.iter().position(|i|i==g).unwrap()).collect::<Vec<_>>();
        memberships.sort_unstable();
        let mut record = json!({"source":ids.iter().position(|id|*id==object.id()),
            "selected":document.is_selected(object.id()),"name":attributes.name(),
            "layer":layers.iter().position(|l|*l==attributes.layer_id()),"color":[color.red,color.green,color.blue],
            "groups":memberships});
        if let Geometry::Mesh(mesh) = object.geometry() {
            record["mesh"] = polygon_mesh_value(mesh);
        } else if let Geometry::Brep(brep) = object.geometry() {
            record["brep"] = crate::brep_join::geometry_record(brep,tolerance)?;
        } else if let Some(curve) = object.geometry().curve_ref() {
            record["curve"] = crate::curve_interchange::curve_record(curve)?;
        } else { return Err(invalid()); }
        Ok(record)
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    let mut result = json!({"succeeded":succeeded,"objects":objects});
    if f.absolute_tolerance.is_some() {
        result["absolute_tolerance"] = json!(tolerance.absolute());
    }
    Ok((result, 0))
}
