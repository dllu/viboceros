//! Affine display instances share original geometry, samples, and tessellation.
use super::display_cache::DisplayGeometry;
use super::*;
use std::collections::BTreeMap;
use std::rc::Rc;
use viboceros_geometry::AffineTransform3;

pub(super) struct PreviewObject {
    pub geometry: Rc<DisplayGeometry>,
    pub transform: Option<AffineTransform3>,
}

/// Deformation previews retain selected sources and add object-colored wires.
#[derive(Clone, Copy)]
pub(super) enum ObjectPreview<'a> {
    Affine(TransformedObjects<'a>),
    Deformed(&'a BTreeMap<ObjectId, PreviewObject>),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TransformedObjects<'a> {
    pub sources: &'a [ObjectId],
    pub grips: &'a [viboceros_document::ControlPointId],
    pub copy: bool,
    pub reference_sources: bool,
    pub draw_source_faces: bool,
    pub reversing: bool,
    /// A Normal reference uses temporary selection styling without selecting it in the model.
    pub reference: Option<ObjectId>,
    pub transform: AffineTransform3,
    pub rigid_layout: Option<&'a viboceros_command::nonuniform_scale::RigidLayout>,
}

impl<'a> TransformedObjects<'a> {
    pub(super) fn object_transform(
        self,
        id: ObjectId,
    ) -> Result<AffineTransform3, viboceros_geometry::GeometryError> {
        if let Some(center) = self.rigid_layout.and_then(|layout| layout.center(id)) {
            viboceros_command::nonuniform_scale::rigid_map(center, self.transform)
        } else {
            Ok(self.transform)
        }
    }
    pub(super) fn reflection(
        sources: &'a [ObjectId],
        reference_sources: bool,
        transform: AffineTransform3,
    ) -> Self {
        Self {
            sources,
            grips: &[],
            copy: false,
            reference_sources,
            draw_source_faces: false,
            reversing: true,
            reference: None,
            transform,
            rigid_layout: None,
        }
    }
}
