//! Ordered curve fillets, shared preparation and command preferences.
use super::*;
use viboceros_geometry::{
    Curve3, CurveArcExtensionStyle, CurveFilletExtensionStyles, CurveOtherExtensionStyle,
};
mod closed;
mod options;
mod prepare;
pub use options::{Arguments, Preferences, parse, parse_radius};
pub use prepare::{Prepared, prepare};
const USAGE: &str = "Fillet [Radius=distance] [Sources=id,id] [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [Trim=Yes|No] [Dynamic=Yes|No] [ExtendArcsBy=Arc|Line] [ExtendOtherCurvesBy=Line|Smooth]";
pub(super) struct FilletCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);
impl Command for FilletCommand {
    fn name(&self) -> &'static str {
        "Fillet"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments, self.0.get(), document.units())?;
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            document
                .selected_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| CommandError::FilletRequiresTwoCurves)?
        };
        let curves = ids.map(|id| {
            document
                .object(id)
                .and_then(|o| o.geometry().curve_ref())
                .ok_or(CommandError::FilletRequiresTwoCurves)
                .map(|c| c.to_owned())
        });
        let [first, second] = curves;
        let (a, b) = nearest_ends(&first?, &second?)?;
        let picks = [
            (ids[0], options.picks[0].unwrap_or(a)),
            (ids[1], options.picks[1].unwrap_or(b)),
        ];
        let prepared = prepare(document, picks, options.preferences)?;
        prepared.apply(document)?;
        self.0.set(Preferences {
            dynamic: false,
            ..options.preferences
        });
        Ok("Filleted two curves".into())
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(
            arguments,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
        )?;
        if options.sources.is_some() {
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

pub(super) fn nearest_ends(
    first: &Curve3,
    second: &Curve3,
) -> Result<(Point3, Point3), CommandError> {
    let first_ref = first.as_ref();
    let second_ref = second.as_ref();
    let first_ends = [
        first_ref.evaluate(*first_ref.domain().start())?,
        first_ref.evaluate(*first_ref.domain().end())?,
    ];
    let second_ends = [
        second_ref.evaluate(*second_ref.domain().start())?,
        second_ref.evaluate(*second_ref.domain().end())?,
    ];
    let mut best = (Real::INFINITY, first_ends[0], second_ends[0]);
    for first_end in first_ends {
        for second_end in second_ends {
            let distance = first_end.distance_to(second_end)?;
            if distance < best.0 {
                best = (distance, first_end, second_end);
            }
        }
    }
    Ok((best.1, best.2))
}

#[cfg(test)]
mod tests;
