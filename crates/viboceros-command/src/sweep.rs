use super::*;
use viboceros_geometry::{Sweep1, SweepBlend, SweepEndPoints, SweepFrameStyle, SweepSection};

const USAGE: &str = "Sweep1 [Parameters=native-rail-parameters] [RailName=name|RailId=id] [FrameStyle=Freeform|Roadlike] [Axis=x,y,z] [GlobalShapeBlending=Yes|No] [RefitRail=Yes|No] [Simplify=None|Rebuild] [Rebuild=point-count] [StartPoint=x,y,z] [EndPoint=x,y,z]";

pub(super) fn rebuild_sections(
    sections: &[SweepSection],
    count: usize,
    tolerance: Tolerance,
) -> Result<Vec<SweepSection>, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::rebuild_sweep_sections(sections, count, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(Sweep1::rebuild_sections(sections, count, tolerance)?)
    }
}

pub(super) fn model(
    rail: CurveRef<'_>,
    sections: &[SweepSection],
    style: SweepFrameStyle,
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<Sweep1, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::sweep_model(rail, sections, style, blend, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(Sweep1::try_new(rail, sections, style, blend, tolerance)?)
    }
}

pub(super) fn model_with_point_ends(
    rail: CurveRef<'_>,
    sections: &[SweepSection],
    ends: SweepEndPoints,
    style: SweepFrameStyle,
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<Sweep1, CommandError> {
    if ends.is_empty() {
        return model(rail, sections, style, blend, tolerance);
    }
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::sweep_model_with_point_ends(rail, sections, ends, style, blend, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(Sweep1::try_new_with_point_ends(
            rail, sections, ends, style, blend, tolerance,
        )?)
    }
}

pub(super) fn surface(sweep: &Sweep1, refit: bool) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::sweep_surface(sweep, refit).map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(if refit {
            sweep.to_surface()?
        } else {
            sweep.to_rail_basis_surface()?
        })
    }
}

pub(super) fn fitted_model_surface(sweep: &Sweep1) -> Result<NurbsSurface, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_sweep_model_surface(sweep)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(sweep.fit_model_surface()?)
    }
}

mod options;
mod placement;
pub use options::Options;

pub(super) struct SweepCommand;

impl Command for SweepCommand {
    fn name(&self) -> &'static str {
        "Sweep1"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Options::parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Sweep1",
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let Options {
            rail_name,
            rail_id,
            parameters,
            roadlike,
            axis,
            global,
            refit,
            rebuild_point_count,
            ends,
        } = Options::parse(arguments)?;
        let rail_name = rail_name.as_deref();
        let selected = selected_ids(document)?;
        let rail_id = if let Some(id) = rail_id {
            id
        } else if rail_name.is_some() || parameters.is_some() {
            extrude_along_curve::resolve_curve_along_curve_path(document, &selected, rail_name)?
        } else {
            selected[0]
        };
        if !document.is_object_selectable(rail_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let rail = document
            .object(rail_id)
            .unwrap()
            .geometry()
            .curve_ref()
            .ok_or(CommandError::Usage(USAGE))?;
        let profiles = selected
            .into_iter()
            .filter(|id| *id != rail_id)
            .collect::<Vec<_>>();
        if profiles.is_empty()
            || parameters
                .as_ref()
                .is_some_and(|p| profiles.len() != p.len())
        {
            return Err(CommandError::Usage(USAGE));
        }
        let curves = profiles
            .iter()
            .map(|id| {
                let curve = document
                    .object(*id)
                    .unwrap()
                    .geometry()
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))?;
                Ok(curve)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let (automatic_rail, sections) = if let Some(parameters) = parameters {
            (
                None,
                curves
                    .into_iter()
                    .zip(parameters)
                    .map(|(curve, parameter)| SweepSection { parameter, curve })
                    .collect(),
            )
        } else {
            let (rail, sections) = if ends.is_empty() {
                placement::sections(rail, curves, document.tolerance())?
            } else {
                placement::sections_with_point_ends(rail, curves, document.tolerance())?
            };
            (Some(rail), sections)
        };
        let full_rail_anchor = if ends.is_empty()
            && !refit.unwrap_or(false)
            && let Some(rail) = &automatic_rail
        {
            placement::full_rail_anchor(rail, &sections)?
        } else {
            false
        };
        // Captured partial line intervals use a cubic refit. Keeping the line
        // basis instead changes the geometry between two interior profiles.
        let automatic_partial = automatic_rail.as_ref().is_some_and(|rail| {
            !full_rail_anchor
                && (sections[0].parameter != *rail.domain().start()
                    || (sections.len() > 1
                        && sections.last().unwrap().parameter != *rail.domain().end()))
        });
        let style = if roadlike.unwrap_or(false) {
            match axis {
                Some(axis) => SweepFrameStyle::Roadlike(axis),
                None => SweepFrameStyle::roadlike_default(rail, document.tolerance())?,
            }
        } else {
            SweepFrameStyle::Freeform
        };
        let rail = automatic_rail.as_ref().map_or(rail, CurveRef::NurbsCurve);
        let sections = if rebuild_point_count > 1 {
            rebuild_sections(&sections, rebuild_point_count, document.tolerance())?
        } else {
            sections
        };
        let blend = if global.unwrap_or(false) {
            SweepBlend::Global
        } else {
            SweepBlend::Local
        };
        let mut sweep =
            model_with_point_ends(rail, &sections, ends, style, blend, document.tolerance())?;
        if full_rail_anchor {
            sweep = sweep.try_on_full_rail()?;
        }
        let surface = surface(
            &sweep,
            refit.unwrap_or(false) || automatic_partial || !ends.is_empty(),
        )?;
        let [u, v] = surface.sampled_kink_parameters(document.tolerance().angular())?;
        let brep = Brep::try_surface_grid(&surface, &u, &v, document.tolerance())?;
        let count = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Swept {} sections into one BRep with {count} faces",
            sections.len()
        ))
    }
}

#[cfg(test)]
mod arc_station_tests;
#[cfg(test)]
mod automatic_tests;
#[cfg(test)]
mod curved_tests;
#[cfg(test)]
mod point_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod rebuild_tests;
#[cfg(test)]
mod roadlike_tests;
