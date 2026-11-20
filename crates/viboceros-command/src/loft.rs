use super::*;
use std::borrow::Cow;
use viboceros_geometry::{LoftEndPoints, LoftOptions, LoftProfiles, LoftStyle};

pub const USAGE: &str = "Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No] [SplitAtTangents=Yes|No] [MatchStartTangent=Yes|No] [MatchEndTangent=Yes|No] [Natural] [StartPoint=x,y,z] [EndPoint=x,y,z] [Rebuild=0..512|Refit=tolerance]";

mod options;
pub use options::Options;
mod tangency;
pub use crate::surface_edge_reference::Edge as TangentSupport;

pub(super) struct LoftCommand(pub crease_splitting::Preference);

impl Command for LoftCommand {
    fn name(&self) -> &'static str {
        "Loft"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let input = Options::parse(arguments)?;
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
        let brep = prepare(
            LoftProfiles::Automatic(&profiles),
            input,
            self.0.get().0,
            document.tolerance(),
        )?;
        let faces = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Lofted {} curve sections into one BRep with {faces} faces",
            profiles.len(),
        ))
    }
}

/// Build the exact draft B-rep through the same solver as the registered command.
pub fn prepare(
    profiles: LoftProfiles<'_>,
    input: Options,
    crease_splitting: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    prepare_with_tangent_supports(profiles, input, crease_splitting, tolerance, [None; 2])
}

/// Build the displayed draft with the supporting faces of the aligned end edges.
pub fn prepare_with_tangent_supports(
    profiles: LoftProfiles<'_>,
    input: Options,
    crease_splitting: bool,
    tolerance: Tolerance,
    supports: [Option<&TangentSupport>; 2],
) -> Result<Brep, CommandError> {
    let supports = tangency::requested_supports(input.active_tangent_matches(), supports)?;
    let ends = input.ends;
    let options = LoftOptions {
        crease_splitting,
        split_at_tangents: input.split_at_tangents,
        natural_seams: input.natural_seams,
    };
    let build = |curves: &[NurbsCurve], style, closed, ends| {
        let sections = simplified_sections(curves, style, closed, ends, input, tolerance)?;
        if supports.iter().any(Option::is_some) {
            let simplified = [
                curves.first() != sections.first(),
                curves.last() != sections.last(),
            ];
            tangency::surface(&sections, style, ends, supports, simplified, tolerance)
        } else if ends.is_empty() {
            surface(&sections, style, closed)
        } else {
            surface_with_end_points(&sections, style, ends)
        }
    };
    Ok(if ends.is_empty() {
        Brep::try_loft_from_profiles(
            profiles,
            input.style,
            input.closed,
            options,
            tolerance,
            &mut |c, s, closed| build(c, s, closed, ends),
        )?
    } else {
        Brep::try_loft_from_profiles_and_points(
            profiles,
            input.style,
            ends,
            options,
            tolerance,
            &mut |c, s, ends| build(c, s, false, ends),
        )?
    })
}

fn simplified_sections(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    options: Options,
    tolerance: Tolerance,
) -> Result<Cow<'_, [NurbsCurve]>, CommandError> {
    if let Some(fit_tolerance) = options.refit_tolerance.filter(|t| *t > 0.) {
        return Ok(Cow::Owned(
            viboceros_geometry::try_prepare_refitted_loft_sections(curves, fit_tolerance)?,
        ));
    }
    if options.rebuild_point_count != 0 {
        return Ok(Cow::Owned(
            viboceros_smlib::rebuild_loft_profiles(
                curves,
                style,
                closed,
                ends,
                options.rebuild_point_count,
                tolerance,
            )
            .map_err(knot_refinement::native_geometry_error)?,
        ));
    }
    Ok(Cow::Borrowed(curves))
}

fn surface_with_end_points(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::loft_surface_with_end_points(curves, style, ends)
        .map_err(knot_refinement::native_geometry_error)
}

pub(super) fn surface(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::loft_surface(curves, style, closed)
        .map_err(knot_refinement::native_geometry_error)
}

#[cfg(test)]
mod refit_tests;
#[cfg(test)]
mod tests;
