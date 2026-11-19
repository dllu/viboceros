//! Original profiles and reference geometry for tapered extrusion getters.
use super::*;
use std::collections::BTreeMap;
use viboceros_command::extrude_surface::surface_extrusion_reference_normal;
use viboceros_command::extrude_tapered::curve_reference_normal;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{BoundingBox3, Brep, NurbsCurve};

pub(super) enum Profile {
    Curve(NurbsCurve),
    Face(Brep),
}

pub(super) struct Sources {
    pub(super) center: Point3,
    pub(super) normal: Option<Vector3>,
    pub(super) profiles: std::sync::Arc<[Profile]>,
    snapshots: Vec<(ObjectId, GeometrySnapshot)>,
}

impl Sources {
    pub(super) fn curves(doc: &Document) -> Option<Self> {
        let mut profiles = Vec::new();
        let mut snapshots = Vec::new();
        let mut bounds = None;
        for object in doc.selected_objects() {
            let Ok(Some(curve)) = object.geometry().nurbs_curve_representation() else {
                continue;
            };
            include_bounds(&mut bounds, object.geometry().bounds()?).ok()?;
            snapshots.push((object.id(), object.geometry_snapshot().clone()));
            profiles.push(Profile::Curve(curve));
        }
        let Profile::Curve(first) = profiles.first()? else {
            return None;
        };
        Some(Self {
            center: bounds?.center().ok()?,
            normal: curve_reference_normal(first, doc.tolerance())
                .ok()
                .flatten(),
            profiles: profiles.into(),
            snapshots,
        })
    }

    pub(super) fn faces(doc: &Document, targets: &[(ObjectId, usize)]) -> Option<Self> {
        let tolerance = doc.tolerance();
        let mut bounds = None;
        let mut profiles = Vec::with_capacity(targets.len());
        let mut snapshots = BTreeMap::new();
        for &(id, face) in targets {
            let object = doc.object(id)?;
            let brep = match object.geometry() {
                Geometry::Brep(brep) => brep.duplicate_faces(&[face], tolerance).ok()?,
                Geometry::NurbsSurface(surface) => {
                    Brep::try_surface_face(surface.clone(), tolerance).ok()?
                }
                _ => return None,
            };
            include_bounds(
                &mut bounds,
                brep.padded_trim_bounds().unwrap_or_else(|_| brep.bounds()),
            )
            .ok()?;
            snapshots
                .entry(id)
                .or_insert_with(|| object.geometry_snapshot().clone());
            profiles.push(Profile::Face(brep));
        }
        let Profile::Face(first) = profiles.first()? else {
            return None;
        };
        Some(Self {
            center: bounds?.center().ok()?,
            normal: surface_extrusion_reference_normal(first, tolerance)
                .ok()
                .flatten(),
            profiles: profiles.into(),
            snapshots: snapshots.into_iter().collect(),
        })
    }

    pub(super) fn is_current(&self, doc: &Document) -> bool {
        self.snapshots.iter().all(|(id, snapshot)| {
            doc.object(*id)
                .filter(|_| doc.is_object_selectable(*id))
                .is_some_and(|object| object.geometry_snapshot() == snapshot)
        }) && (!matches!(&self.profiles[0], Profile::Curve(_))
            || doc
                .selected_objects()
                .filter(|object| object.geometry().curve_ref().is_some())
                .map(|object| object.id())
                .eq(self.snapshots.iter().map(|(id, _)| *id)))
    }
}

fn include_bounds(
    bounds: &mut Option<BoundingBox3>,
    add: BoundingBox3,
) -> Result<(), viboceros_geometry::GeometryError> {
    *bounds = Some(bounds.map_or(Ok(add), |current| current.union(add))?);
    Ok(())
}
