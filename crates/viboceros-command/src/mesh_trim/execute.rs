use super::*;
impl MeshTrimCommand {
    pub(super) fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let args = Arguments::parse(args)?;
        let target = args.target.ok_or(CommandError::Usage(USAGE))?;
        let point = args.point.ok_or(CommandError::Usage(USAGE))?;
        let cutters = args.cutters.unwrap_or_else(|| {
            doc.selected_objects()
                .filter(|o| ObjectSelectionFilter::MeshSplitCutters.accepts_object(o))
                .map(|o| o.id())
                .collect()
        });
        MeshTrimCutters::capture(doc, &cutters)?
            .execute(doc, target, point, &args.planes, context)
            .map(|(message, _)| message)
    }
}
impl MeshTrimCutters {
    pub(super) fn execute(
        &self,
        doc: &mut Document,
        target: ObjectId,
        point: Point3,
        planes: &[String],
        context: CommandContext,
    ) -> Result<(String, Vec<ObjectId>), CommandError> {
        if (self.objects.is_empty() && planes.is_empty())
            || self.objects.len().saturating_add(planes.len()) > 128
        {
            return Err(CommandError::Usage(USAGE));
        }
        if !doc.is_object_selectable(target) {
            return Err(CommandError::Usage("a selectable MeshTrim target"));
        }
        let output = compute(doc, target, self, planes, point, context)?;
        let selected = doc
            .selected_object_ids()
            .chain(self.objects.iter().map(|(id, _)| *id))
            .chain([target])
            .filter(|id| doc.object(*id).is_some())
            .collect::<BTreeSet<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        let ids = doc.copy_object_pieces_with_metadata_into_source_groups(
            output
                .into_iter()
                .map(|m| (target, Geometry::Mesh(m), BTreeMap::new())),
        )?;
        doc.delete_objects([target])?;
        let removed = ids.is_empty();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Remove)?;
        let message = if removed {
            "MeshTrim removed the mesh".into()
        } else {
            "MeshTrim removed the picked region".into()
        };
        Ok((message, ids))
    }
}
#[cfg(feature = "native-smlib")]
fn compute(
    doc: &Document,
    target: ObjectId,
    cutters: &MeshTrimCutters,
    planes: &[String],
    point: Point3,
    context: CommandContext,
) -> Result<Option<TriangleMesh>, CommandError> {
    let (parts, selected) =
        super::geometry::partition(doc, target, cutters, planes, point, context)?;
    let Geometry::Mesh(source) = doc.object(target).unwrap().geometry() else {
        unreachable!()
    };
    let appearance = crate::mesh_partition::Appearance::new(source)?;
    let mut result = Vec::new();
    let mut work = 0;
    for (i, part) in parts.into_iter().enumerate() {
        if i == selected {
            continue;
        }
        let mesh = appearance.restore(part.mesh, part.source_faces, false)?;
        work += mesh.vertices().len() + mesh.faces().len();
        if work > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("mesh trim"));
        }
        result.push(mesh);
    }
    // Rhino retains all surviving regions in one object, including disconnected
    // regions. A pick on the only region (even without an intersection) deletes it.
    Ok(if result.is_empty() {
        None
    } else {
        Some(TriangleMesh::try_append(
            &result.iter().collect::<Vec<_>>(),
        )?)
    })
}
#[cfg(not(feature = "native-smlib"))]
fn compute(
    _: &Document,
    _: ObjectId,
    _: &MeshTrimCutters,
    _: &[String],
    _: Point3,
    _: CommandContext,
) -> Result<Option<TriangleMesh>, CommandError> {
    Err(CommandError::MeshTrimRequiresNativeKernel)
}
