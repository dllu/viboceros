use super::*;

pub(super) fn targets(
    document: &Document,
    arguments: &[&str],
) -> Result<Vec<CreaseTarget>, CommandError> {
    let mut object = None;
    let mut edges = None;
    let mut vertices = None;
    let mut i = 0;
    while i < arguments.len() {
        let (name, value, n) = if let Some((name, value)) = arguments[i].split_once('=') {
            (name, value, 1)
        } else {
            (
                arguments[i],
                *arguments.get(i + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        if option_name_eq(name, "Object") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            );
        } else if option_name_eq(name, "Edges") && edges.is_none() {
            edges = Some(parse_surface_face_indices(value, USAGE)?);
        } else if option_name_eq(name, "Vertices") && vertices.is_none() {
            vertices = Some(parse_surface_face_indices(value, USAGE)?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += n;
    }
    if object.is_some_and(|id| !document.is_selected(id)) {
        return Err(CommandError::Usage(USAGE));
    }
    let mut targets = Vec::new();
    if edges.is_none() && vertices.is_none() && object.is_none() {
        let subds = document
            .objects()
            .filter(|o| matches!(o.geometry(), Geometry::SubD(_)))
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        for (grip, _) in document.selected_control_points() {
            if subds.contains(&grip.object) {
                targets.push(CreaseTarget::Vertex(grip.object, grip.index));
            }
        }
        return Ok(targets);
    }
    if edges.is_none() && vertices.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    for source in document
        .selected_objects()
        .filter(|o| object.is_none_or(|id| o.id() == id))
    {
        if !matches!(source.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)) {
            continue;
        }
        for (selection, vertex) in [(&edges, false), (&vertices, true)] {
            let Some(selection) = selection else { continue };
            let indices = match selection {
                SurfaceFaceIndices::Indices(values) => values.clone(),
                SurfaceFaceIndices::All => {
                    let count = match (source.geometry(), vertex) {
                        (Geometry::SubD(s), false) => s.edges().len(),
                        (Geometry::SubD(s), true) => s.control_points().len(),
                        (Geometry::Mesh(m), false) => m.topology_edge_points().len(),
                        _ => return Err(CommandError::CreaseUnavailable),
                    };
                    if count > MAX_TARGETS.saturating_sub(targets.len()) {
                        return Err(CommandError::CreaseResourceLimit);
                    }
                    (0..count).collect()
                }
            };
            if indices.len() > MAX_TARGETS.saturating_sub(targets.len()) {
                return Err(CommandError::CreaseResourceLimit);
            }
            targets.extend(indices.into_iter().map(|i| {
                if vertex {
                    CreaseTarget::Vertex(source.id(), i)
                } else {
                    CreaseTarget::Edge(source.id(), i)
                }
            }));
        }
    }
    Ok(targets)
}
