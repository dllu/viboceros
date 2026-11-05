//! Read-only selected-object measurements and exact finite-value aggregation.

use super::{
    Command, CommandError, ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow,
    geometry_curve_ref, option_name_eq, require_consumed,
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{FiniteSum, GeometryError, LengthUnitSystem, Real, Tolerance};

mod angle;
mod length;
pub(super) use length::LengthCommand;
mod area_centroid;
mod volume;
pub(super) use area_centroid::AreaCentroidCommand;
pub(super) use volume::VolumeCommand;
mod distance;
mod domain;
mod subcurve;
pub(super) use domain::DomainCommand;
mod evaluate_point;
mod evaluate_uv;
pub(super) use angle::AngleCommand;
pub(super) use evaluate_point::EvaluatePointCommand;
pub(super) use evaluate_uv::EvaluateUvCommand;
pub use evaluate_uv::{
    EvaluateUvOptions, EvaluateUvResult, evaluate_surface_uv, evaluate_surface_uv_on_face,
};
#[cfg(test)]
mod tests;
pub(super) use distance::DistanceCommand;
pub use distance::distance_display_units;

fn format_measurement(value: Real) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if !(1e-6..1e12).contains(&value.abs()) {
        format!("{value:e}")
    } else {
        value.to_string()
    }
}

pub(crate) fn measurement_display_option(
    argument: &str,
    usage: &'static str,
) -> Result<Option<LengthUnitSystem>, CommandError> {
    let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(usage))?;
    if !option_name_eq(name, "Units") {
        return Err(CommandError::Usage(usage));
    }
    Ok(distance_display_units(value)?.and_then(crate::model_units::parse_units))
}

pub(crate) fn measurement_display_scale(
    document: &Document,
    target: Option<&LengthUnitSystem>,
    source_units_error: &'static str,
) -> Result<Real, CommandError> {
    let Some(target) = target else {
        return Ok(1.0);
    };
    if document
        .units()
        .meters_per_unit()
        .map_err(viboceros_document::DocumentError::from)?
        .is_none()
    {
        return Err(CommandError::Usage(source_units_error));
    }
    Ok(document
        .units()
        .scale_to(target)
        .map_err(viboceros_document::DocumentError::from)?)
}

pub(super) struct AreaCommand;
const AREA_USAGE: &str = "Area [Units=name]";

impl Command for AreaCommand {
    fn name(&self) -> &'static str {
        "Area"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let eligible = match arguments {
            [] => true,
            [option] => {
                measurement_display_option(option, AREA_USAGE)?;
                true
            }
            _ => false,
        };
        Ok(eligible.then_some(ObjectSelectionPrompt {
            command: "Area",
            filter: ObjectSelectionFilter::Area,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: crate::CommandContext,
    ) -> Result<String, CommandError> {
        let report = self.run(document, arguments)?;
        document.clear_selection();
        Ok(report)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let target = match arguments {
            [] => None,
            [option] => measurement_display_option(option, AREA_USAGE)?,
            _ => return Err(CommandError::Usage(AREA_USAGE)),
        };
        let scale = measurement_display_scale(
            document,
            target.as_ref(),
            "Area display conversion requires physical source units",
        )?;
        let (count, sum) =
            accumulate_selected_measurement(document, |geometry, tolerance| match geometry {
                Geometry::NurbsSurface(surface) => Ok(surface.area(tolerance)?),
                Geometry::Brep(brep) => Ok(brep.area(tolerance)?),
                Geometry::Mesh(mesh) => Ok(mesh.area()?),
                Geometry::SubD(subd) => Ok(subd.area(tolerance)?),
                _ => geometry_curve_ref(geometry)
                    .ok_or(CommandError::UnsupportedAreaGeometry)?
                    .planar_area(tolerance)
                    .map_err(CommandError::from),
            })?;
        let total = if scale == 1.0 {
            sum.total()?
        } else {
            sum.scaled_total_squared(scale).map_err(|_| {
                viboceros_document::DocumentError::from(
                    viboceros_geometry::UnitError::UnrepresentableScale,
                )
            })?
        };
        if total == 0.0 && sum.total()? != 0.0 {
            return Err(viboceros_document::DocumentError::from(
                viboceros_geometry::UnitError::UnrepresentableScale,
            )
            .into());
        }
        Ok(format!(
            "Measured {count} object(s): total area {}{}",
            format_measurement(total),
            target
                .map(|unit| format!(" {}²", unit.name()))
                .unwrap_or_default()
        ))
    }
}

#[cfg(test)]
fn selected_measurement(
    document: &Document,
    measure: impl FnMut(&Geometry, Tolerance) -> Result<Real, CommandError>,
) -> Result<(usize, Real), CommandError> {
    let (count, sum) = accumulate_selected_measurement(document, measure)?;
    Ok((count, sum.total()?))
}

fn accumulate_selected_measurement(
    document: &Document,
    mut measure: impl FnMut(&Geometry, Tolerance) -> Result<Real, CommandError>,
) -> Result<(usize, FiniteSum), CommandError> {
    let mut count = 0;
    let mut sum = FiniteSum::default();
    for object in document.selected_objects() {
        let value = measure(object.geometry(), document.tolerance())?;
        if !value.is_finite() || value < 0.0 {
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
    Ok((count, sum))
}
