//! Two ordered rail picks followed by cross sections, or explicit named rails.
use super::*;

use viboceros_document::GeometrySnapshot;
use viboceros_geometry::SweepEndPoints;

const NAME: &str = "Sweep2";
const USAGE: &str = "Sweep2 [Rail1Name=name Rail2Name=name] [Rebuild=point-count|Refit=tolerance] [MaintainHeight=Yes|No] [RefitRails=Yes|No] [Closed=Yes|No] [StartPoint=x,y,z] [EndPoint=x,y,z] [Slash=x,y,z/x,y,z ...] (select two rails, then cross sections)";
const SLASH_USAGE: &str = "Sweep2 accepts up to 32 options of the form Slash=x,y,z/x,y,z";
pub const MAX_SECTIONS: usize = 32;
pub const MAX_SLASHES: usize = viboceros_smlib::MAX_TWO_RAIL_SLASHES;

mod options;
mod rail;
use crate::chain_selection::Target;
pub use options::{Options, SectionFit, ShapeOptions};
pub use rail::Rail;
pub(super) struct SweepTwoRailsCommand;

impl Command for SweepTwoRailsCommand {
    fn name(&self) -> &'static str {
        NAME
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Options::parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: NAME,
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = Options::parse(args)?;
        let source = Source::capture(doc, &selected_ids(doc)?, &options)?;
        let brep = source.build(options.shape, &options.slashes, doc.tolerance())?;
        let message = result_message(source.sections.len(), brep.faces().len());
        doc.add_geometry(Geometry::Brep(brep))?;
        Ok(message)
    }
}

pub fn result_message(section_count: usize, face_count: usize) -> String {
    format!("Swept {section_count} sections along two rails into one BRep with {face_count} faces")
}

/// Immutable inputs shared by viewport previews and the registered command.
#[derive(Debug)]
pub struct Source {
    rails: [Rail; 2],
    sections: Vec<GeometrySnapshot>,
}

impl Source {
    pub fn capture(
        doc: &Document,
        selected: &[ObjectId],
        options: &Options,
    ) -> Result<Self, CommandError> {
        let rails = match options.explicit_rails(doc)? {
            Some(rails) => rails,
            None if selected.len() >= 2 => [selected[0], selected[1]],
            None => return Err(CommandError::Usage(USAGE)),
        };
        if rails[0] == rails[1] {
            return Err(CommandError::Usage(USAGE));
        }
        let rails = [
            Rail::capture(doc, Target::Curve(rails[0]))?,
            Rail::capture(doc, Target::Curve(rails[1]))?,
        ];
        Self::with_rails(doc, rails, selected)
    }

    pub fn with_rails(
        doc: &Document,
        rails: [Rail; 2],
        sections: &[ObjectId],
    ) -> Result<Self, CommandError> {
        let mut rail_targets = std::collections::BTreeSet::new();
        if !rails.iter().all(|rail| rail.current(doc)) {
            return Err(CommandError::ChainStale);
        }
        if rails
            .iter()
            .flat_map(Rail::targets)
            .any(|target| !rail_targets.insert(target))
        {
            return Err(CommandError::Usage("Sweep2: select distinct rail pieces"));
        }
        let sections = sections
            .iter()
            .copied()
            .filter(|id| !rail_targets.contains(&Target::Curve(*id)))
            .take(MAX_SECTIONS + 1)
            .map(|id| {
                doc.object(id)
                    .filter(|o| doc.is_object_selectable(id) && o.geometry().curve_ref().is_some())
                    .map(|o| o.geometry_snapshot().clone())
                    .ok_or(CommandError::Usage(USAGE))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if sections.len() > MAX_SECTIONS {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self { rails, sections })
    }

    fn curve(source: &GeometrySnapshot) -> Result<NurbsCurve, CommandError> {
        source
            .nurbs_curve_representation()?
            .ok_or(CommandError::Usage(USAGE))
    }

    pub fn validate_point_ends(
        &self,
        ends: SweepEndPoints,
        tolerance: Tolerance,
    ) -> Result<(), CommandError> {
        for rail in &self.rails {
            if !ends.is_empty() && rail.is_closed() {
                return Err(CommandError::Usage("Sweep2 point ends require open rails"));
            }
            viboceros_smlib::two_rail_point_stations(rail.curve(), ends, tolerance)?;
        }
        Ok(())
    }

    fn fitted_profiles(
        &self,
        fit: SectionFit,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, CommandError> {
        let sections = self
            .sections
            .iter()
            .map(Self::curve)
            .collect::<Result<Vec<_>, _>>()?;
        match fit {
            SectionFit::Unchanged => Ok(sections),
            SectionFit::Rebuild(count @ 2..=128) => sections
                .iter()
                .map(|curve| {
                    viboceros_smlib::rebuild_curve(
                        CurveRef::NurbsCurve(curve),
                        count,
                        3.min(count - 1),
                        false,
                        tolerance,
                    )
                    .map_err(knot_refinement::native_geometry_error)
                })
                .collect(),
            SectionFit::Rebuild(_) => Err(CommandError::Usage(USAGE)),
            SectionFit::Refit(distance) => Ok(
                viboceros_geometry::try_prepare_refitted_loft_sections(&sections, distance)?,
            ),
        }
    }

    pub fn slash_paths(
        &self,
        shape: ShapeOptions,
        slashes: &[[Point3; 2]],
        tolerance: Tolerance,
    ) -> Result<[NurbsCurve; 2], CommandError> {
        let sections = self.fitted_profiles(shape.fit, tolerance)?;
        Ok(viboceros_smlib::two_rail_slash_paths(
            self.rails[0].curve(),
            self.rails[1].curve(),
            &sections,
            shape.native(slashes),
            tolerance,
        )?)
    }

    pub fn build(
        &self,
        shape: ShapeOptions,
        slashes: &[[Point3; 2]],
        tolerance: Tolerance,
    ) -> Result<Brep, CommandError> {
        let row_count = self.sections.len() + point_count(shape.ends);
        if self.sections.is_empty() || row_count > MAX_SECTIONS {
            return Err(CommandError::Usage(USAGE));
        }
        let sections = self.fitted_profiles(shape.fit, tolerance)?;
        let surface = viboceros_smlib::two_rail_sweep_surface_with_options(
            self.rails[0].curve(),
            self.rails[1].curve(),
            &sections,
            shape.native(slashes),
            tolerance,
        )?;
        let [mut u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        if surface.is_rational() && self.rails.iter().all(Rail::is_closed) {
            // The exact rational loop retains the span boundaries of the rail basis.
            let domain = surface.domain_u();
            u.extend(
                surface
                    .knots_u()
                    .chunk_by(|a, b| a == b)
                    .filter_map(|group| {
                        let knot = group[0];
                        (group.len() == surface.degree_u()
                            && *domain.start() < knot
                            && knot < *domain.end())
                        .then_some(knot)
                    }),
            );
            u.sort_by(f64::total_cmp);
            u.dedup();
        }
        if (u.len() + 1).saturating_mul(v.len() + 1) > 4096 {
            return Err(CommandError::Usage("Sweep2 output exceeds 4096 faces"));
        }
        Ok(Brep::try_surface_grid(&surface, &u, &v, tolerance)?)
    }
}

pub fn point_count(ends: SweepEndPoints) -> usize {
    [ends.start, ends.end].into_iter().flatten().count()
}

#[cfg(test)]
mod closed_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod point_tests;

#[cfg(test)]
mod rail_tests;

#[cfg(test)]
mod slash_tests;
