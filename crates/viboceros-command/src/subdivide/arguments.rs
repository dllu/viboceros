use super::*;
pub(super) fn parse(
    arguments: &[&str],
) -> Result<(Option<ObjectId>, Option<SurfaceFaceIndices>), CommandError> {
    let mut object = None;
    let mut faces = None;
    let mut i = 0;
    while i < arguments.len() {
        let (name, value, n) = if let Some((n, v)) = arguments[i].split_once('=') {
            (n, v, 1)
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
        } else if option_name_eq(name, "Faces") && faces.is_none() {
            faces = Some(parse_surface_face_indices(value, USAGE)?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += n;
    }
    if object.is_some() && faces.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((object, faces))
}
pub(super) fn targets(
    document: &Document,
    arguments: &[&str],
) -> Result<Vec<SubdivisionTarget>, CommandError> {
    let (object, faces) = parse(arguments)?;
    if object.is_some_and(|id| !document.is_selected(id)) {
        return Err(CommandError::Usage(USAGE));
    }
    let mut result = Vec::new();
    for source in document.selected_objects().filter(|o| {
        object.is_none_or(|id| o.id() == id) && ObjectSelectionFilter::MeshAndSubD.accepts_object(o)
    }) {
        let count = match source.geometry() {
            Geometry::Mesh(m) => m.face_count(),
            Geometry::SubD(s) => s.faces().len(),
            _ => unreachable!(),
        };
        match &faces {
            None => result.push(SubdivisionTarget::Whole(source.id())),
            Some(SurfaceFaceIndices::All) => {
                result.extend((0..count).map(|f| SubdivisionTarget::Face(source.id(), f)))
            }
            Some(SurfaceFaceIndices::Indices(faces)) => result.extend(
                faces
                    .iter()
                    .map(|&f| SubdivisionTarget::Face(source.id(), f)),
            ),
        }
    }
    Ok(result)
}
