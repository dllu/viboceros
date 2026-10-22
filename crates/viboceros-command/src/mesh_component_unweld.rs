//! Selected seam commands, component picking and stored-normal policies.
use super::*;

const UNWELD_EDGE_USAGE: &str = "UnweldEdge (point|Edges=All|Edges=0,2,...) [ModifyNormals=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
enum UnweldEdgeSelection {
    Point(Point3),
    Edges(SurfaceFaceIndices),
}

#[derive(Clone, Debug, PartialEq)]
struct UnweldEdgeOptions {
    selection: UnweldEdgeSelection,
    modify_normals: bool,
}

pub(super) struct UnweldEdgeCommand;

impl Command for UnweldEdgeCommand {
    fn name(&self) -> &'static str {
        "UnweldEdge"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["UnweldMeshEdge"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_unweld_edge_arguments(arguments)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedUnweldEdgeGeometry
        })?;

        let selections = selected_unweld_edges(&sources, &options.selection, document.tolerance())?;
        let mesh_count = sources.len();
        let mut selected_edge_count = 0_usize;
        let mut separated_edge_count = 0_usize;
        let mut replacements = Vec::new();
        for (source_index, edge_indices) in selections {
            selected_edge_count += edge_indices.len();
            let source = &sources[source_index];
            let (unwelded, separated) = source.mesh.unwelded_topology_edges_with_normal_update(
                &edge_indices,
                options.modify_normals,
            )?;
            separated_edge_count += separated;
            if &unwelded != source.mesh {
                replacements.push((source.id, Geometry::Mesh(unwelded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoSelectedMeshEdgesToUnweld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Processed {selected_edge_count} selected mesh edge(s) in {changed_mesh_count} changed mesh(es): {separated_edge_count} new seam(s); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn selected_unweld_edges(
    sources: &[MeshTopologySource<'_>],
    selection: &UnweldEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        UnweldEdgeSelection::Edges(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.mesh.topology().edge_count();
                let edge_indices = match selection {
                    SurfaceFaceIndices::All => (0..edge_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&edge) = indices.iter().find(|&&edge| edge >= edge_count) {
                            return Err(CommandError::UnweldEdgeIndexOutOfRange {
                                edge,
                                edge_count,
                            });
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, edge_indices))
            })
            .collect(),
        UnweldEdgeSelection::Point(target) => {
            let mut best = None;
            for (source_index, source) in sources.iter().enumerate() {
                for (edge_index, edge) in source
                    .mesh
                    .wireframe_lines(tolerance)?
                    .into_iter()
                    .enumerate()
                {
                    let closest = edge.closest_point(*target, tolerance)?;
                    let distance = closest.distance_to(*target)?;
                    if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                        best = Some((distance, source_index, edge_index));
                    }
                }
            }
            let Some((_, source, edge)) = best else {
                return Err(CommandError::NoSelectedMeshEdgesToUnweld);
            };
            Ok(vec![(source, vec![edge])])
        }
    }
}

fn parse_unweld_edge_arguments(arguments: &[&str]) -> Result<UnweldEdgeOptions, CommandError> {
    let mut edge_selection = None;
    let mut modify_normals = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Edges")
            || option_name_eq(argument, "EdgeIndices")
            || option_name_eq(argument, "ModifyNormals")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(UNWELD_EDGE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Edges") || option_name_eq(name, "EdgeIndices"))
                && edge_selection.is_none()
            {
                edge_selection = Some(parse_surface_face_indices(value, UNWELD_EDGE_USAGE)?);
            } else if option_name_eq(name, "ModifyNormals") && modify_normals.is_none() {
                modify_normals =
                    Some(parse_yes_no(value).ok_or(CommandError::Usage(UNWELD_EDGE_USAGE))?);
            } else {
                return Err(CommandError::Usage(UNWELD_EDGE_USAGE));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    let selection = if let Some(edges) = edge_selection {
        require_consumed(&positional, 0, UNWELD_EDGE_USAGE)?;
        UnweldEdgeSelection::Edges(edges)
    } else {
        if positional.is_empty() {
            return Err(CommandError::Usage(UNWELD_EDGE_USAGE));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, UNWELD_EDGE_USAGE)?;
        UnweldEdgeSelection::Point(point)
    };
    Ok(UnweldEdgeOptions {
        selection,
        modify_normals: modify_normals.unwrap_or(true),
    })
}

const UNWELD_VERTEX_USAGE: &str =
    "UnweldVertex (point|Vertices=All|Vertices=0,2,...) [ModifyNormals=Yes|No]";

#[derive(Clone, Debug, PartialEq)]
enum UnweldVertexSelection {
    Point(Point3),
    Vertices(SurfaceFaceIndices),
}

#[derive(Clone, Debug, PartialEq)]
struct UnweldVertexOptions {
    selection: UnweldVertexSelection,
    modify_normals: bool,
}

pub(super) struct UnweldVertexCommand;

impl Command for UnweldVertexCommand {
    fn name(&self) -> &'static str {
        "UnweldVertex"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["UnweldVertices", "UnweldMeshVertex"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_unweld_vertex_arguments(arguments)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedUnweldVertexGeometry
        })?;

        let selections = selected_unweld_vertices(&sources, &options.selection)?;
        let mesh_count = sources.len();
        let mut selected_vertex_count = 0_usize;
        let mut separated_vertex_count = 0_usize;
        let mut replacements = Vec::new();
        for (source_index, vertex_indices) in selections {
            selected_vertex_count += vertex_indices.len();
            let source = &sources[source_index];
            let (unwelded, separated) = source.mesh.unwelded_topology_vertices_with_normal_update(
                &vertex_indices,
                options.modify_normals,
            )?;
            separated_vertex_count += separated;
            if &unwelded != source.mesh {
                replacements.push((source.id, Geometry::Mesh(unwelded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoSelectedMeshVerticesToUnweld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Processed {selected_vertex_count} selected mesh topology vertex index(es) in {changed_mesh_count} changed mesh(es): {separated_vertex_count} previously shared vertex set(s) separated; {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn selected_unweld_vertices(
    sources: &[MeshTopologySource<'_>],
    selection: &UnweldVertexSelection,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        UnweldVertexSelection::Vertices(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let vertex_count = source.mesh.topology().topological_vertex_count();
                let vertex_indices = match selection {
                    SurfaceFaceIndices::All => (0..vertex_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&vertex) =
                            indices.iter().find(|&&vertex| vertex >= vertex_count)
                        {
                            return Err(CommandError::UnweldVertexIndexOutOfRange {
                                vertex,
                                vertex_count,
                            });
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, vertex_indices))
            })
            .collect(),
        UnweldVertexSelection::Point(target) => {
            let mut best = None;
            for (source_index, source) in sources.iter().enumerate() {
                for (vertex_index, vertex) in
                    source.mesh.topology_vertex_points().into_iter().enumerate()
                {
                    let distance = vertex.distance_to(*target)?;
                    if best.is_none_or(|(best_distance, _, _)| distance < best_distance) {
                        best = Some((distance, source_index, vertex_index));
                    }
                }
            }
            let Some((_, source, vertex)) = best else {
                return Err(CommandError::NoSelectedMeshVerticesToUnweld);
            };
            Ok(vec![(source, vec![vertex])])
        }
    }
}

fn parse_unweld_vertex_arguments(arguments: &[&str]) -> Result<UnweldVertexOptions, CommandError> {
    let mut vertex_selection = None;
    let mut modify_normals = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Vertices")
            || option_name_eq(argument, "VertexIndices")
            || option_name_eq(argument, "ModifyNormals")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(UNWELD_VERTEX_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Vertices") || option_name_eq(name, "VertexIndices"))
                && vertex_selection.is_none()
            {
                vertex_selection = Some(parse_surface_face_indices(value, UNWELD_VERTEX_USAGE)?);
            } else if option_name_eq(name, "ModifyNormals") && modify_normals.is_none() {
                modify_normals =
                    Some(parse_yes_no(value).ok_or(CommandError::Usage(UNWELD_VERTEX_USAGE))?);
            } else {
                return Err(CommandError::Usage(UNWELD_VERTEX_USAGE));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    let selection = if let Some(vertices) = vertex_selection {
        require_consumed(&positional, 0, UNWELD_VERTEX_USAGE)?;
        UnweldVertexSelection::Vertices(vertices)
    } else {
        if positional.is_empty() {
            return Err(CommandError::Usage(UNWELD_VERTEX_USAGE));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, UNWELD_VERTEX_USAGE)?;
        UnweldVertexSelection::Point(point)
    };
    Ok(UnweldVertexOptions {
        selection,
        modify_normals: modify_normals.unwrap_or(true),
    })
}
