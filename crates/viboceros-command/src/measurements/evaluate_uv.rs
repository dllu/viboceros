//! Surface-coordinate inspection, deliberately ignoring face trims.
use super::*;
use crate::{
    BooleanSelectionOption, ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow,
    parse_point,
};
use viboceros_geometry::Point3;

const USAGE: &str = "EvaluateUVPt [Normalized=Yes|No] [CreatePoint=Yes|No] point [Face=index]";

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvaluateUvOptions {
    pub normalized: bool,
    pub create_point: bool,
}

impl EvaluateUvOptions {
    pub fn parse(self, arguments: &[&str]) -> Result<(Self, Option<Point3>), CommandError> {
        let (options, point, face) = self.parse_with_face(arguments)?;
        if face.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok((options, point))
    }

    fn parse_with_face(
        mut self,
        arguments: &[&str],
    ) -> Result<(Self, Option<Point3>, Option<usize>), CommandError> {
        let (mut index, mut point, mut face, mut seen) = (0, None, None, [false; 2]);
        while index < arguments.len() {
            if let Some((key, value)) = arguments[index].split_once('=') {
                if crate::option_name_eq(key, "Face") && face.is_none() {
                    face = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))?,
                    );
                    index += 1;
                    continue;
                }
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
        if face.is_some() && point.is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok((self, point, face))
    }
    pub fn command_line(self) -> String {
        format!(
            "EvaluateUVPt Normalized={} CreatePoint={}",
            if self.normalized { "Yes" } else { "No" },
            if self.create_point { "Yes" } else { "No" }
        )
    }
}

#[derive(Default)]
pub(crate) struct EvaluateUvCommand {
    options: crate::remembered::Remembered<EvaluateUvOptions>,
}
impl Command for EvaluateUvCommand {
    fn name(&self) -> &'static str {
        "EvaluateUVPt"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (options, point, _) = self.options.get().parse_with_face(arguments)?;
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
    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        let (options, point) = self.options.get().parse(arguments)?;
        if point.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        self.options.set(options);
        Ok(())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (options, point, face) = self.options.get().parse_with_face(arguments)?;
        let target = point.ok_or(CommandError::Usage(USAGE))?;
        // Accepted options are application preferences, not undoable model edits.
        self.options.set(options);
        let result = evaluate_surface_uv_on_face(document, target, options, face)?;
        if let Some(marker) = result.marker {
            document.add_geometry(Geometry::Point(marker))?;
        }
        Ok(result.report)
    }
}

/// A staged UV report and optional marker, with no document mutations.
pub struct EvaluateUvResult {
    pub report: String,
    pub marker: Option<Point3>,
}

/// Shared by atomic scripted commands and the interactive multi-pick session.
pub fn evaluate_surface_uv(
    document: &Document,
    target: Point3,
    options: EvaluateUvOptions,
) -> Result<EvaluateUvResult, CommandError> {
    evaluate_surface_uv_on_face(document, target, options, None)
}

/// Evaluate one selected surface, optionally fixing the B-rep face selected in a viewport.
pub fn evaluate_surface_uv_on_face(
    document: &Document,
    target: Point3,
    options: EvaluateUvOptions,
    face_index: Option<usize>,
) -> Result<EvaluateUvResult, CommandError> {
    let mut selected = document.selected_objects();
    let object = selected.next().ok_or(CommandError::NoObjectsSelected)?;
    if selected.next().is_some() {
        return Err(CommandError::Usage(
            "EvaluateUVPt requires one selected surface",
        ));
    }
    let (surface, u, v) = match object.geometry() {
        Geometry::NurbsSurface(surface) => {
            if let Some(face) = face_index
                && face != 0
            {
                return Err(CommandError::EvaluateUvFaceIndexOutOfRange {
                    face,
                    face_count: 1,
                });
            }
            let (u, v) = surface.closest_parameters(target, document.tolerance())?;
            (surface, u, v)
        }
        Geometry::Brep(brep) => {
            let (face, u, v) = if let Some(face) = face_index {
                let selected =
                    brep.faces()
                        .get(face)
                        .ok_or(CommandError::EvaluateUvFaceIndexOutOfRange {
                            face,
                            face_count: brep.faces().len(),
                        })?;
                let (u, v) = selected
                    .surface()
                    .closest_parameters(target, document.tolerance())?;
                (face, u, v)
            } else {
                brep.closest_underlying_face_parameters(target, document.tolerance())?
            };
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
    Ok(EvaluateUvResult { report, marker })
}
