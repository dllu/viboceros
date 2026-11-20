use super::*;
use crate::chain_selection::Target;
pub use crate::sweep_rail::{Rail, RailReference};
use std::collections::BTreeSet;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{
    Sweep1, SweepBlend, SweepEndPoints, SweepFrameStyle, SweepProfileMiterOptions, SweepSection,
};

const USAGE: &str = "Sweep1 [Parameters=native-rail-parameters] [RailName=name|RailId=id|RailEdge=id,index[,face]] [FrameStyle=Freeform|Roadlike|Surface] [Axis=x,y,z] [ShapeBlending=Local|Global] [RefitRail=Yes|No] [Closed=Yes|No] [UntrimmedMiters=Yes|No] [Simplify=None|Rebuild|Refit] [Rebuild=point-count] [Refit=distance] [StartPoint=x,y,z] [EndPoint=x,y,z]";

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

mod closed;
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
        Ok(add_result(document, brep, section_count)?)
    }
}

/// Add each connected sweep component within the caller's command transaction.
pub fn add_result(
    document: &mut Document,
    brep: Brep,
    sections: usize,
) -> Result<String, DocumentError> {
    let faces = brep.faces().len();
    let parts = brep.into_edge_connected_components(document.tolerance())?;
    let count = parts.len();
    // Rhino emits the separate pieces from the end of the rail back to the start.
    for part in parts.into_iter().rev() {
        document.add_geometry(Geometry::Brep(part))?;
    }
    if count == 1 {
        Ok(format!(
            "Swept {sections} sections into one BRep with {faces} faces"
        ))
    } else {
        Ok(format!(
            "Swept {sections} sections into {count} separate BReps with {faces} faces. Some miters could not be joined."
        ))
    }
}

/// Capture the ordered inputs once so drafting and command execution can use
/// the same sweep construction without preview edits to the document.
#[derive(Clone, Debug)]
pub struct Source {
    rail: Rail,
    profiles: Vec<(ObjectId, GeometrySnapshot)>,
    seam_profiles: usize,
}

