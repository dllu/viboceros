use super::*;
impl MeshSplitCommand {
    pub(super) fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        post: bool,
    ) -> Result<String, CommandError> {
        let parsed = self.parse(args)?;
        self.remember(&parsed);
        let first = parsed.first.clone().unwrap_or_else(|| {
            if post {
                doc.selected_objects()
                    .filter(|o| ObjectSelectionFilter::EdgeAnalysis.accepts_object(o))
                    .map(|o| o.id())
                    .collect()
            } else {
                doc.objects()
                    .filter(|o| {
                        doc.is_selected(o.id())
                            && ObjectSelectionFilter::EdgeAnalysis.accepts_object(o)
                    })
                    .map(|o| o.id())
                    .collect()
            }
        });
        if first.is_empty()
            || parsed.second.is_empty() && parsed.planes.is_empty()
            || first.len() + parsed.second.len() > 128
        {
            return Err(CommandError::Usage(USAGE));
        }
        let first_set = first.iter().copied().collect::<BTreeSet<_>>();
        if first_set.len() != first.len()
            || parsed.second.iter().collect::<BTreeSet<_>>().len() != parsed.second.len()
            || !parsed.permit && parsed.second.iter().any(|id| first_set.contains(id))
        {
            return Err(CommandError::Usage(USAGE));
        }
        for &id in first.iter().chain(&parsed.second) {
            if !doc.is_object_selectable(id) {
                return Err(CommandError::Usage("selectable MeshSplit objects"));
            }
        }
        let staged = compute(doc, &first, &parsed, context)?;
        let changed = staged
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<BTreeSet<_>>();
        let selected = doc
            .selected_object_ids()
            .chain(first.iter().copied())
            .chain(parsed.second.iter().copied())
            .filter(|id| post || !changed.contains(id) || !doc.is_selected(*id))
            .collect::<BTreeSet<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.select_objects_direct(
            first.iter().chain(&parsed.second).copied(),
            SelectionMode::Add,
        )?;
        let count = staged.len();
        let ids = doc.copy_object_pieces_with_metadata_into_source_groups(
            staged
                .into_iter()
                .map(|(owner, mesh)| (owner, Geometry::Mesh(mesh), BTreeMap::new())),
        )?;
        doc.delete_objects(changed)?;
        doc.select_objects_direct(ids, SelectionMode::Remove)?;
        Ok(format!("MeshSplit produced {count} mesh(es)"))
    }
}
#[cfg(not(feature = "native-smlib"))]
fn compute(
    _: &Document,
    _: &[ObjectId],
    _: &Arguments,
    _: CommandContext,
) -> Result<Vec<(ObjectId, TriangleMesh)>, CommandError> {
    Err(CommandError::MeshSplitRequiresNativeKernel)
}
#[cfg(feature = "native-smlib")]
fn compute(
    doc: &Document,
    first: &[ObjectId],
    parsed: &Arguments,
    context: CommandContext,
) -> Result<Vec<(ObjectId, TriangleMesh)>, CommandError> {
    super::geometry::compute(doc, first, parsed, context)
}
