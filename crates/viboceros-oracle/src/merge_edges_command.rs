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
    run_impl(&f.base, construction, EdgeAction::Merge(f))
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SplitEdgeFixture {
    #[serde(flatten)]
    base: MergeEdgesFixture,
    edge: usize,
    #[serde(default, deserialize_with = "present_split_input")]
    parameters: Option<Vec<f64>>,
    #[serde(default, deserialize_with = "present_split_input")]
    inputs: Option<Vec<SplitEdgeInput>>,
    pick: Option<String>,
    finish: Option<String>,
    #[serde(default)]
    object_preselect: bool,
    #[serde(default)]
    record_viewport: bool,
    #[serde(default)]
    persistent_snaps: Vec<SplitEdgeSnap>,
    #[serde(default, deserialize_with = "present_split_input")]
    snap_to_meshes: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum SplitEdgeInput {
    Point(f64),
    Mouse(f64),
    Distance(f64),
    Pick(SplitEdgePick),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SplitEdgePick {
    point: [f64; 3],
    #[serde(default, deserialize_with = "present_split_input")]
    aim: Option<[f64; 3]>,
    osnap: SplitEdgeSnap,
    #[serde(default)]
    offset: [i32; 2],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
enum SplitEdgeSnap {
    NoSnap,
    Point,
    End,
    Mid,
    Cen,
    Quad,
    Near,
    Persistent,
}

// Missing alternatives are optional; an explicitly present null is not a list.
fn present_split_input<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

pub(super) fn run_split(
    f: &SplitEdgeFixture,
    construction: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.snap_to_meshes.is_some() {
        return Err(ProbeError::FixtureInvariant(
            "mesh snap picks require calibrated camera-derived targets",
        ));
    }
    if f.base.preselect
        || f.base.cancel
        || f.parameters.is_some() == f.inputs.is_some()
        || (f.record_viewport
            && f.inputs.as_ref().is_none_or(|steps| {
                !steps
                    .iter()
                    .any(|step| matches!(step, SplitEdgeInput::Pick(_) | SplitEdgeInput::Mouse(_)))
            }))
        || f.persistent_snaps.len() > 6
        || f.persistent_snaps.iter().enumerate().any(|(i, mode)| {
            matches!(mode, SplitEdgeSnap::NoSnap | SplitEdgeSnap::Persistent)
                || f.persistent_snaps[..i].contains(mode)
        })
        || f.parameters
            .as_ref()
            .is_some_and(|v| v.len() > 64 || v.iter().any(|t| !t.is_finite()))
        || f.inputs.as_ref().is_some_and(|v| {
            v.len() > 64
                || v.iter().any(|step| match step {
                    SplitEdgeInput::Point(t)
                    | SplitEdgeInput::Mouse(t)
                    | SplitEdgeInput::Distance(t) => !t.is_finite(),
                    SplitEdgeInput::Pick(p) => {
                        p.point.iter().any(|v| !v.is_finite())
                            || p.aim.is_some_and(|aim| aim.iter().any(|v| !v.is_finite()))
                            || (p.osnap == SplitEdgeSnap::Persistent
                                && f.persistent_snaps.is_empty())
                            || p.offset.iter().any(|v| !(-32..=32).contains(v))
                    }
                })
        })
        || f.pick.as_deref() != Some("mouse")
        || f.finish
            .as_deref()
            .is_some_and(|s| !matches!(s, "Enter" | "Cancel"))
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid SplitEdge command fixture",
        ));
    }
    if f.inputs.as_ref().is_some_and(|inputs| {
        inputs
            .iter()
            .any(|step| matches!(step, SplitEdgeInput::Pick(p) if p.osnap == SplitEdgeSnap::NoSnap))
    }) {
        return Err(ProbeError::FixtureInvariant(
            "NoSnap screen controls require recorded viewport calibration",
        ));
    }
    if f.inputs.as_ref().is_some_and(|inputs| inputs.iter().any(|step| {
        matches!(step, SplitEdgeInput::Pick(p) if p.osnap == SplitEdgeSnap::Near
            || (p.osnap == SplitEdgeSnap::Persistent && f.persistent_snaps.contains(&SplitEdgeSnap::Near)))
    })) {
        return Err(ProbeError::FixtureInvariant("Near picks require calibrated camera-derived targets"));
    }
    run_impl(&f.base, construction, EdgeAction::Split(f))
}

#[derive(Clone, Copy)]
enum EdgeAction<'a> {
    All,
    Merge(&'a SelectedEdgeFixture),
    Split(&'a SplitEdgeFixture),
}
impl EdgeAction<'_> {
    fn name(self) -> &'static str {
        match self {
            Self::All => "MergeAllEdges",
            Self::Merge(_) => "MergeEdge",
            Self::Split(_) => "SplitEdge",
        }
    }
    fn component(self) -> bool {
        !matches!(self, Self::All)
    }
    fn preselect(self) -> bool {
        match self {
            Self::All => false,
            Self::Merge(f) => f.object_preselect,
            Self::Split(f) => f.object_preselect,
        }
    }
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
    run_impl(f, construction, EdgeAction::All)
}

fn run_impl(
    f: &MergeEdgesFixture,
    construction: Tolerance,
    action: EdgeAction<'_>,
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
        || (action.component() && order.len() != 1)
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
    if f.preselect || action.preselect() {
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
    if !f.preselect && !action.component() {
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
    let succeeded = if let EdgeAction::Split(selected) = action {
        document.clear_selection();
        let mut session = viboceros_command::SplitEdgeSelection::prepare(
            &document,
            ids[order[0]],
            selected.edge,
        )?;
        let inputs = selected.inputs.clone().unwrap_or_else(|| {
            selected
                .parameters
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .map(SplitEdgeInput::Point)
                .collect()
        });
        for step in inputs {
            match step {
                SplitEdgeInput::Distance(distance) => session.set_distance(distance)?,
                SplitEdgeInput::Pick(pick) => {
                    // Replay the declared model-space snap target. Native feature
                    // discovery/capture is tested through the production viewport;
                    // this command adapter does not pretend to replay screen pixels.
                    session.add_point(Point3::try_from(pick.point)?)?;
                }
                SplitEdgeInput::Point(parameter) | SplitEdgeInput::Mouse(parameter) => {
                    // Replay the requested model location, not the quantized screen
                    // coordinates. Camera/pixel equivalence needs separate UI tests.
                    let point = session.curve().evaluate(parameter)?;
                    session.add_point(point)?;
                }
            }
        }
        if session.parameters().is_empty() {
            false
        } else {
            match session.commit(&mut document) {
                Ok(_) => true,
                Err(CommandError::Geometry(GeometryError::InvalidCurveSplitParameter)) => false,
                Err(error) => return Err(error.into()),
            }
        }
    } else if let EdgeAction::Merge(selected) = action {
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
        let changed = document.undo_label() == Some(action.name());
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
