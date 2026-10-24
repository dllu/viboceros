use super::*;
use std::borrow::Cow;

impl MeshBooleanCommand {
    pub(super) fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let arguments = Arguments::parse(self.kind, self.options.get(), args)?;
        self.options.set(arguments.options);
        let (first, second) = arguments.sets(self.kind, doc, post)?;
        let ids = first.iter().chain(&second).copied().collect::<Vec<_>>();
        if ids.len() > 128 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            return Err(CommandError::Usage(
                "at most 128 distinct mesh Boolean sources",
            ));
        }
        let selectable = doc
            .selectable_objects()
            .chain(doc.selected_objects())
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        let mut meshes = Vec::with_capacity(ids.len());
        for &id in &ids {
            let obj = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
            if !selectable.contains(&id) {
                return Err(CommandError::Usage("selectable mesh Boolean sources"));
            }
            meshes.push(match obj.geometry() {
                Geometry::Mesh(mesh) => Cow::Borrowed(mesh),
                Geometry::NurbsSurface(s) => Cow::Owned(
                    s.tessellate(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())?,
                ),
                Geometry::Brep(b) => Cow::Owned(
                    b.display_mesh(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())?,
                ),
                _ => return Err(CommandError::MeshBooleanRequiresObjects),
            });
        }
        let refs = meshes.iter().map(Cow::as_ref).collect::<Vec<_>>();
        let output = stage(
            self.kind,
            &refs,
            first.len(),
            arguments.options.geometry_tolerance(doc.tolerance()),
        )?;
        drop(meshes);
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        let preserve_picks = self.kind == Kind::Union && !post && !output.is_empty();
        let released = selected
            .iter()
            .copied()
            .filter(|id| !preserve_picks || !ids.contains(id))
            .chain(
                (!preserve_picks)
                    .then_some(ids.iter().copied())
                    .into_iter()
                    .flatten(),
            )
            .collect::<BTreeSet<_>>();
        doc.release_command_selection_on_history_replay(released)?;
        if !preserve_picks && self.kind != Kind::Union {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Remove)?;
        }
        if output.is_empty() {
            return Err(CommandError::MeshBooleanNoResult);
        }
        if post {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Remove)?;
        }
        let owner = first[0];
        let mut output = output.into_iter();
        let initial = output.next().unwrap();
        let mut added = 0;
        // The owner supplies retained attributes and memberships for every result.
        let extras = output
            .map(|mesh| (owner, Geometry::Mesh(mesh), BTreeMap::new()))
            .collect::<Vec<_>>();
        doc.replace_object_geometries_with_history(
            [(owner, Geometry::Mesh(initial))],
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        let removed = first
            .iter()
            .skip(1)
            .chain(second.iter().filter(|_| {
                self.kind != Kind::Split
                    && (self.kind != Kind::Difference || arguments.options.delete_cutters)
            }))
            .copied()
            .collect::<Vec<_>>();
        doc.delete_objects(removed)?;
        doc.move_objects_to_end_in_order([owner])?;
        if !extras.is_empty() {
            added = doc
                .copy_object_pieces_with_metadata_into_source_groups(extras)?
                .len();
        }
        if preserve_picks {
            doc.release_transform_selection_on_history_replay([owner])?;
        }
        doc.select_objects_direct([owner], SelectionMode::Remove)?;
        Ok(format!("{} produced {} mesh(es)", self.name(), added + 1))
    }
}

#[cfg(not(feature = "native-smlib"))]
fn stage(
    _: Kind,
    _: &[&TriangleMesh],
    _: usize,
    _: Tolerance,
) -> Result<Vec<TriangleMesh>, CommandError> {
    Err(CommandError::MeshBooleanRequiresNativeKernel)
}
#[cfg(feature = "native-smlib")]
fn stage(
    kind: Kind,
    meshes: &[&TriangleMesh],
    first: usize,
    tolerance: Tolerance,
) -> Result<Vec<TriangleMesh>, CommandError> {
    super::geometry::stage(kind, meshes, first, tolerance)
}
