//! Selected mesh welding, component selectors and atomic document edits.
use super::*;

const WELD_EDGE_USAGE: &str = "WeldEdge (point|Edges=All|Edges=0,2,...)";

#[derive(Clone, Debug, PartialEq)]
enum WeldEdgeSelection {
    Point(Point3),
    Edges(SurfaceFaceIndices),
}

pub(super) struct WeldEdgeCommand;

impl Command for WeldEdgeCommand {
    fn name(&self) -> &'static str {
        "WeldEdge"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["WeldMeshEdge"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_weld_edge_arguments(arguments)?;
        let sources =
            selected_mesh_topology_sources(document, || CommandError::UnsupportedWeldEdgeGeometry)?;

        let selections = selected_weld_edges(&sources, &selection, document.tolerance())?;
        let mesh_count = sources.len();
        let mut selected_edge_count = 0_usize;
        let mut welded_edge_count = 0_usize;
        let mut removed_vertex_count = 0_usize;
        let mut replacements = Vec::new();
        for (source_index, edge_indices) in selections {
            selected_edge_count += edge_indices.len();
            let source = &sources[source_index];
            let (welded, welded_edges) = source
                .mesh
                .welded_topology_edges_with_normal_generation(&edge_indices, true)?;
            welded_edge_count += welded_edges;
            removed_vertex_count += source.mesh.vertices().len() - welded.vertices().len();
            // Generating display normals alone is not a topology edit and
            // must not add a history entry for an already welded mesh.
            if welded.vertices() != source.mesh.vertices() || welded.faces() != source.mesh.faces()
            {
                replacements.push((source.id, Geometry::Mesh(welded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoSelectedMeshEdgesToWeld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Processed {selected_edge_count} selected mesh edge(s) in {changed_mesh_count} changed mesh(es): welded {welded_edge_count} divided edge(s) and removed {removed_vertex_count} raw vertex occurrence(s); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn selected_weld_edges(
    sources: &[MeshTopologySource<'_>],
    selection: &WeldEdgeSelection,
    tolerance: Tolerance,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        WeldEdgeSelection::Edges(selection) => sources
            .iter()
            .enumerate()
            .map(|(source_index, source)| {
                let edge_count = source.mesh.topology().edge_count();
                let edge_indices = match selection {
                    SurfaceFaceIndices::All => (0..edge_count).collect(),
                    SurfaceFaceIndices::Indices(indices) => {
                        if let Some(&edge) = indices.iter().find(|&&edge| edge >= edge_count) {
                            return Err(CommandError::WeldEdgeIndexOutOfRange { edge, edge_count });
                        }
                        indices.clone()
                    }
                };
                Ok((source_index, edge_indices))
            })
            .collect(),
        WeldEdgeSelection::Point(target) => {
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
                return Err(CommandError::NoSelectedMeshEdgesToWeld);
            };
            Ok(vec![(source, vec![edge])])
        }
    }
}

fn parse_weld_edge_arguments(arguments: &[&str]) -> Result<WeldEdgeSelection, CommandError> {
    let mut edge_selection = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Edges") || option_name_eq(argument, "EdgeIndices") {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(WELD_EDGE_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Edges") || option_name_eq(name, "EdgeIndices"))
                && edge_selection.is_none()
            {
                edge_selection = Some(parse_surface_face_indices(value, WELD_EDGE_USAGE)?);
            } else {
                return Err(CommandError::Usage(WELD_EDGE_USAGE));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    if let Some(edges) = edge_selection {
        require_consumed(&positional, 0, WELD_EDGE_USAGE)?;
        Ok(WeldEdgeSelection::Edges(edges))
    } else {
        if positional.is_empty() {
            return Err(CommandError::Usage(WELD_EDGE_USAGE));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, WELD_EDGE_USAGE)?;
        Ok(WeldEdgeSelection::Point(point))
    }
}

const WELD_VERTICES_USAGE: &str = "WeldVertices (point|Vertices=All|Vertices=0,2,...)";

#[derive(Clone, Debug, PartialEq)]
enum WeldVerticesSelection {
    Point(Point3),
    Vertices(SurfaceFaceIndices),
}

pub(super) struct WeldVerticesCommand;

impl Command for WeldVerticesCommand {
    fn name(&self) -> &'static str {
        "WeldVertices"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["WeldVertex", "WeldMeshVertex"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let selection = parse_weld_vertices_arguments(arguments)?;
        let sources = selected_mesh_topology_sources(document, || {
            CommandError::UnsupportedWeldVerticesGeometry
        })?;

        let selections = selected_weld_vertices(&sources, &selection)?;
        let mesh_count = sources.len();
        let mut selected_vertex_count = 0_usize;
        let mut welded_edge_count = 0_usize;
        let mut removed_vertex_count = 0_usize;
        let mut replacements = Vec::new();
        for (source_index, vertex_indices) in selections {
            selected_vertex_count += vertex_indices.len();
            let source = &sources[source_index];
            let (welded, welded_edges) = source
                .mesh
                .welded_topology_vertices_with_normal_generation(&vertex_indices, true)?;
            welded_edge_count += welded_edges;
            removed_vertex_count += source.mesh.vertices().len() - welded.vertices().len();
            if welded.vertices() != source.mesh.vertices() || welded.faces() != source.mesh.faces()
            {
                replacements.push((source.id, Geometry::Mesh(welded)));
            }
        }
        if replacements.is_empty() {
            return Err(CommandError::NoSelectedMeshVerticesToWeld);
        }
        let changed_mesh_count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Processed {selected_vertex_count} selected mesh topology vertex index(es) in {changed_mesh_count} changed mesh(es): welded {welded_edge_count} incident seam edge(s) and removed {removed_vertex_count} raw vertex occurrence(s); {} mesh(es) unchanged",
            mesh_count - changed_mesh_count
        ))
    }
}

fn selected_weld_vertices(
    sources: &[MeshTopologySource<'_>],
    selection: &WeldVerticesSelection,
) -> Result<Vec<(usize, Vec<usize>)>, CommandError> {
    match selection {
        WeldVerticesSelection::Vertices(selection) => sources
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
                            return Err(CommandError::WeldVertexIndexOutOfRange {
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
        WeldVerticesSelection::Point(target) => {
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
                return Err(CommandError::NoSelectedMeshVerticesToWeld);
            };
            Ok(vec![(source, vec![vertex])])
        }
    }
}

fn parse_weld_vertices_arguments(
    arguments: &[&str],
) -> Result<WeldVerticesSelection, CommandError> {
    let mut vertex_selection = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let option = if let Some((name, value)) = argument.split_once('=') {
            Some((name, value, 1))
        } else if option_name_eq(argument, "Vertices") || option_name_eq(argument, "VertexIndices")
        {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(WELD_VERTICES_USAGE))?;
            Some((argument, *value, 2))
        } else {
            None
        };
        if let Some((name, value, consumed)) = option {
            if (option_name_eq(name, "Vertices") || option_name_eq(name, "VertexIndices"))
                && vertex_selection.is_none()
            {
                vertex_selection = Some(parse_surface_face_indices(value, WELD_VERTICES_USAGE)?);
            } else {
                return Err(CommandError::Usage(WELD_VERTICES_USAGE));
            }
            index += consumed;
        } else {
            positional.push(argument);
            index += 1;
        }
    }
    if let Some(vertices) = vertex_selection {
        require_consumed(&positional, 0, WELD_VERTICES_USAGE)?;
        Ok(WeldVerticesSelection::Vertices(vertices))
    } else {
        if positional.is_empty() {
            return Err(CommandError::Usage(WELD_VERTICES_USAGE));
        }
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, WELD_VERTICES_USAGE)?;
        Ok(WeldVerticesSelection::Point(point))
    }
}
