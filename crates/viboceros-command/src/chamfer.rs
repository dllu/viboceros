//! Ordered curve chamfers with remembered distances and atomic source edits.
use super::*;
use viboceros_geometry::{
    CurveArcExtensionStyle, CurveChamferExtensionStyles, CurveOtherExtensionStyle,
};
mod closed;
mod options;
mod prepare;
mod same_source;
pub use options::{Arguments, Preferences, parse, parse_distance};
pub use prepare::{Prepared, prepare};
const USAGE: &str = "Chamfer [distance1 distance2 | Distances=d1,d2] [Sources=id1,id2] [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [Trim=Yes|No] [ExtendArcsBy=Arc|Line] [ExtendOtherCurvesBy=Line|Smooth]";
pub(super) struct ChamferCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);
impl Command for ChamferCommand {
    fn name(&self) -> &'static str {
        "Chamfer"
    }
    fn run(&self, document: &mut Document, words: &[&str]) -> Result<String, CommandError> {
        let args = parse(words, self.0.get(), document.units())?;
        let ids = if let Some(ids) = args.sources {
            ids
        } else {
            let selected = document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            match selected.as_slice() {
                [id] if args.picks.iter().all(Option::is_some) => [*id, *id],
                [a, b] => [*a, *b],
                _ => return Err(CommandError::ChamferRequiresTwoCurves),
            }
        };
        let curves = ids.map(|id| {
            document
                .object(id)
                .and_then(|o| o.geometry().curve_ref())
                .map(|c| c.to_owned())
                .ok_or(CommandError::ChamferRequiresTwoCurves)
        });
        let [a, b] = curves;
        let (a, b) = super::fillet::nearest_ends(&a?, &b?)?;
        prepare(
            document,
            [
                (ids[0], args.picks[0].unwrap_or(a)),
                (ids[1], args.picks[1].unwrap_or(b)),
            ],
            args.preferences,
        )?
        .apply(document)?;
        self.0.set(args.preferences);
        Ok("Chamfered curves".into())
    }
    fn object_selection_prompt(
        &self,
        words: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let args = parse(
            words,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
        )?;
        if args.sources.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}
#[cfg(test)]
mod tests;
