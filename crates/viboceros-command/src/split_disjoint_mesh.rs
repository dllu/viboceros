use super::*;

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
        let mut inputs = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedSplitDisjointMeshGeometry);
                };
                Ok(SplitMeshInput {
                    id: object.id(),
                    attributes: object.attributes().clone(),
                    group_ids: object.group_ids().to_vec(),
                    pieces: mesh.disjoint_pieces(),
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let split_mesh_count = inputs.iter().filter(|input| input.pieces.len() > 1).count();
        if split_mesh_count == 0 {
            return Err(CommandError::NoDisjointMeshes);
        }
        let unchanged_mesh_count = inputs.len() - split_mesh_count;
        let piece_count = inputs
            .iter()
            .filter(|input| input.pieces.len() > 1)
            .map(|input| input.pieces.len())
            .sum::<usize>();

        let mut replacements = Vec::with_capacity(split_mesh_count);
        for input in &mut inputs {
            if input.pieces.len() > 1 {
                // Move the first piece into the original object. The remaining
                // pieces retain their order and become new objects below.
                replacements.push((input.id, Geometry::Mesh(input.pieces.remove(0))));
            } else {
                input.pieces.clear();
            }
        }
        let replaced = document.replace_object_geometries(replacements)?;
        debug_assert_eq!(replaced, split_mesh_count);

        let mut output_ids = Vec::with_capacity(inputs.len() + piece_count - split_mesh_count);

        for input in inputs {
            output_ids.push(input.id);
            for piece in input.pieces {
                let id = document.add_geometry_with_attributes(
                    Geometry::Mesh(piece),
                    input.attributes.clone(),
                )?;
                output_ids.push(id);
                document.set_object_group_memberships(id, input.group_ids.iter().copied())?;
            }
        }

        // Locked/hidden peers can be edited through a selected group. They
        // cannot seed a new pick; selectable outputs expand their groups to
        // include those peers and their new pieces instead.
        let selectable = document
            .selectable_objects()
            .map(|object| object.id())
            .collect::<BTreeSet<_>>();
        replace_selection(
            document,
            output_ids.into_iter().filter(|id| selectable.contains(id)),
        )?;
        Ok(format!(
            "Split {split_mesh_count} mesh(es) into {piece_count} piece(s); {unchanged_mesh_count} mesh(es) unchanged"
        ))
    }
}

struct SplitMeshInput {
    id: ObjectId,
    attributes: ObjectAttributes,
    group_ids: Vec<GroupId>,
    pieces: Vec<TriangleMesh>,
}
