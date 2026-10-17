//! Whole-object staging shared by curve and surface Flow, with grip precedence.
use super::*;
use std::collections::BTreeSet;
use viboceros_document::ControlPointId;
use viboceros_geometry::{AffineTransform3, PointMorph};
pub(super) trait ObjectMorph: PointMorph + Clone {
    fn rigid_pose(&self, center: Point3) -> Result<AffineTransform3, GeometryError>;
    fn preserving(self, preserve: bool) -> Self;
}
pub(super) fn deformed<M: ObjectMorph>(
    doc: &Document,
    ids: &[ObjectId],
    grips: &[ControlPointId],
    morph: &M,
    rigid: bool,
    rigid_groups: bool,
    origin: crate::rigid_groups::GroupFrameOrigin,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let grip_owners = grips.iter().map(|g| g.object).collect::<BTreeSet<_>>();
    let context = ids
        .iter()
        .copied()
        .chain(grip_owners.iter().copied())
        .collect::<BTreeSet<_>>();
    let ordinary = ids
        .iter()
        .copied()
        .filter(|id| !grip_owners.contains(id))
        .collect::<Vec<_>>();
    let origins = if rigid && rigid_groups && !ordinary.is_empty() {
        crate::rigid_groups::group_origins(
            doc,
            context.iter().copied(),
            origin,
            Tolerance::NUMERICAL_VALIDATION,
        )?
    } else {
        Default::default()
    };
    let transforms = origins
        .into_iter()
        .map(|(id, center)| Ok((id, morph.rigid_pose(center)?)))
        .collect::<Result<std::collections::BTreeMap<_, _>, GeometryError>>()?;
    ordinary
        .iter()
        .map(|id| {
            let object = doc.object(*id).ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if rigid {
                let transform = if rigid_groups && let Some(group) = object.top_group() {
                    transforms[&group]
                } else {
                    morph.rigid_pose(
                        object
                            .geometry()
                            .tight_bounds(Tolerance::NUMERICAL_VALIDATION)?
                            .center()?,
                    )?
                };
                object.geometry().transformed(transform, doc.tolerance())?
            } else {
                // Multi-face B-reps retain the shared edge/surface fitting path.
                let morph = if matches!(object.geometry(),Geometry::Brep(b)if b.faces().len()>1) {
                    morph.clone().preserving(false)
                } else {
                    morph.clone()
                };
                object.geometry().morphed(&morph, doc.tolerance())?
            };
            Ok((*id, geometry))
        })
        .collect()
}
