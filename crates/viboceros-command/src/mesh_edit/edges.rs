//! Mesh edge picking, parsing and topology edits.
use crate::*;

const SWAP_MESH_EDGE_USAGE: &str = "SwapMeshEdge (point|Edge=index)";

#[derive(Clone, Debug, PartialEq)]
pub(super) enum MeshTopologyEdgeSelection {
    Point(Point3),
    Edge(usize),
}

pub(crate) struct SwapMeshEdgeCommand;

impl Command for SwapMeshEdgeCommand {
    fn name(&self) -> &'static str {
        "SwapMeshEdge"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_mesh_topology_edge_arguments(arguments, SWAP_MESH_EDGE_USAGE)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedSwapMeshEdgeGeometry
        })?;

        let selections = selected_swap_mesh_edges(&sources, &selection, document.tolerance())?;
        let mesh_count = sources.len();
        let mut replacements = Vec::new();
        for (source_index, edge_index) in selections {
            let source = &sources[source_index];
            if let Some(swapped) = source
                .mesh
                .swap_topology_edge(edge_index, document.tolerance())?
            {
                replacements.push((source.id, Geometry::Mesh(swapped)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoSwappableMeshEdges);
        }
        let swapped_edge_count = replacements.len();
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Swapped {swapped_edge_count} mesh edge(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn selected_swap_mesh_edges(
    sources: &[MeshTopologySource<'_>],
    selection: &MeshTopologyEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, usize)>, CommandError> {
    match selection {
        MeshTopologyEdgeSelection::Edge(edge) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.mesh.topology().edge_count();
                if *edge >= edge_count {
                    return Err(CommandError::SwapMeshEdgeIndexOutOfRange {
                        edge: *edge,
                        edge_count,
                    });
                }
                Ok((source_index, *edge))
            })
            .collect(),
        MeshTopologyEdgeSelection::Point(target) => {
            let Some((source, edge)) = closest_mesh_topology_edge(sources, *target, tolerance)?
            else {
                return Err(CommandError::NoSwappableMeshEdges);
            };
            Ok(vec![(source, edge)])
        }
    }
}

pub(super) fn closest_mesh_topology_edge(
    sources: &[MeshTopologySource<'_>],
    target: Point3,
    tolerance: Tolerance,
) -> Result<Option<(usize, usize)>, GeometryError> {
    let mut best = None;
    for (source_index, source) in sources.iter().enumerate() {
        for (edge_index, edge) in source
            .mesh
            .wireframe_lines(tolerance)?
            .into_iter()
            .enumerate()
        {
            let closest = edge.closest_point(target, tolerance)?;
            let distance = closest.distance_to(target)?;
            if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                best = Some((distance, source_index, edge_index));
            }
        }
    }
    Ok(best.map(|(_, source, edge)| (source, edge)))
}

pub(super) fn parse_mesh_topology_edge_arguments(
    arguments: &[&str],
    usage: &'static str,
) -> Result<MeshTopologyEdgeSelection, CommandError> {
    let mut edge_selection = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Edge") || option_name_eq(argument, "EdgeIndex") {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(usage))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Edge") || option_name_eq(name, "EdgeIndex"))
                && edge_selection.is_none()
            {
                edge_selection = Some(
                    value
                        .trim_start_matches('_')
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(usage))?,
                );
            } else {
                return Err(CommandError::Usage(usage));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    if let Some(edge) = edge_selection {
        require_consumed(&positional, 0, usage)?;
        Ok(MeshTopologyEdgeSelection::Edge(edge))
    } else {
        if positional.is_empty() {
            return Err(CommandError::Usage(usage));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, usage)?;
        Ok(MeshTopologyEdgeSelection::Point(point))
    }
}

const COLLAPSE_MESH_EDGE_USAGE: &str = "CollapseMeshEdge (point|Edge=index)";

pub(crate) struct CollapseMeshEdgeCommand;

