//! Extract filtered mesh edges as independent lines or joined polylines.
use crate::*;

const EXTRACT_MESH_EDGES_USAGE: &str = "ExtractMeshEdges [ExtractBy=Unwelded|BreakAngle|Naked] [GreaterThanAngle=degrees] [LessThanAngle=degrees] [JoinResults=Yes|No]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractMeshEdgeKind {
    Unwelded,
    BreakAngle,
    Naked,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ExtractMeshEdgesOptions {
    kind: ExtractMeshEdgeKind,
    greater_than_degrees: Real,
    less_than_degrees: Real,
    join_results: bool,
}

impl ExtractMeshEdgesOptions {
    fn filter(self) -> MeshEdgeFilter {
        match self.kind {
            ExtractMeshEdgeKind::Unwelded => MeshEdgeFilter::Unwelded,
            ExtractMeshEdgeKind::Naked => MeshEdgeFilter::Naked,
            ExtractMeshEdgeKind::BreakAngle => MeshEdgeFilter::FaceAngle {
                greater_than_radians: self.greater_than_degrees.to_radians(),
                less_than_radians: self.less_than_degrees.to_radians(),
            },
        }
    }
}

pub(crate) struct ExtractMeshEdgesCommand;

impl Command for ExtractMeshEdgesCommand {
    fn name(&self) -> &'static str {
        "ExtractMeshEdges"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_extract_mesh_edges_arguments(arguments)?;
        let mut staged = Vec::new();
        let mut source_count = 0_usize;
        for object in document.selected_objects() {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::UnsupportedExtractMeshEdgesGeometry);
            };
            source_count += 1;
            if options.join_results {
                staged.extend(
                    mesh.filtered_edge_polylines(options.filter(), document.tolerance())?
                        .into_iter()
                        .map(Geometry::Polyline),
                );
            } else {
                staged.extend(
                    mesh.filtered_edge_lines(options.filter(), document.tolerance())?
                        .into_iter()
                        .map(Geometry::Line),
                );
            }
            if staged.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("ExtractMeshEdges"));
            }
        }
        if source_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if staged.is_empty() {
            return Err(CommandError::NoExtractableMeshEdges);
        }

        let output_count = staged.len();
        let layer = document.current_layer_id();
        let mut output_ids = Vec::with_capacity(output_count);
        for geometry in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        document.select_objects_direct(output_ids, SelectionMode::Add)?;
        Ok(format!(
            "Extracted {output_count} mesh edge curve(s) from {source_count} mesh(es)"
        ))
    }
}

fn parse_extract_mesh_edges_arguments(
    arguments: &[&str],
) -> Result<ExtractMeshEdgesOptions, CommandError> {
    let mut options = ExtractMeshEdgesOptions {
        kind: ExtractMeshEdgeKind::Unwelded,
        greater_than_degrees: 0.0,
        less_than_degrees: 180.0,
        join_results: false,
    };
    let mut extract_by_seen = false;
    let mut greater_seen = false;
    let mut less_seen = false;
    let mut join_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(name, "ExtractBy") && !extract_by_seen {
            let value = value.trim_start_matches('_');
            options.kind = if value.eq_ignore_ascii_case("Unwelded") {
                ExtractMeshEdgeKind::Unwelded
            } else if value.eq_ignore_ascii_case("BreakAngle") {
                ExtractMeshEdgeKind::BreakAngle
            } else if value.eq_ignore_ascii_case("Naked") {
                ExtractMeshEdgeKind::Naked
            } else {
                return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
            };
            extract_by_seen = true;
        } else if (option_name_eq(name, "GreaterThanAngle") || option_name_eq(name, "GreaterThan"))
            && !greater_seen
        {
            options.greater_than_degrees = parse_finite_real(value)?;
            greater_seen = true;
        } else if (option_name_eq(name, "LessThanAngle") || option_name_eq(name, "LessThan"))
            && !less_seen
        {
            options.less_than_degrees = parse_finite_real(value)?;
            less_seen = true;
        } else if option_name_eq(name, "JoinResults") && !join_seen {
            options.join_results =
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE))?;
            join_seen = true;
        } else {
            return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
        }
        index += consumed;
    }
    if (greater_seen || less_seen) && options.kind != ExtractMeshEdgeKind::BreakAngle {
        return Err(CommandError::Usage(EXTRACT_MESH_EDGES_USAGE));
    }
    if options.kind == ExtractMeshEdgeKind::BreakAngle
        && !(options.greater_than_degrees >= 0.0
            && options.greater_than_degrees < options.less_than_degrees
            && options.less_than_degrees <= 180.0)
    {
        return Err(GeometryError::InvalidMeshFaceAngleInterval.into());
    }
    Ok(options)
}
