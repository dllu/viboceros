//! Exact, atomic face extraction independent of transient viewport selection.
use super::*;
use viboceros_document::GeometrySnapshot;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct Plan {
    id: ObjectId,
    source: GeometrySnapshot,
    attributes: ObjectAttributes,
    extracted: Vec<Geometry>,
    remainder: Option<Geometry>,
}

/// Validated face outputs and source remainders staged before document edits.
#[derive(Clone, Debug)]
pub struct ExtractSurfaceSelection {
    plans: Vec<Plan>,
    tolerance: Tolerance,
    copy: bool,
    output_layer: Option<viboceros_document::LayerId>,
}

impl ExtractSurfaceSelection {
    /// Faces may belong to distinct selectable objects. Repeated targets are
    /// idempotent. Sources are processed in reverse document order and faces
    /// in descending index order, as in native ExtractSrf. Outputs inherit source
    /// attributes but no group memberships. All indices are checked before
    /// geometry is staged, and preparation never changes document state.
    pub fn prepare(
        document: &Document,
        faces: impl IntoIterator<Item = (ObjectId, usize)>,
        copy: bool,
        output_current: bool,
    ) -> Result<Self, CommandError> {
        let mut targets = BTreeMap::<ObjectId, Vec<usize>>::new();
        let mut seen = BTreeSet::new();
        for (count, (id, face)) in faces.into_iter().enumerate() {
            if count >= MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("ExtractSrf"));
            }
            let object = document
                .object(id)
                .filter(|_| document.is_object_selectable(id))
                .ok_or(CommandError::NoExtractableSurfaces)?;
            if !matches!(
                object.geometry(),
                Geometry::Brep(_) | Geometry::NurbsSurface(_)
            ) {
                return Err(CommandError::UnsupportedExtractSurfaceGeometry);
            }
            let face_count = extract_surface_face_count(object.geometry());
            if face >= face_count {
                return Err(CommandError::ExtractSurfaceFaceIndexOutOfRange { face, face_count });
            }
            if seen.insert((id, face)) {
                targets.entry(id).or_default().push(face);
            }
        }
        if targets.is_empty() {
            return Err(CommandError::NoExtractableSurfaces);
        }
        let tolerance = document.tolerance();
        let mut order = document
            .objects()
            .filter(|o| targets.contains_key(&o.id()))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        order.reverse();
        let mut plans = Vec::with_capacity(order.len());
        for id in order {
            let object = document.object(id).unwrap();
            let mut indices = targets.remove(&id).unwrap();
            indices.sort_unstable_by(|a, b| b.cmp(a));
            let (extracted, remainder) = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    (vec![Geometry::NurbsSurface(surface.clone())], None)
                }
                Geometry::Brep(brep) => {
                    let extracted = indices
                        .iter()
                        .map(|&face| brep.duplicate_faces(&[face], tolerance).map(Geometry::Brep))
                        .collect::<Result<Vec<_>, _>>()?;
                    let remaining = (0..brep.faces().len())
                        .filter(|face| !seen.contains(&(id, *face)))
                        .collect::<Vec<_>>();
                    let remainder = if copy || remaining.is_empty() {
                        None
                    } else {
                        Some(Geometry::Brep(brep.sub_brep(&remaining, tolerance)?))
                    };
                    (extracted, remainder)
                }
                _ => unreachable!("validated surface source"),
            };
            plans.push(Plan {
                id,
                source: object.geometry_snapshot().clone(),
                attributes: object.attributes().clone(),
                extracted,
                remainder,
            });
        }
        Ok(Self {
            plans,
            tolerance,
            copy,
            output_layer: output_current.then_some(document.current_layer_id()),
        })
    }

    pub fn commit(&self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "ExtractSrf", |doc| self.apply(doc))
    }

    pub(super) fn apply(&self, document: &mut Document) -> Result<String, CommandError> {
        let sources = self
            .plans
            .iter()
            .map(|plan| plan.id)
            .collect::<BTreeSet<_>>();
        if document.tolerance() != self.tolerance
            || self.plans.iter().any(|plan| {
                !document.is_object_selectable(plan.id)
                    || document.object(plan.id).is_none_or(|object| {
                        object.geometry_snapshot() != &plan.source
                            || object.attributes() != &plan.attributes
                    })
            })
            || self.output_layer.is_some_and(|layer| {
                document.layer(layer).is_none() || document.current_layer_id() != layer
            })
            || !document
                .objects()
                .filter(|object| sources.contains(&object.id()))
                .map(|object| object.id())
                .eq(self.plans.iter().rev().map(|plan| plan.id))
        {
            return Err(CommandError::ExtractSurfaceStale);
        }
        // Native creates extracted faces before renewing the source remainder.
        let mut outputs = Vec::new();
        for plan in &self.plans {
            let attributes = self.output_layer.map_or_else(
                || plan.attributes.clone(),
                |layer| plan.attributes.clone().with_layer(layer),
            );
            for geometry in &plan.extracted {
                outputs.push(
                    document.add_geometry_with_attributes(geometry.clone(), attributes.clone())?,
                );
            }
            if !self.copy {
                if let Some(geometry) = &plan.remainder {
                    document.replace_object_geometries([(plan.id, geometry.clone())])?;
                    document.move_objects_to_end_in_order([plan.id])?;
                } else {
                    document.delete_object(plan.id)?;
                }
            }
        }
        replace_selection(document, outputs.iter().copied())?;
        Ok(format!(
            "Extracted {} surface(s) from {} object(s); source faces {}",
            outputs.len(),
            self.plans.len(),
            if self.copy { "copied" } else { "removed" }
        ))
    }
}
