use super::*;
use crate::chain_selection::Target;
pub use crate::sweep_rail::{Rail, RailReference};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{Sweep1, SweepBlend, SweepEndPoints, SweepFrameStyle, SweepSection};

const USAGE: &str = "Sweep1 [Parameters=native-rail-parameters] [RailName=name|RailId=id|RailEdge=id,index[,face]] [FrameStyle=Freeform|Roadlike|Surface] [Axis=x,y,z] [GlobalShapeBlending=Yes|No] [RefitRail=Yes|No] [Simplify=None|Rebuild] [Rebuild=point-count] [StartPoint=x,y,z] [EndPoint=x,y,z]";

pub(super) fn rebuild_sections(
    sections: &[SweepSection],
    count: usize,
    tolerance: Tolerance,
) -> Result<Vec<SweepSection>, CommandError> {
    viboceros_smlib::rebuild_sweep_sections(sections, count, tolerance)
        .map_err(knot_refinement::native_geometry_error)
}

pub(super) fn model(
    rail: CurveRef<'_>,
    sections: &[SweepSection],
    style: SweepFrameStyle,
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<Sweep1, CommandError> {
    viboceros_smlib::sweep_model(rail, sections, style, blend, tolerance)
        .map_err(knot_refinement::native_geometry_error)
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
    {
        viboceros_smlib::sweep_model_with_point_ends(rail, sections, ends, style, blend, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
}

pub(super) fn surface(sweep: &Sweep1, refit: bool) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::sweep_surface(sweep, refit).map_err(knot_refinement::native_geometry_error)
}

pub(super) fn fitted_model_surface(sweep: &Sweep1) -> Result<NurbsSurface, CommandError> {
    viboceros_smlib::fit_sweep_model_surface(sweep).map_err(knot_refinement::native_geometry_error)
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
        let options = Options::parse(arguments)?;
        let source = Source::capture(document, &selected_ids(document)?, &options)?;
        let (brep, section_count) = source.build(&options, document.tolerance())?;
        let count = brep.faces().len();
        document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Swept {section_count} sections into one BRep with {count} faces"
        ))
    }
}

/// Capture the ordered inputs once so drafting and command execution can use
/// the same sweep construction without preview edits to the document.
#[derive(Debug)]
pub struct Source {
    rail: Rail,
    profiles: Vec<GeometrySnapshot>,
}

impl Source {
    pub fn capture(
        document: &Document,
        selected: &[ObjectId],
        options: &Options,
    ) -> Result<Self, CommandError> {
        let rail = if let Some(reference) = options.rail_reference(document)? {
            Rail::capture_reference(document, reference)?
        } else if options.parameters.is_some() {
            let id = extrude_along_curve::resolve_curve_along_curve_path(document, selected, None)?;
            Rail::capture(document, Target::Curve(id))?
        } else {
            let id = *selected.first().ok_or(CommandError::NoObjectsSelected)?;
            Rail::capture(document, Target::Curve(id))?
        };
        Self::with_rail(document, rail, selected, options)
    }

    /// Capture profiles for a privately assembled rail in the interactive getter.
    pub fn with_rail(
        document: &Document,
        rail: Rail,
        selected: &[ObjectId],
        options: &Options,
    ) -> Result<Self, CommandError> {
        if !rail.current(document) {
            return Err(CommandError::Usage(USAGE));
        }
        let owners = rail
            .targets()
            .map(Target::object)
            .collect::<std::collections::BTreeSet<_>>();
        let profiles = selected
            .iter()
            .copied()
            .filter(|id| !owners.contains(id))
            .map(|id| {
                document
                    .object(id)
                    .filter(|object| object.geometry().curve_ref().is_some())
                    .map(|object| object.geometry_snapshot().clone())
                    .ok_or(CommandError::Usage(USAGE))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if profiles.is_empty()
            || options
                .parameters
                .as_ref()
                .is_some_and(|p| profiles.len() != p.len())
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self { rail, profiles })
    }

    /// Produce the exact sweep and the number of real profiles used.
    pub fn build(
        &self,
        options: &Options,
        tolerance: Tolerance,
    ) -> Result<(Brep, usize), CommandError> {
        let Options {
            parameters,
            roadlike,
            axis,
            global,
            refit,
            rebuild_point_count,
            ends,
            ..
        } = options;
        let ends = *ends;
        let rail = self.rail.curve_ref();
        let curves = self
            .profiles
            .iter()
            .map(|profile| {
                let curve = profile
                    .nurbs_curve_representation()?
                    .ok_or(CommandError::Usage(USAGE))?;
                Ok(curve)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let (automatic_rail, sections) = if let Some(parameters) = parameters {
            if parameters.len() != curves.len() {
                return Err(CommandError::Usage(USAGE));
            }
            (
                None,
                curves
                    .into_iter()
                    .zip(parameters.iter().copied())
                    .map(|(curve, parameter)| SweepSection { parameter, curve })
                    .collect(),
            )
        } else {
            let (rail, sections) = if ends.is_empty() {
                placement::sections(rail, curves, tolerance)?
            } else {
                placement::sections_with_point_ends(rail, curves, tolerance)?
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
                Some(axis) => SweepFrameStyle::Roadlike(*axis),
                None => SweepFrameStyle::roadlike_default(rail, tolerance)?,
            }
        } else {
            SweepFrameStyle::Freeform
        };
        let rail = automatic_rail.as_ref().map_or(rail, CurveRef::NurbsCurve);
        let sections = if *rebuild_point_count > 1 {
            rebuild_sections(&sections, *rebuild_point_count, tolerance)?
        } else {
            sections
        };
        let blend = if global.unwrap_or(false) {
            SweepBlend::Global
        } else {
            SweepBlend::Local
        };
        let mut sweep = model_with_point_ends(rail, &sections, ends, style, blend, tolerance)?;
        if options.surface_aligned {
            let [support] = self.rail.supports() else {
                return Err(CommandError::Usage(
                    "Sweep1 AlignWithSurface requires an edge with one supporting face",
                ));
            };
            sweep = sweep.try_align_with_surface(support.target.surface.clone())?;
        }
        if full_rail_anchor {
            sweep = sweep.try_on_full_rail()?;
        }
        let surface = surface(
            &sweep,
            refit.unwrap_or(false)
                || options.surface_aligned
                || automatic_partial
                || !ends.is_empty(),
        )?;
        let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        let brep = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
        Ok((brep, sections.len()))
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
#[cfg(test)]
mod surface_alignment_tests;
