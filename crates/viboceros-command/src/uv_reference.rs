//! Read-only UV-command references require a face for multi-face B-reps.
use super::*;
use viboceros_geometry::BrepFace;
#[cfg(test)]
mod tests;

pub(super) struct UvReference<'a> {
    pub(super) object: ObjectId,
    pub(super) surface: &'a NurbsSurface,
    pub(super) face: Option<&'a BrepFace>,
}

pub(super) fn resolve<'a>(
    document: &'a Document,
    arguments: &[&str],
    usage: &'static str,
) -> Result<UvReference<'a>, CommandError> {
    let mut object = None;
    let mut face = None;
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(usage))?;
        if option_name_eq(name, "Surface") && object.is_none() {
            object = Some(
                value
                    .parse::<ObjectId>()
                    .map_err(|_| CommandError::Usage(usage))?,
            );
        } else if option_name_eq(name, "Face") && face.is_none() {
            face = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(usage))?,
            );
        } else {
            return Err(CommandError::Usage(usage));
        }
    }
    let object = object.ok_or(CommandError::Usage(usage))?;
    if !document.is_object_selectable(object) {
        return Err(CommandError::Usage(usage));
    }
    let geometry = document
        .object(object)
        .ok_or(DocumentError::ObjectNotFound(object))?
        .geometry();
    let (surface, face) = match geometry {
        Geometry::NurbsSurface(surface) if face.is_none_or(|index| index == 0) => (surface, None),
        Geometry::Brep(brep) => {
            let index = face
                .or_else(|| (brep.faces().len() == 1).then_some(0))
                .ok_or(CommandError::Usage(usage))?;
            let face = brep.faces().get(index).ok_or(CommandError::Usage(usage))?;
            (face.surface(), Some(face))
        }
        _ => return Err(CommandError::Usage(usage)),
    };
    Ok(UvReference {
        object,
        surface,
        face,
    })
}
