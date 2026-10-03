//! Axis deformation and rigid placement, with atomic geometry replacement.
use super::*;
use std::collections::BTreeMap;
use viboceros_document::{CopyGroupPolicy, ObjectId, ReplacementHistory};
use viboceros_geometry::{BoundingBox3, TwistPointMorph};

pub const USAGE: &str = "Twist axis-start axis-end angle | axis-start axis-end reference target [Copy=Yes|No] [Rigid=Yes|No] [Infinite=Yes|No] [PreserveStructure=Yes|No]";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TwistOptions {
    pub copy: bool,
    pub rigid: bool,
    pub infinite: bool,
    pub preserve_structure: bool,
}

impl TwistOptions {
    pub fn update(&mut self, input: &str) -> Result<(), CommandError> {
        let tokens = input.split_whitespace().collect::<Vec<_>>();
        let (name, value) = match tokens.as_slice() {
            [token] => token.split_once('=').ok_or(CommandError::Usage(USAGE))?,
            [name, value] => (*name, *value),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        let target = if option_name_eq(name, "Copy") {
            &mut self.copy
        } else if option_name_eq(name, "Rigid") {
            &mut self.rigid
        } else if option_name_eq(name, "Infinite") {
            &mut self.infinite
        } else if option_name_eq(name, "PreserveStructure") {
            &mut self.preserve_structure
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        *target = value;
        Ok(())
    }
    pub fn command_options(self) -> String {
        let yes_no = |v| if v { "Yes" } else { "No" };
        format!(
            "Copy={} Rigid={} Infinite={} PreserveStructure={}",
            yes_no(self.copy),
            yes_no(self.rigid),
            yes_no(self.infinite),
            yes_no(self.preserve_structure)
        )
    }
    pub fn from_arguments<'a>(
        arguments: &[&'a str],
        default: Self,
    ) -> Result<(Vec<&'a str>, Self), CommandError> {
        let mut options = default;
        let mut positional = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for token in arguments {
            if let Some((name, _)) = token.split_once('=') {
                if !seen.insert(name.trim_start_matches(['_', '-']).to_ascii_lowercase()) {
                    return Err(CommandError::Usage(USAGE));
                }
                options.update(token)?;
            } else {
                positional.push(*token);
            }
        }
        Ok((positional, options))
    }
}

/// Stages every source before the document changes. Rigid groups use the
/// selected top-group peers' combined bounds; unseen peers are not moved.
pub fn deformed_geometries(
    document: &Document,
    ids: &[ObjectId],
    start: Point3,
    end: Point3,
    degrees: Real,
    options: TwistOptions,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let morph = TwistPointMorph::try_new(
        start,
        end,
        degrees.to_radians(),
        options.infinite,
        document.tolerance(),
    )?
    .with_preserve_structure(options.preserve_structure);
    let mut group_bounds = BTreeMap::new();
    if options.rigid && degrees != 0. {
        for id in ids {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            if let Some(group) = object.top_group() {
                let bounds = object.geometry().tight_bounds(document.tolerance())?;
                if let Some(b) = group_bounds.get_mut(&group) {
                    *b = BoundingBox3::union(*b, bounds)?;
                } else {
                    group_bounds.insert(group, bounds);
                }
            }
        }
    }
    let group_transforms = group_bounds
        .into_iter()
        .map(|(id, bounds)| Ok((id, morph.rigid_transform(bounds.center()?)?)))
        .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
    ids.iter()
        .map(|id| {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if degrees == 0. {
                object.geometry().clone()
            } else if options.rigid {
                let transform = if let Some(group) = object.top_group() {
                    group_transforms[&group]
                } else {
                    morph.rigid_transform(
                        object
                            .geometry()
                            .tight_bounds(document.tolerance())?
                            .center()?,
                    )?
                };
                object
                    .geometry()
                    .transformed(transform, document.tolerance())?
            } else {
                object.geometry().morphed(&morph, document.tolerance())?
            };
            Ok((*id, geometry))
        })
        .collect()
}

pub fn reference_angle(
    start: Point3,
    end: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    axis_rotation_angle(
        start,
        start.vector_to(end)?.normalized(tolerance)?,
        reference,
        target,
        tolerance,
    )
    .map(Real::to_degrees)
}

#[derive(Default)]
pub(super) struct TwistCommand(remembered::Remembered<Option<Real>>);
impl Command for TwistCommand {
    fn name(&self) -> &'static str {
        "Twist"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn scalar_default(&self) -> Option<Real> {
        self.0.get()
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, mut sources) = transform_arguments(document, arguments, USAGE)?;
        let (positional, options) =
            TwistOptions::from_arguments(&arguments, TwistOptions::default())?;
        let (start, n) = parse_point(&positional)?;
        let (end, m) = parse_point(&positional[n..])?;
        let remaining = &positional[n + m..];
        let degrees = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?
        } else {
            let (reference, n) = parse_point(remaining)?;
            let (target, m) = parse_point(&remaining[n..])?;
            require_consumed(remaining, n + m, USAGE)?;
            reference_angle(start, end, reference, target, document.tolerance())?
        };
        let staged = deformed_geometries(document, &sources.ids, start, end, degrees, options)?;
        self.0.set(Some(degrees));
        if degrees == 0. {
            return Ok(format!(
                "Twisted {} object(s) by 0 degrees",
                sources.ids.len()
            ));
        }
        if options.copy {
            sources.release_selection_on_replay();
        }
        if sources.release_on_replay {
            document.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
        }
        let count = staged.len();
        if options.copy {
            document.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
            document.select_command_results(sources.ids.iter().copied())?;
        } else {
            document.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            document.move_objects_to_end_in_order(sources.ids.iter().copied())?;
            if sources.postselected {
                document.clear_selection();
            } else {
                // A trailing replay marker lets Undo restore the recorded source
                // selection, while Redo releases it after restoring the deformation.
                document
                    .release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            }
        }
        Ok(format!(
            "Twisted {count} object(s) by {degrees:.6} degrees{}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}
#[cfg(test)]
mod tests;
