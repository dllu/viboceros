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
        if (cutters.is_empty() && args.planes.is_empty())
            || cutters.len().saturating_add(args.planes.len()) > 128
            || cutters.iter().collect::<BTreeSet<_>>().len() != cutters.len()
        {
            return Err(CommandError::Usage(USAGE));
        }
        for &id in cutters.iter().chain(std::iter::once(&target)) {
            if !doc.is_object_selectable(id) {
                return Err(CommandError::Usage("selectable MeshTrim objects"));
            }
        }
        let output = compute(doc, target, &cutters, &args.planes, point, context)?;
        let selected = doc
            .selected_object_ids()
            .chain(cutters.iter().copied())
            .chain([target])
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
        doc.select_objects_direct(ids, SelectionMode::Remove)?;
        Ok(if removed {
            "MeshTrim removed the mesh".into()
        } else {
            "MeshTrim removed the picked region".into()
        })
    }
}
#[cfg(feature = "native-smlib")]
fn compute(
    doc: &Document,
    target: ObjectId,
    cutters: &[ObjectId],
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
    _: &[ObjectId],
    _: &[String],
    _: Point3,
    _: CommandContext,
) -> Result<Option<TriangleMesh>, CommandError> {
    Err(CommandError::MeshTrimRequiresNativeKernel)
}
