use super::*;

#[cfg(test)]
mod tests;

pub(super) struct SplitDisjointMeshCommand;

impl Command for SplitDisjointMeshCommand {
    fn name(&self) -> &'static str {
        "SplitDisjointMesh"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "SplitDisjointMesh")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let (inputs, piece_count) = stage_disjoint_meshes(document, MAX_SPAN_OUTPUT_OBJECTS)?;
        let split_mesh_count = inputs.iter().filter(|input| input.pieces.len() > 1).count();
        if split_mesh_count == 0 {
            return Err(CommandError::NoDisjointMeshes);
        }
        let unchanged_mesh_count = inputs.len() - split_mesh_count;

        let mut deleted_sources = Vec::with_capacity(split_mesh_count);
        let mut output_ids = Vec::with_capacity(inputs.len() + piece_count);
        let mut pieces = Vec::with_capacity(piece_count);

        for input in inputs {
            if input.pieces.len() <= 1 {
                output_ids.push(input.id);
                continue;
            }
            // Rhino creates fresh IDs for every piece. Its command retains
            // object-hidden/locked and layer-locked sources. A hidden layer
            // alone does not prevent source deletion in the live command.
            if input.delete_source {
                deleted_sources.push(input.id);
            } else {
                output_ids.push(input.id);
            }
            for piece in input.pieces {
                pieces.push((input.id, Geometry::Mesh(piece)));
            }
        }

        // Source-derived copies can inherit a locked layer from an editable
        // group-selected peer. Validate all sources once before insertion.
        output_ids.extend(document.copy_object_pieces_into_source_groups(pieces)?);

        // Attach all output memberships before deleting sources, so source
        // groups never become temporarily empty during the command.
        document.delete_objects(deleted_sources)?;

        document.select_command_results(output_ids)?;
        Ok(format!(
            "Split {split_mesh_count} mesh(es) into {piece_count} piece(s); {unchanged_mesh_count} mesh(es) unchanged"
        ))
    }
}

fn stage_disjoint_meshes(
    document: &Document,
    maximum: usize,
) -> Result<(Vec<SplitMeshInput>, usize), CommandError> {
    let locked_layers = document
        .layers()
        .filter(|layer| layer.is_locked())
        .map(|layer| layer.id())
        .collect::<BTreeSet<_>>();
    let mut piece_count = 0usize;
    let inputs = document
        .selected_objects()
        .map(|object| {
            let Geometry::Mesh(mesh) = object.geometry() else {
                return Err(CommandError::UnsupportedSplitDisjointMeshGeometry);
            };
            let pieces = mesh
                .try_disjoint_pieces((maximum - piece_count).max(1))
                .map_err(|error| match error {
                    GeometryError::MeshComponentLimit { .. } => {
                        too_many_span_outputs("SplitDisjointMesh")
                    }
                    other => CommandError::from(other),
                })?;
            if pieces.len() > 1 {
                piece_count = piece_count
                    .checked_add(pieces.len())
                    .filter(|count| *count <= maximum)
                    .ok_or_else(|| too_many_span_outputs("SplitDisjointMesh"))?;
            }
            Ok(SplitMeshInput {
                delete_source: object.attributes().is_visible()
                    && !object.attributes().is_locked()
                    && !locked_layers.contains(&object.attributes().layer_id()),
                id: object.id(),
                pieces,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok((inputs, piece_count))
}

struct SplitMeshInput {
    delete_source: bool,
    id: ObjectId,
    pieces: Vec<TriangleMesh>,
}
