//! Two ordered rail picks followed by cross sections, or explicit named rails.
use super::*;

use std::borrow::Cow;
use viboceros_document::GeometrySnapshot;

const NAME: &str = "Sweep2";
const USAGE: &str = "Sweep2 [Rail1Name=name Rail2Name=name] [Rebuild=point-count|Refit=tolerance] (select two rails first, then at least two cross sections)";
pub const MAX_SECTIONS: usize = 32;

mod options;
pub use options::{Options, SectionFit};
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
        let brep = source.build(options.fit, doc.tolerance())?;
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
    rails: [GeometrySnapshot; 2],
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
        let capture = |id| {
            doc.object(id)
                .filter(|o| doc.is_object_selectable(id) && o.geometry().curve_ref().is_some())
                .map(|o| o.geometry_snapshot().clone())
                .ok_or(CommandError::Usage(USAGE))
        };
        let sections = selected
            .iter()
            .copied()
            .filter(|id| !rails.contains(id))
            .take(MAX_SECTIONS + 1)
            .map(capture)
            .collect::<Result<Vec<_>, _>>()?;
        if !(2..=MAX_SECTIONS).contains(&sections.len()) {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self {
            rails: [capture(rails[0])?, capture(rails[1])?],
            sections,
        })
    }

    pub fn build(&self, fit: SectionFit, tolerance: Tolerance) -> Result<Brep, CommandError> {
        let convert = |source: &GeometrySnapshot| {
            source
                .nurbs_curve_representation()?
                .ok_or(CommandError::Usage(USAGE))
        };
        let rails = [convert(&self.rails[0])?, convert(&self.rails[1])?];
        let sections = self
            .sections
            .iter()
            .map(convert)
            .collect::<Result<Vec<_>, _>>()?;
        let fitted = match fit {
            SectionFit::Unchanged => Cow::Borrowed(sections.as_slice()),
            SectionFit::Rebuild(count @ 2..=128) => Cow::Owned(
                sections
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
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            SectionFit::Rebuild(_) => return Err(CommandError::Usage(USAGE)),
            SectionFit::Refit(distance) => Cow::Owned(
                viboceros_geometry::try_prepare_refitted_loft_sections(&sections, distance)?,
            ),
        };
        let surface =
            viboceros_smlib::two_rail_sweep_surface(&rails[0], &rails[1], &fitted, tolerance)?;
        let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        if (u.len() + 1).saturating_mul(v.len() + 1) > 4096 {
            return Err(CommandError::Usage("Sweep2 output exceeds 4096 faces"));
        }
        Ok(Brep::try_surface_grid(&surface, &u, &v, tolerance)?)
    }
}

#[cfg(test)]
mod tests;
