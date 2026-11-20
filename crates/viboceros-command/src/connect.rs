//! Extends or trims selected curve ends until they meet.

use super::*;
use viboceros_geometry::{Curve3, CurveArcExtensionStyle, CurveOtherExtensionStyle};

mod interactive;
pub use crate::curve_pair_edit::Prepared;
pub use interactive::{Source, prepare, preview};

pub const USAGE: &str = "Connect [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [ExtendArcsBy=Arc|Line] [ExtendOtherCurvesBy=Line|Smooth]";

pub(super) struct ConnectCommand(pub std::sync::Arc<remembered::Remembered<Options>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub join: bool,
    pub arc_extension: CurveArcExtensionStyle,
    pub other_extension: CurveOtherExtensionStyle,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            join: false,
            arc_extension: CurveArcExtensionStyle::Arc,
            other_extension: CurveOtherExtensionStyle::Line,
        }
    }
}

impl Options {
    pub fn arguments(self) -> String {
        format!(
            "Join={} ExtendArcsBy={:?} ExtendOtherCurvesBy={:?}",
            if self.join { "Yes" } else { "No" },
            self.arc_extension,
            self.other_extension,
        )
    }
}

pub struct Arguments {
    pub picks: [Option<Point3>; 2],
    pub options: Options,
}

impl Command for ConnectCommand {
    fn name(&self) -> &'static str {
        "Connect"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let Arguments { picks, options } = parse(arguments, self.0.get())?;
        let selected = document
            .selected_objects()
            .map(|object| {
                Ok((
                    object.id(),
                    object
                        .geometry()
                        .curve_ref()
                        .ok_or(CommandError::ConnectRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::ConnectRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let curves = solve([first, second], picks, options, document.tolerance())?;
        let outputs = document.copy_object_pieces_into_source_groups(
            [*first_id, *second_id].into_iter().zip(curves),
        )?;
        document.delete_objects([*first_id, *second_id])?;
        document.select_objects_direct(outputs, SelectionMode::Replace)?;
        self.0.set(options);
        Ok("Connected two curves".to_string())
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments, self.0.get())?;
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

pub fn parse(arguments: &[&str], defaults: Options) -> Result<Arguments, CommandError> {
    let mut picks = [None, None];
    let mut join = None;
    let mut arc_extension = None;
    let mut other_extension = None;
    for argument in arguments {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let value = value.trim_start_matches('_');
        if option_name_eq(name, "ExtendOtherCurvesBy") {
            let style = if value.eq_ignore_ascii_case("Line") {
                CurveOtherExtensionStyle::Line
            } else if value.eq_ignore_ascii_case("Smooth") {
                CurveOtherExtensionStyle::Smooth
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if other_extension.replace(style).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "ExtendArcsBy") {
            let style = if value.eq_ignore_ascii_case("Arc") {
                CurveArcExtensionStyle::Arc
            } else if value.eq_ignore_ascii_case("Line") {
                CurveArcExtensionStyle::Line
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if arc_extension.replace(style).is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "Join") {
            if join
                .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                .is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        let index = if option_name_eq(name, "Pick1") {
            0
        } else if option_name_eq(name, "Pick2") {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if picks[index].is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let (pick, consumed) = parse_point(&[value])?;
        if consumed != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        picks[index] = Some(pick);
    }
    Ok(Arguments {
        picks,
        options: Options {
            join: join.unwrap_or(defaults.join),
            arc_extension: arc_extension.unwrap_or(defaults.arc_extension),
            other_extension: other_extension.unwrap_or(defaults.other_extension),
        },
    })
}

fn solve(
    curves: [&Curve3; 2],
    picks: [Point3; 2],
    options: Options,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let first = (curves[0], picks[0]);
    let second = (curves[1], picks[1]);
    if options.join {
        Ok(vec![Geometry::PolyCurve(
            super::curve_connections::connect_joined(
                first,
                second,
                options.arc_extension,
                options.other_extension,
                tolerance,
            )?,
        )])
    } else {
        Ok(super::curve_connections::connect_parts(
            first,
            second,
            options.arc_extension,
            options.other_extension,
            tolerance,
        )?
        .into_iter()
        .map(Geometry::from)
        .collect())
    }
}

#[cfg(test)]
mod tests;
