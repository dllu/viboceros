//! Geometry admission policy, separate from transforms and historical snapshots.
use super::*;
use viboceros_geometry::BrepSolidOrientation;

#[cfg(test)]
mod tests;

/// Only an exact, supported inward witness authorizes whole-object reversal.
/// Unknown is not inward: preserve unsupported/coincident geometry and never
/// fall back to signed volume or independently orient connected components.
pub(super) fn normalize_geometry(mut geometry: Geometry) -> Result<Geometry, GeometryError> {
    if let Geometry::Brep(brep) = &mut geometry
        && brep.solid_orientation()? == BrepSolidOrientation::Inward
    {
        brep.reverse_orientation();
    }
    Ok(geometry)
}

impl Document {
    /// Adds geometry on the current layer, globally reversing known inward
    /// B-rep solids. Open/inconsistent B-reps, unknown solid orientations and
    /// non-B-rep geometry retain their supplied sense.
    pub fn add_geometry(&mut self, geometry: Geometry) -> Result<ObjectId, DocumentError> {
        self.add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(self.current_layer))
    }

    /// Adds geometry with the same admission policy as [`Self::add_geometry`].
    /// Permissions and all fallible geometry work precede object/history edits.
    /// File import also uses this policy; low-level file readers stay lossless.
    pub fn add_geometry_with_attributes(
        &mut self,
        geometry: Geometry,
        attributes: ObjectAttributes,
    ) -> Result<ObjectId, DocumentError> {
        self.add_geometry_with_metadata(geometry, attributes, BTreeMap::new())
    }

    /// Inserts geometry with its geometry-attached user text in one history edit.
    pub fn add_geometry_with_metadata(
        &mut self,
        geometry: Geometry,
        attributes: ObjectAttributes,
        geometry_user_text: BTreeMap<String, String>,
    ) -> Result<ObjectId, DocumentError> {
        let mut folded_keys = BTreeSet::new();
        for (key, value) in &geometry_user_text {
            validate_user_text(key, Some(value))?;
            if value.is_empty() {
                return Err(DocumentError::InvalidUserText("value is empty"));
            }
            if !folded_keys.insert(key.to_lowercase()) {
                return Err(DocumentError::InvalidUserText("duplicate key"));
            }
        }
        let layer = self
            .layer(attributes.layer_id)
            .ok_or(DocumentError::LayerNotFound(attributes.layer_id))?;
        if layer.locked {
            return Err(DocumentError::LayerLocked(layer.id));
        }
        let geometry = normalize_geometry(geometry)?;
        let id = ObjectId::new();
        let index = self.objects.len();
        self.objects.push(Object {
            id,
            geometry: geometry.into(),
            geometry_user_text,
            attributes,
            isolation: ObjectIsolation::None,
            group_ids: Vec::new(),
        });
        self.record_edit(
            "Add object",
            Edit::ObjectInserted {
                index,
                id,
                stored: None,
                selected: false,
            },
        );
        Ok(id)
    }
}
