use super::*;
use viboceros_geometry::{LoftEndPoints, LoftOptions, LoftStyle};

pub const USAGE: &str = "Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No] [SplitAtTangents=Yes|No] [Natural] [StartPoint=x,y,z] [EndPoint=x,y,z] [Rebuild=0..512|Refit=tolerance]";

mod options;
pub use options::Options;

pub(super) struct LoftCommand(pub crease_splitting::Preference);

impl Command for LoftCommand {
    fn name(&self) -> &'static str {
        "Loft"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let input = Options::parse(arguments)?;
        let ends = input.ends;
        let selected = selected_ids(document)?;
        let profiles = selected
            .iter()
            .map(|id| {
                document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::LoftRequiresCurves)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let options = LoftOptions {
            crease_splitting: self.0.get().0,
            split_at_tangents: input.split_at_tangents,
            natural_seams: input.natural_seams,
        };
        let brep = if ends.is_empty() {
            Brep::try_loft_with_surface_options(
                &profiles,
                input.style,
                input.closed,
                options,
                document.tolerance(),
                &mut |c, s, closed| {
                    simplified_surface(c, s, closed, ends, input, document.tolerance())
                },
            )?
        } else {
            Brep::try_loft_with_surface_end_points(
                &profiles,
                input.style,
                ends,
                options,
                document.tolerance(),
                &mut |c, s, ends| {
                    simplified_surface(c, s, false, ends, input, document.tolerance())
                },
            )?
        };
        let faces = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Lofted {} curve sections into one BRep with {faces} faces",
            profiles.len(),
        ))
    }
}

fn simplified_surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    options: Options,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    if let Some(fit_tolerance) = options.refit_tolerance.filter(|t| *t > 0.) {
        let sections =
            viboceros_geometry::try_prepare_refitted_loft_sections(curves, fit_tolerance)?;
        return if ends.is_empty() {
            surface(&sections, style, closed)
        } else {
            surface_with_end_points(&sections, style, ends)
        };
    }
    if options.rebuild_point_count != 0 {
        rebuilt_surface(
            curves,
            style,
            closed,
            ends,
            options.rebuild_point_count,
            tolerance,
        )
    } else if ends.is_empty() {
        surface(curves, style, closed)
    } else {
        surface_with_end_points(curves, style, ends)
    }
}

pub(super) fn rebuilt_surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    count: usize,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface_rebuilt(curves, style, closed, ends, count, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves_rebuilt(
            curves, style, closed, ends, count, tolerance,
        )?)
    }
}

fn surface_with_end_points(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface_with_end_points(curves, style, ends)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves_with_end_points(
            curves, style, ends,
        )?)
    }
}

pub(super) fn surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::loft_surface(curves, style, closed)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::try_loft_nurbs_curves(
            curves, style, closed,
        )?)
    }
}

#[cfg(test)]
mod refit_tests;
#[cfg(test)]
mod tests;
