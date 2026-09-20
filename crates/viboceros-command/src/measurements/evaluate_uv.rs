//! Surface-coordinate inspection, deliberately ignoring face trims.
use super::*;
use crate::{
    BooleanSelectionOption, ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow,
    parse_point,
};
use viboceros_geometry::Point3;

const USAGE: &str = "EvaluateUVPt [Normalized=Yes|No] [CreatePoint=Yes|No] point";

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvaluateUvOptions {
    pub normalized: bool,
    pub create_point: bool,
}

impl EvaluateUvOptions {
    pub fn parse(mut self, arguments: &[&str]) -> Result<(Self, Option<Point3>), CommandError> {
        let (mut index, mut point, mut seen) = (0, None, [false; 2]);
        while index < arguments.len() {
            if let Some((key, value)) = arguments[index].split_once('=') {
                let slot = if crate::option_name_eq(key, "Normalized") {
                    0
                } else if crate::option_name_eq(key, "CreatePoint") {
                    1
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
                if seen[slot] {
                    return Err(CommandError::Usage(USAGE));
                }
                seen[slot] = true;
                let value = crate::parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                if slot == 0 {
                    self.normalized = value;
                } else {
                    self.create_point = value;
                }
                index += 1;
            } else if point.is_none() {
                let (value, consumed) = parse_point(&arguments[index..])?;
                point = Some(value);
                index += consumed;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        Ok((self, point))
    }
    pub fn command_line(self) -> String {
        format!(
            "EvaluateUVPt Normalized={} CreatePoint={}",
            if self.normalized { "Yes" } else { "No" },
            if self.create_point { "Yes" } else { "No" }
        )
    }
}

pub(crate) struct EvaluateUvCommand;
impl Command for EvaluateUvCommand {
    fn name(&self) -> &'static str {
        "EvaluateUVPt"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, point) = EvaluateUvOptions::default().parse(arguments)?;
        Ok(point.is_none().then_some(ObjectSelectionPrompt {
            command: "EvaluateUVPt",
            filter: ObjectSelectionFilter::SurfaceComponents,
            options: vec![
                BooleanSelectionOption {
                    name: "Normalized",
                    value: options.normalized,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "CreatePoint",
                    value: options.create_point,
                    aliases: &[],
                },
            ],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (options, point) = EvaluateUvOptions::default().parse(arguments)?;
        let target = point.ok_or(CommandError::Usage(USAGE))?;
        let mut selected = document.selected_objects();
        let object = selected.next().ok_or(CommandError::NoObjectsSelected)?;
        if selected.next().is_some() {
            return Err(CommandError::Usage(
                "EvaluateUVPt requires one selected surface",
            ));
        }
        drop(selected);
        let (surface, u, v) = match object.geometry() {
            Geometry::NurbsSurface(surface) => {
                let (u, v) = surface.closest_parameters(target, document.tolerance())?;
                (surface, u, v)
            }
            Geometry::Brep(brep) => {
                let (face, u, v) =
                    brep.closest_underlying_face_parameters(target, document.tolerance())?;
                (brep.faces()[face].surface(), u, v)
            }
            _ => {
                return Err(CommandError::Usage(
                    "EvaluateUVPt requires a surface or polysurface",
                ));
            }
        };
        let parameters = if options.normalized {
            surface.normalized_parameters(u, v)?
        } else {
            [u, v]
        };
        let report = format!(
            "Surface UV coordinates = {},{}{}",
            format_measurement(parameters[0]),
            format_measurement(parameters[1]),
            if options.normalized {
                " (normalized)"
            } else {
                ""
            }
        );
        let marker = options
            .create_point
            .then(|| surface.evaluate(u, v))
            .transpose()?;
        if let Some(marker) = marker {
            document.add_geometry(Geometry::Point(marker))?;
        }
        Ok(report)
    }
}
