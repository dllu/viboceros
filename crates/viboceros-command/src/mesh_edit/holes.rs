//! Single and multiple mesh-hole filling.
use crate::*;

use super::edges::{MeshTopologyEdgeSelection, closest_mesh_topology_edge};

const FILL_MESH_HOLE_USAGE: &str = "FillMeshHole (point|Edge=index) [JoinMesh=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
struct FillMeshHoleOptions {
    selection: MeshTopologyEdgeSelection,
    join_mesh: bool,
}

struct FillMeshHoleSource<'a> {
    id: ObjectId,
    mesh: &'a TriangleMesh,
    attributes: &'a ObjectAttributes,
}

struct FillMeshHolePlan {
    source: ObjectId,
    filled: TriangleMesh,
    patch: TriangleMesh,
    attributes: ObjectAttributes,
}

pub(crate) struct FillMeshHoleCommand;

impl Command for FillMeshHoleCommand {
    fn name(&self) -> &'static str {
        "FillMeshHole"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_fill_mesh_hole_arguments(arguments)?;
        let sources = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedFillMeshHoleGeometry);
                };
                Ok(FillMeshHoleSource {
                    id: object.id(),
                    mesh,
                    attributes: object.attributes(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let topology_sources = sources
            .iter()
            .map(|source| MeshTopologySource {
                id: source.id,
                mesh: source.mesh,
            })
            .collect::<Vec<_>>();
        let selections = selected_fill_mesh_hole_edges(
            &topology_sources,
            &options.selection,
            document.tolerance(),
        )?;
        if selections.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("FillMeshHole"));
        }

        let mut plans = Vec::new();
        for (source_index, edge_index) in selections {
            let source = &sources[source_index];
            let Some(fill) = source
                .mesh
                .fill_topology_hole(edge_index, document.tolerance())?
            else {
                continue;
            };
            let (filled, patch) = fill.into_parts();
            plans.push(FillMeshHolePlan {
                source: source.id,
                filled,
                patch,
                attributes: source.attributes.clone(),
            });
        }
        if plans.is_empty() {
            return Err(CommandError::NoFillableMeshHoles);
        }
        let filled_face_count = plans
            .iter()
            .map(|plan| plan.patch.face_count())
            .sum::<usize>();
        let filled_hole_count = plans.len();

        if options.join_mesh {
            let changed_mesh_count = document.replace_object_geometries(
                plans
                    .into_iter()
                    .map(|plan| (plan.source, Geometry::Mesh(plan.filled))),
            )?;
            Ok(format!(
                "Filled {filled_hole_count} mesh hole(s) with {filled_face_count} triangle(s) in {changed_mesh_count} joined mesh(es)"
            ))
        } else {
            let mut output_ids = Vec::with_capacity(plans.len());
            for plan in plans {
                output_ids.push(
                    document.add_geometry_with_attributes(
                        Geometry::Mesh(plan.patch),
                        plan.attributes,
                    )?,
                );
            }
            document.select_objects_direct(output_ids, SelectionMode::Replace)?;
            Ok(format!(
                "Filled {filled_hole_count} mesh hole(s) with {filled_face_count} triangle(s) in separate patch mesh(es)"
            ))
        }
    }
}

fn selected_fill_mesh_hole_edges(
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
                    return Err(CommandError::FillMeshHoleEdgeIndexOutOfRange {
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
                return Err(CommandError::NoFillableMeshHoles);
            };
            Ok(vec![(source, edge)])
        }
    }
}

fn parse_fill_mesh_hole_arguments(arguments: &[&str]) -> Result<FillMeshHoleOptions, CommandError> {
    let mut edge = None;
    let mut join_mesh = true;
    let mut join_mesh_seen = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Edge")
            || option_name_eq(argument, "EdgeIndex")
            || option_name_eq(argument, "JoinMesh")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(FILL_MESH_HOLE_USAGE))?;
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
                    .map_err(|_| CommandError::Usage(FILL_MESH_HOLE_USAGE))?,
            );
        } else if option_name_eq(name, "JoinMesh") && !join_mesh_seen {
            join_mesh = parse_yes_no(value).ok_or(CommandError::Usage(FILL_MESH_HOLE_USAGE))?;
            join_mesh_seen = true;
        } else {
            return Err(CommandError::Usage(FILL_MESH_HOLE_USAGE));
        }
        index += consumed;
    }

    let selection = if let Some(edge) = edge {
        require_consumed(&positional, 0, FILL_MESH_HOLE_USAGE)?;
        MeshTopologyEdgeSelection::Edge(edge)
    } else {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, FILL_MESH_HOLE_USAGE)?;
        MeshTopologyEdgeSelection::Point(point)
    };
    Ok(FillMeshHoleOptions {
        selection,
        join_mesh,
    })
}

pub(crate) struct FillMeshHolesCommand;

impl Command for FillMeshHolesCommand {
    fn name(&self) -> &'static str {
        "FillMeshHoles"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "FillMeshHoles")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let selected_mesh_count = selected.len();
        let mut filled_hole_count = 0_usize;
        let mut filled_face_count = 0_usize;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                return Err(CommandError::UnsupportedFillMeshHolesGeometry);
            };
            let source_face_count = mesh.face_count();
            let (filled, hole_count) = mesh.fill_holes(document.tolerance())?;
            if hole_count == 0 {
                continue;
            }
            filled_hole_count = filled_hole_count
                .checked_add(hole_count)
                .ok_or(GeometryError::TooManyMeshFaces)?;
            filled_face_count = filled_face_count
                .checked_add(filled.face_count() - source_face_count)
                .ok_or(GeometryError::TooManyMeshFaces)?;
            replacements.push((id, Geometry::Mesh(filled)));
        }
        if replacements.is_empty() {
            return Err(CommandError::NoFillableMeshHoles);
        }

        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Filled {filled_hole_count} mesh hole(s) with {filled_face_count} triangle(s) in {changed_mesh_count} mesh(es); {} mesh(es) unchanged",
            selected_mesh_count - changed_mesh_count
        ))
    }
}