impl Source {
    pub fn can_close(&self, options: &Options) -> bool {
        self.profiles.len() >= 2 && options.ends.is_empty() && self.rail.is_closed()
    }

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
                    .map(|object| (id, object.geometry_snapshot().clone()))
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
        Ok(Self {
            rail,
            profiles,
            seam_profiles: 0,
        })
    }

    pub fn profile_curves(
        &self,
    ) -> impl Iterator<Item = Result<(ObjectId, NurbsCurve), CommandError>> + '_ {
        self.profiles.iter().map(|(id, profile)| {
            let curve = profile
                .nurbs_curve_representation()?
                .ok_or(CommandError::Usage(USAGE))?;
            Ok((*id, curve))
        })
    }

    pub fn closed_profile_ids(&self) -> Result<Vec<ObjectId>, CommandError> {
        let mut ids = Vec::new();
        for (id, profile) in &self.profiles {
            if profile
                .curve_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .is_closed()?
            {
                ids.push(*id);
            }
        }
        Ok(ids)
    }

    /// Reject explicit stations that collide with a pole before entering the seam editor.
    pub fn validate_point_end_stations(&self, options: &Options) -> Result<(), CommandError> {
        if let Some((first, last)) = options
            .parameters
            .as_ref()
            .and_then(|parameters| parameters.first().zip(parameters.last()))
            && !options.ends.is_empty()
        {
            let domain = self.rail.curve_ref().domain();
            if (options.ends.start.is_some() && first <= domain.start())
                || (options.ends.end.is_some() && last >= domain.end())
            {
                return Err(CommandError::Usage(
                    "Sweep1 profile parameters must lie inside the point ends",
                ));
            }
        }
        Ok(())
    }

    /// Replace captured profiles by ID while preserving section order and the rail.
    pub fn with_profile_curves(
        mut self,
        profiles: impl IntoIterator<Item = (ObjectId, NurbsCurve)>,
    ) -> Result<Self, CommandError> {
        let mut edits = std::collections::BTreeMap::new();
        for (id, curve) in profiles {
            if edits.insert(id, curve).is_some() {
                return Err(CommandError::Usage("Duplicate Sweep1 profile edit"));
            }
        }
        self.seam_profiles = edits.len();
        for (id, profile) in &mut self.profiles {
            if let Some(curve) = edits.remove(id) {
                *profile = Geometry::NurbsCurve(curve).into();
            }
        }
        if !edits.is_empty() {
            return Err(CommandError::Usage(
                "Sweep1 profile edits must match the selected profiles",
            ));
        }
        Ok(self)
    }

    /// Produce the exact sweep and the number of real profiles used.
    pub fn build(
        &self,
        options: &Options,
        tolerance: Tolerance,
    ) -> Result<(Brep, usize), CommandError> {
        self.build_with_reversed_profiles(options, tolerance, &BTreeSet::new())
    }

    /// Apply direction edits after rail placement so alignment keeps profile stations.
    pub fn build_with_reversed_profiles(
        &self,
        options: &Options,
        tolerance: Tolerance,
        reversed_profiles: &BTreeSet<ObjectId>,
    ) -> Result<(Brep, usize), CommandError> {
        self.validate_point_end_stations(options)?;
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
        let closed = options.closed.unwrap_or(false);
        if closed && !self.can_close(options) {
            return Err(CommandError::Usage(closed::REQUIREMENTS));
        }
        let rail = self.rail.curve_ref();
        let curves = self
            .profile_curves()
            .map(|profile| profile.map(|(_, curve)| curve))
            .collect::<Result<Vec<_>, CommandError>>()?;
        let automatic_loop = parameters.is_none() && ends.is_empty() && self.rail.is_closed();
        let single_loop = automatic_loop && curves.len() == 1;
        let place_loop = closed || automatic_loop;
        let loop_profiles_closed = place_loop
            && curves
                .iter()
                .try_fold(true, |all, curve| -> Result<bool, GeometryError> {
                    Ok(all && curve.is_closed()?)
                })?;
        let (placed_rail, mut sections, closed_refit) = if place_loop {
            let (rail, sections, refit) =
                closed::sections(rail, curves, parameters.as_deref(), tolerance)?;
            (Some(rail), sections, refit)
        } else if let Some(parameters) = parameters {
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
                false,
            )
        } else {
            let (rail, sections) = if ends.is_empty() {
                placement::sections(rail, curves, tolerance)?
            } else {
                placement::sections_with_point_ends(rail, curves, tolerance)?
            };
            (Some(rail), sections, false)
        };
        let full_rail_anchor = if !place_loop
            && ends.is_empty()
            && !refit.unwrap_or(false)
            && let Some(rail) = &placed_rail
        {
            placement::full_rail_anchor(rail, &sections)?
        } else {
            false
        };
        // Captured partial line intervals use a cubic refit. Keeping the line
        // basis instead changes the geometry between two interior profiles.
        let automatic_partial = !place_loop
            && placed_rail.as_ref().is_some_and(|rail| {
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
        let rail = placed_rail.as_ref().map_or(rail, CurveRef::NurbsCurve);
        let linear_rail = match rail {
            CurveRef::Line(_) => true,
            CurveRef::NurbsCurve(curve) => curve.degree() == 1 && curve.control_points().len() == 2,
            _ => false,
        };
        let mut remaining = reversed_profiles.clone();
        for ((id, _), section) in self.profiles.iter().zip(&mut sections) {
            if reversed_profiles.contains(id) {
                section.curve = section.curve.reversed()?;
                remaining.remove(id);
            }
        }
        if !remaining.is_empty() {
            return Err(CommandError::Usage(
                "Reverse only selected Sweep1 cross sections",
            ));
        }
        // A straight rail uses cubic U for refitted circles. Other closed
        // profiles can keep the authored rail basis when RefitRail is off.
        let fitted_circular_section = options
            .refit_tolerance
            .is_some_and(|distance| distance > 2. * f64::EPSILON.sqrt())
            && linear_rail
            && sections.iter().try_fold(false, |any, section| {
                let curve = &section.curve;
                Ok::<_, GeometryError>(
                    any || (curve.degree() == 2
                        && curve.is_closed()?
                        && curve.circular_center(Tolerance::DEFAULT)?.is_some()),
                )
            })?;
        let sections = if *rebuild_point_count > 1 {
            rebuild_sections(&sections, *rebuild_point_count, tolerance)?
        } else if let Some(distance) = options.refit_tolerance {
            Sweep1::refit_sections(sections, distance)?
        } else {
            sections
        };
        if ends.is_empty()
            && !options.surface_aligned
            && let [profile] = sections.as_slice()
        {
            let rail_domain = rail.domain();
            let start = if full_rail_anchor {
                *rail_domain.start()
            } else {
                profile.parameter
            };
            if let Some(brep) = Sweep1::try_polyline_miters(
                rail,
                profile,
                start..=*rail_domain.end(),
                style,
                options.untrimmed_miters.unwrap_or_default(),
                tolerance,
            )? {
                return Ok((brep, 1));
            }
        }
        let blend = if global.unwrap_or(false) {
            SweepBlend::Global
        } else {
            SweepBlend::Local
        };
        if !options.surface_aligned
            && let Some(brep) = viboceros_smlib::sweep_profile_miters(
                rail,
                &sections,
                SweepProfileMiterOptions {
                    ends,
                    style,
                    blend,
                    closed,
                    untrimmed: options.untrimmed_miters.unwrap_or_default(),
                },
                tolerance,
            )
            .map_err(knot_refinement::native_geometry_error)?
        {
            return Ok((brep, sections.len()));
        }
        let mut sweep = model_with_point_ends(rail, &sections, ends, style, blend, tolerance)?;
        if options.refit_tolerance.is_some()
            && let [section] = sections.as_slice()
            && (ends.start.is_none() || ends.end.is_none())
        {
            sweep = sweep.try_with_section_domain(section.curve.domain())?;
        }
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
        if closed || single_loop {
            sweep = sweep.try_close_rail()?;
        }
        // A pair of closed sections with different local correspondence needs
        // interior stations to carry the blend. Identical sections can keep
        // the straight rail's linear basis.
        let closed_section_blend = self.seam_profiles == 2
            && sections.len() == 2
            && linear_rail
            && sweep.sections_vary_in_frame();
        let refit = refit.unwrap_or(false)
            || options.surface_aligned
            || closed_refit
            || loop_profiles_closed
            || closed_section_blend
            || fitted_circular_section
            // Manual directions use a cubic rail basis to carry the blend.
            || !reversed_profiles.is_empty()
            || automatic_partial
            || !ends.is_empty();
        let surface = if closed && refit {
            viboceros_smlib::refit_closed_sweep(&sweep)
                .map_err(knot_refinement::native_geometry_error)?
        } else {
            surface(&sweep, refit)?
        };
        let [mut u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        if place_loop && !refit {
            // Sweep1 retains separate faces at fully multiple rail knots,
            // including geometrically smooth rational circle joins.
            u = closed::split_parameters(&surface);
        }
        let brep = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
        Ok((brep, sections.len()))
    }
}

#[cfg(test)]
mod arc_station_tests;
#[cfg(test)]
mod automatic_tests;
#[cfg(test)]
mod closed_tests;
#[cfg(test)]
mod curved_tests;
#[cfg(test)]
mod miter_tests;
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
