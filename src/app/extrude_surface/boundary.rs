//! A picked cutter retains its face and original object revision.
use super::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::Brep;

pub(in crate::app) enum Boundary {
    None,
    Selecting,
    Picked {
        reference: BoundaryRef,
        source: GeometrySnapshot,
        face: Box<Brep>,
    },
}

impl Boundary {
    pub(in crate::app) fn capture(
        document: &Document,
        reference: BoundaryRef,
    ) -> Result<Self, viboceros_command::CommandError> {
        let face = reference.capture(document)?;
        Ok(Self::Picked {
            reference,
            source: document
                .object(reference.object)
                .unwrap()
                .geometry_snapshot()
                .clone(),
            face: Box::new(face),
        })
    }

    pub(in crate::app) fn is_current(&self, document: &Document) -> bool {
        match self {
            Self::Picked {
                reference, source, ..
            } => {
                document.is_object_selectable(reference.object)
                    && document
                        .object(reference.object)
                        .is_some_and(|object| object.geometry_snapshot() == source)
            }
            _ => true,
        }
    }

    pub(in crate::app) fn face(&self) -> Option<&Brep> {
        match self {
            Self::Picked { face, .. } => Some(face),
            _ => None,
        }
    }
}
