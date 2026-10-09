use super::{BlockDefinition, Document, DocumentError, Edit, GeometrySnapshot, ObjectId};
use viboceros_geometry::{AffineTransform3, LengthUnitSystem, Point3, Tolerance};

impl Document {
    /// Changes model units as one atomic, undoable document-setting edit.
    /// When rescale is true, all geometry (including hidden/locked objects)
    /// is scaled to preserve physical size. Numeric tolerances stay unchanged.
    /// Otherwise only metadata changes. Selection, attributes, groups, and
    /// object order are preserved. No history entry is created for a no-op.
    /// Unitless conversions retain coordinates; rescaling involving unset
    /// units is rejected. This is a document API, not a Rhino command emulation.
    pub fn set_units(
        &mut self,
        units: LengthUnitSystem,
        rescale: bool,
    ) -> Result<bool, DocumentError> {
        units.validate()?;
        if units == self.units {
            return Ok(false);
        }
        let scale = if rescale {
            self.units.scale_to(&units)?
        } else {
            1.0
        };
        // Complete every fallible transformation before touching the document.
        let staged_definitions = if scale != 1.0 && !self.block_definitions.is_empty() {
            Some(super::blocks::rescaled_definitions(self, scale)?)
        } else {
            None
        };
        let geometries = if scale != 1.0 {
            let transform =
                AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0)?, scale)?;
            let staged = self
                .objects
                .iter()
                .map(|object| {
                    // Model tolerance is not a minimum feature size for existing
                    // geometry. In particular it stays numerically unchanged
                    // across conversions, so scaling it here breaks round trips.
                    // Brep reconstruction additionally checks approximate topology;
                    // retain its dimensional matching allowance while the kernel
                    // scales stored vertex/edge tolerances and validates edge curves.
                    let tolerance = super::blocks::transformation_tolerance(
                        &object.geometry,
                        self.tolerance,
                        scale,
                    )?;
                    object.geometry.transformed(transform, tolerance)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Some(
                self.objects
                    .iter_mut()
                    .zip(staged)
                    .map(|(object, geometry)| {
                        (
                            object.id,
                            std::mem::replace(&mut object.geometry, geometry.into()),
                        )
                    })
                    .collect(),
            )
        } else {
            None
        };
        let block_definitions = staged_definitions
            .map(|definitions| std::mem::replace(&mut self.block_definitions, definitions));
        let old_units = std::mem::replace(&mut self.units, units);
        let old_tolerance = self.tolerance;
        self.record_edit(
            "Units",
            Edit::UnitsChanged {
                units: old_units,
                tolerance: old_tolerance,
                geometries,
                block_definitions,
            },
        );
        if scale != 1.0 {
            self.synchronize_control_points();
        }
        Ok(true)
    }
}

pub(super) fn exchange_units(
    document: &mut Document,
    units: &mut LengthUnitSystem,
    tolerance: &mut Tolerance,
    geometries: &mut Option<Vec<(ObjectId, GeometrySnapshot)>>,
    block_definitions: &mut Option<Vec<BlockDefinition>>,
) -> Result<(), DocumentError> {
    // Check both tables before exchanging either one.
    if let Some(definitions) = block_definitions.as_ref()
        && (definitions.len() != document.block_definitions.len()
            || definitions
                .iter()
                .zip(&document.block_definitions)
                .any(|(before, after)| before.id() != after.id()))
    {
        return Err(DocumentError::HistoryInvariant(
            "unit-change definition order differs from its snapshot",
        ));
    }
    if let Some(geometries) = geometries.as_ref()
        && (geometries.len() != document.objects.len()
            || geometries
                .iter()
                .zip(&document.objects)
                .any(|((id, _), object)| *id != object.id))
    {
        return Err(DocumentError::HistoryInvariant(
            "unit-change object order differs from its snapshot",
        ));
    }
    if let Some(geometries) = geometries {
        for ((_, geometry), object) in geometries.iter_mut().zip(&mut document.objects) {
            std::mem::swap(geometry, &mut object.geometry);
        }
    }
    if let Some(definitions) = block_definitions {
        std::mem::swap(definitions, &mut document.block_definitions);
    }
    std::mem::swap(units, &mut document.units);
    std::mem::swap(tolerance, &mut document.tolerance);
    Ok(())
}

#[cfg(test)]
use super::Geometry;
#[cfg(test)]
mod tests;
