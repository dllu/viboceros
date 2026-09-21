//! Actual edge cleanup, selection, attributes and undo on shared B-rep inputs.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MergeEdgesFixture {
    sources: Vec<Source>,
    selected: Option<Vec<usize>>,
    #[serde(default)]
    preselect: bool,
    #[serde(default)]
    undo_redo: bool,
    #[serde(default)]
    cancel: bool,
    absolute_tolerance: Option<f64>,
    angular_tolerance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SelectedEdgeFixture {
    #[serde(flatten)]
    base: MergeEdgesFixture,
    edge: usize,
    choice: Option<String>,
    pick: Option<String>,
    #[serde(default)]
    object_preselect: bool,
}

pub(super) fn run_selected(
    f: &SelectedEdgeFixture,
    construction: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.base.preselect
        || f.base.cancel
        || f.pick.as_deref().is_some_and(|p| p != "mouse")
        || f.choice.as_deref().is_some_and(|c| {
            c != "Cancel" && c != "Auto" && viboceros_command::MergeEdgeChoice::parse(c).is_none()
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid selected-edge command fixture",
        ));
    }
    run_impl(&f.base, construction, Some(f))
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Source {
    Brep {
        brep: Box<crate::brep_source::BrepSourceFixture>,
    },
    Other(crate::object_source::ObjectSource),
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
            Self::Other(source) => {
                let geometry = source.geometry(tolerance)?;
                if matches!(geometry, Geometry::Brep(_)) {
                    return Err(ProbeError::FixtureInvariant(
                        "edge merge B-reps require shared-source wrappers",
                    ));
                }
                Ok(geometry)
            }
        }
    }
}

pub(super) fn run(
    f: &MergeEdgesFixture,
    construction: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_impl(f, construction, None)
}

fn run_impl(
    f: &MergeEdgesFixture,
    construction: Tolerance,
    selected_edge: Option<&SelectedEdgeFixture>,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid edge merge fixture");
    let order = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..f.sources.len()).collect());
    if f.sources.is_empty()
        || f.sources.len() > 32
        || order.is_empty()
        || order.iter().any(|&i| i >= f.sources.len())
        || order.iter().collect::<BTreeSet<_>>().len() != order.len()
        || (f.cancel && f.preselect)
        || (selected_edge.is_some() && order.len() != 1)
    {
        return Err(invalid());
    }
    let tolerance = Tolerance::try_new(
        f.absolute_tolerance.unwrap_or(construction.absolute()),
        construction.relative(),
        f.angular_tolerance.unwrap_or(construction.angular()),
    )?;
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    let mut groups = Vec::new();
    for (index, source) in f.sources.iter().enumerate() {
        let layer = document.add_layer(format!("Source {index}"), ColorRgb::BLACK)?;
        layers.push(layer);
        ids.push(
            document.add_geometry_with_attributes(
                source.geometry(construction)?,
                ObjectAttributes::on_layer(layer)
                    .with_name(format!("source-{index}"))
                    .with_object_color(ColorRgb::new(11 + index as u8, 22, 33)),
            )?,
        );
    }
    for (i, &id) in ids.iter().enumerate() {
        groups.push(document.add_group(Some(format!("Source {i}")), [id])?);
    }
    groups.push(document.add_group(Some("Shared".into()), ids.iter().copied())?);
    let registry = CommandRegistry::with_builtins();
    if f.preselect || selected_edge.is_some_and(|f| f.object_preselect) {
        document.select_objects_direct(order.iter().map(|&i| ids[i]), SelectionMode::Replace)?;
    }
    let snapshot = |document: &Document| -> Result<Value, ProbeError> {
        let mut objects = document.objects().collect::<Vec<_>>();
        objects.sort_by_key(|o| ids.iter().position(|&id| id == o.id()).unwrap_or(ids.len()));
        let rows = objects.into_iter().map(|object| {
            let attributes = object.attributes();
            let color = attributes.object_color();
            let mut memberships = object.group_ids().iter().filter_map(|g| groups.iter().position(|id| id == g)).collect::<Vec<_>>();
            memberships.sort_unstable();
            let geometry = match object.geometry() {
                Geometry::Brep(brep) => json!({"brep": crate::brep_join::geometry_record(brep, construction)?}),
                Geometry::NurbsSurface(surface) => json!({"brep": crate::brep_join::geometry_record(&Brep::try_surface_face(surface.clone(), construction)?, construction)?}),
                Geometry::Point(point) => json!({"point": point.to_array()}),
                Geometry::PointCloud(cloud) => json!({"point_cloud": cloud.points().iter().map(|p| p.to_array()).collect::<Vec<_>>()}),
                Geometry::Mesh(mesh) => json!({"mesh": polygon_mesh_value(mesh)}),
                geometry => json!({"curve": crate::curve_interchange::curve_record(geometry.curve_ref().ok_or_else(invalid)?)?}),
            };
            Ok(json!({"source": ids.iter().position(|&id| id == object.id()),
                "selected": document.is_selected(object.id()), "name": attributes.name(),
                "layer": layers.iter().position(|&id| id == attributes.layer_id()),
                "color": [color.red, color.green, color.blue], "color_source": format!("ColorFrom{:?}", attributes.color_source()),
                "groups": memberships, "geometry": geometry}))
        }).collect::<Result<Vec<_>, ProbeError>>()?;
        Ok(json!(rows))
    };
    let before = snapshot(&document)?;
    if !f.preselect && selected_edge.is_none() {
        let prompt = registry
            .object_selection_prompt("MergeAllEdges")?
            .ok_or_else(invalid)?;
        let accepted = order
            .iter()
            .map(|&i| ids[i])
            .filter(|&id| prompt.filter.accepts_object(document.object(id).unwrap()))
            .collect::<Vec<_>>();
        document.select_objects_direct(accepted, SelectionMode::Replace)?;
    }
    let succeeded = if let Some(selected) = selected_edge {
        document.clear_selection();
        let selection = viboceros_command::MergeEdgeSelection::prepare(
            &document,
            ids[order[0]],
            selected.edge,
        )?;
        let choice = selected.choice.as_deref().unwrap_or("All");
        if matches!(choice, "Cancel" | "Auto") || selection.choices().is_empty() {
            false
        } else {
            registry.execute(
                &mut document,
                &format!("MergeEdge {} {} {choice}", ids[order[0]], selected.edge),
            )?;
            true
        }
    } else if f.cancel {
        document.clear_selection();
        false
    } else {
        let result = if f.preselect {
            registry.execute(&mut document, "MergeAllEdges")
        } else {
            registry.execute_postselected(&mut document, "MergeAllEdges", Default::default())
        };
        match result {
            Ok(_) => true,
            Err(
                CommandError::NoObjectsSelected | CommandError::UnsupportedMergeAllEdgesGeometry,
            ) => false,
            Err(error) => return Err(error.into()),
        }
    };
    let after = snapshot(&document)?;
    let mut result = json!({"before": before, "after": after, "succeeded": succeeded,
        "absolute_tolerance": tolerance.absolute(), "angular_tolerance": tolerance.angular()});
    if f.undo_redo {
        let changed = document.undo_label()
            == Some(if selected_edge.is_some() {
                "MergeEdge"
            } else {
                "MergeAllEdges"
            });
        result["history_tested"] = json!(changed);
        if changed {
            for (command, key) in [("Undo", "undo"), ("Redo", "redo")] {
                registry.execute(&mut document, command)?;
                result[key] = snapshot(&document)?;
            }
        }
    }
    Ok((result, 0))
}
