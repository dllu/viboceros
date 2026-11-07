//! Immutable boundary edge and supporting-face ownership shared by commands.
use crate::{CommandError, curve_reference::CapturedCurve, orient_surface::Target};
use viboceros_document::{Document, DocumentError, Geometry, ObjectId};
use viboceros_geometry::Brep;

/// The chosen edge and face must belong to the same captured parent topology.
#[derive(Clone, Debug)]
pub struct Edge {
    pub guide: CapturedCurve,
    pub target: Target,
    pub reversed_trim: bool,
}
impl Edge {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        index: usize,
        face: Option<usize>,
    ) -> Result<Self, CommandError> {
        let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
        let converted;
        let brep = match object.geometry() {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => {
                converted = Brep::try_surface_face(s.clone(), doc.tolerance())?;
                &converted
            }
            _ => return Err(CommandError::Usage("Select a surface edge")),
        };
        let adjacent = brep
            .faces()
            .iter()
            .enumerate()
            .flat_map(|(i, f)| {
                f.loops().iter().flat_map(move |l| {
                    l.trims()
                        .iter()
                        .filter(move |t| t.edge() == Some(index))
                        .map(move |t| (i, t.is_reversed_3d()))
                })
            })
            .collect::<Vec<_>>();
        let (face, reversed_trim) = if let Some(face) = face {
            adjacent
                .iter()
                .find(|(i, _)| *i == face)
                .copied()
                .ok_or(CommandError::Usage(
                    "Face must be adjacent to the selected edge",
                ))?
        } else {
            if adjacent
                .iter()
                .any(|(f, _)| Some(*f) != adjacent.first().map(|(f, _)| *f))
            {
                return Err(CommandError::Usage(
                    "Specify Face=index to choose the surface on a shared edge",
                ));
            }
            *adjacent
                .first()
                .ok_or(CommandError::Usage("Select a surface edge"))?
        };
        if adjacent.iter().filter(|(i, _)| *i == face).count() != 1 {
            return Err(CommandError::Usage(
                "Seam edges with multiple trims on one face are not supported yet",
            ));
        }
        Ok(Self {
            guide: CapturedCurve::capture(doc, id, Some(index))?,
            target: Target::capture(doc, id, Some(face))?,
            reversed_trim,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.guide.current(doc) && self.target.current(doc)
    }
}
