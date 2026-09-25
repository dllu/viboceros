//! Shared document transaction for mesh face extraction by computed face set.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct FilterOutputOptions {
    pub make_copy: bool,
    pub border_only: bool,
}

struct Plan {
    source: ObjectId,
    remainder: Option<TriangleMesh>,
    extracted: TriangleMesh,
    borders: Vec<Geometry>,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}

pub(super) fn extract_filtered_mesh_faces(
    document: &mut Document,
    command: &'static str,
    output: FilterOutputOptions,
    unsupported: CommandError,
    no_matches: CommandError,
    no_borders: CommandError,
    mut matches: impl FnMut(&TriangleMesh, usize) -> Result<bool, GeometryError>,
) -> Result<String, CommandError> {
    extract_selected_mesh_faces(
        document,
        command,
        output,
        unsupported,
        no_matches,
        no_borders,
        |mesh| {
            let mut indices = Vec::new();
            for index in 0..mesh.face_count() {
                if matches(mesh, index)? {
                    indices.push(index);
                }
            }
            Ok(indices)
        },
    )
}

pub(super) fn extract_selected_mesh_faces(
    document: &mut Document,
    command: &'static str,
    output: FilterOutputOptions,
    unsupported: CommandError,
    no_matches: CommandError,
    no_borders: CommandError,
    mut select: impl FnMut(&TriangleMesh) -> Result<Vec<usize>, GeometryError>,
) -> Result<String, CommandError> {
    let tolerance = document.tolerance();
    let mut source_count = 0;
    let mut face_count = 0_usize;
    let mut output_count = 0_usize;
    let mut plans = Vec::new();
    for object in document.selected_objects() {
        source_count += 1;
        let Geometry::Mesh(mesh) = object.geometry() else {
            return Err(unsupported);
        };
        let indices = select(mesh)?;
        if indices.is_empty() {
            continue;
        }
        face_count = face_count
            .checked_add(indices.len())
            .ok_or_else(|| too_many_span_outputs(command))?;
        let (remainder, extracted) = mesh.extract_faces(&indices)?.into_parts();
        let borders = if output.border_only {
            extracted
                .boundary_polylines(tolerance)?
                .into_iter()
                .map(Geometry::Polyline)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        output_count = output_count
            .checked_add(if output.border_only { borders.len() } else { 1 })
            .filter(|&count| count <= MAX_SPAN_OUTPUT_OBJECTS)
            .ok_or_else(|| too_many_span_outputs(command))?;
        plans.push(Plan {
            source: object.id(),
            remainder,
            extracted,
            borders,
            attributes: object.attributes().clone(),
            groups: object.group_ids().to_vec(),
        });
    }
    if source_count == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    if plans.is_empty() {
        return Err(no_matches);
    }
    if output_count == 0 {
        return Err(no_borders);
    }
    let mesh_count = plans.len();
    if !output.border_only && !output.make_copy {
        document.clear_selection();
        document.replace_object_geometries(plans.iter().map(|plan| {
            (
                plan.source,
                Geometry::Mesh(plan.remainder.as_ref().unwrap_or(&plan.extracted).clone()),
            )
        }))?;
    }
    let mut results = Vec::with_capacity(output_count);
    for plan in plans {
        if output.border_only {
            for border in plan.borders {
                let id = document.add_geometry_with_attributes(border, plan.attributes.clone())?;
                document.set_object_group_memberships(id, plan.groups.clone())?;
                results.push(id);
            }
        } else if !output.make_copy && plan.remainder.is_none() {
            results.push(plan.source);
        } else {
            let id = document
                .add_geometry_with_attributes(Geometry::Mesh(plan.extracted), plan.attributes)?;
            document.set_object_group_memberships(id, plan.groups)?;
            results.push(id);
        }
    }
    document.select_command_results(results)?;
    Ok(format!(
        "Extracted {face_count} mesh face(s) from {mesh_count} mesh(es){}",
        if output.border_only {
            " as border curves"
        } else if output.make_copy {
            "; source faces copied"
        } else {
            "; source faces removed"
        }
    ))
}