impl Command for CollapseMeshEdgeCommand {
    fn name(&self) -> &'static str {
        "CollapseMeshEdge"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_mesh_topology_edge_arguments(arguments, COLLAPSE_MESH_EDGE_USAGE)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedCollapseMeshEdgeGeometry
        })?;

        let selections = selected_collapse_mesh_edges(&sources, &selection, document.tolerance())?;
        let plans = selections
            .into_iter()
            .map(|(source_index, edge_index)| {
                let source = &sources[source_index];
                Ok((
                    source.id,
                    source
                        .mesh
                        .collapse_topology_edge(edge_index, document.tolerance())?,
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let collapsed_edge_count = plans.len();
        let mut replacements = Vec::new();
        let mut deleted = Vec::new();
        for (id, remainder) in plans {
            if let Some(mesh) = remainder {
                replacements.push((id, Geometry::Mesh(mesh)));
            } else {
                deleted.push(id);
            }
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        let deleted_mesh_count = document.delete_objects(deleted)?;
        Ok(format!(
            "Collapsed {collapsed_edge_count} mesh edge(s) in {} mesh(es); deleted {deleted_mesh_count} empty mesh(es)",
            changed_mesh_count + deleted_mesh_count
        ))
    }
}

fn selected_collapse_mesh_edges(
    sources: &[MeshTopologySource<'_>],
    selection: &MeshTopologyEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, usize)>, CommandError> {
    match selection {
        MeshTopologyEdgeSelection::Edge(edge) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.mesh.topology().edge_count();
                if *edge >= edge_count {
                    return Err(CommandError::CollapseMeshEdgeIndexOutOfRange {
                        edge: *edge,
                        edge_count,
                    });
                }
                Ok((source_index, *edge))
            })
            .collect(),
        MeshTopologyEdgeSelection::Point(target) => {
            let Some((source, edge)) = closest_mesh_topology_edge(sources, *target, tolerance)?
            else {
                return Err(CommandError::NoCollapsibleMeshEdges);
            };
            Ok(vec![(source, edge)])
        }
    }
}

const SPLIT_MESH_EDGE_USAGE: &str =
    "SplitMeshEdge (edge-point split-point|Edge=index Parameter=number)";

#[derive(Clone, Debug, PartialEq)]
enum SplitMeshEdgeSelection {
    Points {
        edge_point: Point3,
        split_point: Point3,
    },
    Edge {
        edge: usize,
        parameter: Real,
    },
}

pub(crate) struct SplitMeshEdgeCommand;

impl Command for SplitMeshEdgeCommand {
    fn name(&self) -> &'static str {
        "SplitMeshEdge"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_split_mesh_edge_arguments(arguments)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedSplitMeshEdgeGeometry
        })?;

        let selections = selected_split_mesh_edges(&sources, &selection, document.tolerance())?;
        let replacements = selections
            .into_iter()
            .map(|(source_index, edge_index, parameter)| {
                let source = &sources[source_index];
                let split = source
                    .mesh
                    .split_topology_edge(edge_index, parameter, document.tolerance())?
                    .ok_or(CommandError::NoSplittableMeshEdges)?;
                Ok((source.id, Geometry::Mesh(split)))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let split_edge_count = replacements.len();
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Split {split_edge_count} mesh edge(s) in {changed_mesh_count} mesh(es)"
        ))
    }
}

fn selected_split_mesh_edges(
    sources: &[MeshTopologySource<'_>],
    selection: &SplitMeshEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, usize, Real)>, CommandError> {
    match selection {
        SplitMeshEdgeSelection::Edge { edge, parameter } => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.mesh.topology().edge_count();
                if *edge >= edge_count {
                    return Err(CommandError::SplitMeshEdgeIndexOutOfRange {
                        edge: *edge,
                        edge_count,
                    });
                }
                Ok((source_index, *edge, *parameter))
            })
            .collect(),
        SplitMeshEdgeSelection::Points {
            edge_point,
            split_point,
        } => {
            let Some((source_index, edge_index)) =
                closest_mesh_topology_edge(sources, *edge_point, tolerance)?
            else {
                return Err(CommandError::NoSplittableMeshEdges);
            };
            let edge = sources[source_index].mesh.wireframe_lines(tolerance)?[edge_index];
            let parameter = edge.closest_parameter(*split_point, tolerance)?;
            Ok(vec![(source_index, edge_index, parameter)])
        }
    }
}

fn parse_split_mesh_edge_arguments(
    arguments: &[&str],
) -> Result<SplitMeshEdgeSelection, CommandError> {
    let mut edge = None;
    let mut parameter = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Edge")
            || option_name_eq(argument, "EdgeIndex")
            || option_name_eq(argument, "Parameter")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(SPLIT_MESH_EDGE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        let Some((name, value, consumed)) = option else {
            positional.push(argument);
            index += 1;
            continue;
        };
        if (option_name_eq(name, "Edge") || option_name_eq(name, "EdgeIndex")) && edge.is_none() {
            edge = Some(
                value
                    .trim_start_matches('_')
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(SPLIT_MESH_EDGE_USAGE))?,
            );
        } else if option_name_eq(name, "Parameter") && parameter.is_none() {
            parameter = Some(parse_finite_real(value.trim_start_matches('_'))?);
        } else {
            return Err(CommandError::Usage(SPLIT_MESH_EDGE_USAGE));
        }
        index += consumed;
    }

    if edge.is_some() || parameter.is_some() {
        require_consumed(&positional, 0, SPLIT_MESH_EDGE_USAGE)?;
        let (Some(edge), Some(parameter)) = (edge, parameter) else {
            return Err(CommandError::Usage(SPLIT_MESH_EDGE_USAGE));
        };
        if !(0.0..=1.0).contains(&parameter) {
            return Err(CommandError::InvalidMeshEdgeSplitParameter(parameter));
        }
        Ok(SplitMeshEdgeSelection::Edge { edge, parameter })
    } else {
        let (edge_point, edge_consumed) = parse_point(&positional)?;
        let (split_point, split_consumed) = parse_point(&positional[edge_consumed..])?;
        require_consumed(
            &positional,
            edge_consumed + split_consumed,
            SPLIT_MESH_EDGE_USAGE,
        )?;
        Ok(SplitMeshEdgeSelection::Points {
            edge_point,
            split_point,
        })
    }
}
