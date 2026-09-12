//! Read-only selected-object measurements and exact finite-value aggregation.

use super::{Command, CommandError, geometry_curve_ref, require_consumed};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{FiniteSum, GeometryError, Real, Tolerance};

mod distance;
#[cfg(test)]
mod tests;
pub(super) use distance::DistanceCommand;

fn format_measurement(value: Real) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if !(1e-6..1e12).contains(&value.abs()) {
        format!("{value:e}")
    } else {
        value.to_string()
    }
}

pub(super) struct LengthCommand;

impl Command for LengthCommand {
    fn name(&self) -> &'static str {
        "Length"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Len"]
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Length")?;
        let (count, total) = selected_measurement(
            document,
            MeasurementSign::Nonnegative,
            |geometry, tolerance| {
                geometry_curve_ref(geometry)
                    .ok_or(CommandError::UnsupportedLengthGeometry)?
                    .length(tolerance)
                    .map_err(CommandError::from)
            },
        )?;
        let total = format_measurement(total);
        Ok(format!("Measured {count} curve(s): total length {total}"))
    }
}

pub(super) struct AreaCommand;

impl Command for AreaCommand {
    fn name(&self) -> &'static str {
        "Area"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Area")?;
        let (count, total) = selected_measurement(
            document,
            MeasurementSign::Nonnegative,
            |geometry, tolerance| match geometry {
                Geometry::NurbsSurface(surface) => Ok(surface.area(tolerance)?),
                Geometry::Brep(brep) => Ok(brep.area(tolerance)?),
                Geometry::Mesh(mesh) => Ok(mesh.area()?),
                _ => geometry_curve_ref(geometry)
                    .ok_or(CommandError::UnsupportedAreaGeometry)?
                    .planar_area(tolerance)
                    .map_err(CommandError::from),
            },
        )?;
        let total = format_measurement(total);
        Ok(format!("Measured {count} object(s): total area {total}"))
    }
}

pub(super) struct VolumeCommand;

impl Command for VolumeCommand {
    fn name(&self) -> &'static str {
        "Volume"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Volume")?;
        let (count, total) =
            selected_measurement(document, MeasurementSign::Signed, |geometry, tolerance| {
                Ok(match geometry {
                    Geometry::Mesh(mesh) => {
                        if !mesh.topology().is_closed() {
                            return Err(CommandError::OpenMeshVolume);
                        }
                        mesh.signed_volume()?
                    }
                    Geometry::Brep(brep) => {
                        if !brep.is_solid() {
                            return Err(CommandError::OpenBrepVolume);
                        }
                        brep.signed_volume(tolerance)?
                    }
                    _ => return Err(CommandError::UnsupportedVolumeGeometry),
                })
            })?;
        let total = format_measurement(total);
        Ok(format!(
            "Measured {count} closed object(s): total volume {total}"
        ))
    }
}

#[derive(Clone, Copy)]
enum MeasurementSign {
    Nonnegative,
    Signed,
}

fn selected_measurement(
    document: &Document,
    sign: MeasurementSign,
    mut measure: impl FnMut(&Geometry, Tolerance) -> Result<Real, CommandError>,
) -> Result<(usize, Real), CommandError> {
    let mut count = 0;
    let mut sum = FiniteSum::default();
    for object in document.selected_objects() {
        let value = measure(object.geometry(), document.tolerance())?;
        if !value.is_finite() || (matches!(sign, MeasurementSign::Nonnegative) && value < 0.0) {
            return Err(GeometryError::NonFinite {
                context: "geometry measurement",
            }
            .into());
        }
        sum.add(value)?;
        count += 1;
    }
    if count == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    Ok((count, sum.total()?))
}
