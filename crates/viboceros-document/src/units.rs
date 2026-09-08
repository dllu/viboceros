use super::{Document, DocumentError, Edit, Geometry, ObjectId};
use viboceros_geometry::{AffineTransform3, LengthUnitSystem, Point3, Tolerance};

impl Document {
    /// Changes model units as one atomic, undoable document-setting edit.
    /// When rescale is true, all geometry (including hidden/locked objects)
    /// and the absolute tolerance are scaled to preserve physical size.
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
        let tolerance = Tolerance::try_new(
            self.tolerance.absolute() * scale,
            self.tolerance.relative(),
            self.tolerance.angular(),
        )?;
        // Complete every fallible transformation before touching the document.
        let geometries = if scale != 1.0 {
            let transform =
                AffineTransform3::try_uniform_scale(Point3::try_new(0.0, 0.0, 0.0)?, scale)?;
            let staged = self
                .objects
                .iter()
                .map(|object| object.geometry.transformed(transform, tolerance))
                .collect::<Result<Vec<_>, _>>()?;
            Some(
                self.objects
                    .iter_mut()
                    .zip(staged)
                    .map(|(object, geometry)| {
                        (object.id, std::mem::replace(&mut object.geometry, geometry))
                    })
                    .collect(),
            )
        } else {
            None
        };
        let old_units = std::mem::replace(&mut self.units, units);
        let old_tolerance = std::mem::replace(&mut self.tolerance, tolerance);
        self.record_edit(
            "Units",
            Edit::UnitsChanged {
                units: old_units,
                tolerance: old_tolerance,
                geometries,
            },
        );
        Ok(true)
    }
}

pub(super) fn exchange_units(
    document: &mut Document,
    units: &mut LengthUnitSystem,
    tolerance: &mut Tolerance,
    geometries: &mut Option<Vec<(ObjectId, Geometry)>>,
) -> Result<(), DocumentError> {
    if let Some(geometries) = geometries {
        if geometries.len() != document.objects.len()
            || geometries
                .iter()
                .zip(&document.objects)
                .any(|((id, _), object)| *id != object.id)
        {
            return Err(DocumentError::HistoryInvariant(
                "unit-change object order differs from its snapshot",
            ));
        }
        for ((_, geometry), object) in geometries.iter_mut().zip(&mut document.objects) {
            std::mem::swap(geometry, &mut object.geometry);
        }
    }
    std::mem::swap(units, &mut document.units);
    std::mem::swap(tolerance, &mut document.tolerance);
    Ok(())
}

#[cfg(test)]
mod tests;
