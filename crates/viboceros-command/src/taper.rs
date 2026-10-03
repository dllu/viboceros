//! Axis taper distances, construction-plane direction and atomic replacement.
use super::*;
use std::collections::BTreeMap;
use viboceros_document::{CopyGroupPolicy, ObjectId, ReplacementHistory};
use viboceros_geometry::{BoundingBox3, TaperPointMorph};

pub const USAGE: &str = "Taper axis-start axis-end start-distance end-distance [Copy=Yes|No] [Rigid=Yes|No] [Flat=Yes|No] [Infinite=Yes|No] [PreserveStructure=Yes|No]";
// Retained actual Line/Curve/Surface definitions are identical at 1e-5 and 1e-9.
const MINIMUM_FITTING_TOLERANCE: Real = 1e-5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TaperOptions {
    pub copy: bool,
    pub rigid: bool,
    pub flat: bool,
    pub infinite: bool,
    pub preserve_structure: bool,
}

impl TaperOptions {
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
        } else if option_name_eq(name, "Flat") {
            &mut self.flat
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
            "Copy={} Rigid={} Flat={} Infinite={} PreserveStructure={}",
            yes_no(self.copy),
            yes_no(self.rigid),
            yes_no(self.flat),
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaperDistance {
    Number(Real),
    Point(Point3),
}
impl TaperDistance {
    /// Complete command arguments use comma coordinates for a picked distance.
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        if input.contains(',') {
            let (point, n) = parse_point(&[input])?;
            require_consumed(&[input], n, USAGE)?;
            Ok(Self::Point(point))
        } else {
            Ok(Self::Number(parse_finite_real(input)?))
        }
    }
    pub fn command_argument(self) -> String {
        match self {
            Self::Number(v) => v.to_string(),
            Self::Point(p) => p.to_array().map(|v| v.to_string()).join(","),
        }
    }
    fn radius(self, frame: Frame3) -> Result<Real, GeometryError> {
        match self {
            Self::Number(value) => Ok(value),
            Self::Point(p) => {
                let [x, y] = frame.projected_coordinates_of(p)?;
                Ok(x.hypot(y))
            }
        }
    }
}

/// Distance picks are radial projections; Flat follows the first picked radial
/// direction. Numeric distances use the axis/cplane intersection, or cplane X
/// when the axis is normal to that plane.
pub fn point_morph(
    start: Point3,
    end: Point3,
    initial: TaperDistance,
    target: TaperDistance,
    options: TaperOptions,
    context: CommandContext,
) -> Result<TaperPointMorph, CommandError> {
    let axis = start.vector_to(end)?;
    let length = axis.length()?;
    let normal = axis.normalized_nonzero()?.as_vector();
    let radial_frame = Frame3::try_from_normal(start, normal, Tolerance::NUMERICAL_VALIDATION)?;
    let r0 = initial.radius(radial_frame)?;
    let r1 = target.radius(radial_frame)?;
    let frame = if options.flat {
        let x = match initial {
            TaperDistance::Point(p) => {
                let [x, y] = radial_frame.projected_coordinates_of(p)?;
                radial_frame.vector_at([x, y, 0.])?
            }
            TaperDistance::Number(_) => {
                let cross = normal.cross(context.construction_plane.z_axis().as_vector())?;
                if cross.length()? > Tolerance::NUMERICAL_VALIDATION.angular() {
                    cross
                } else {
                    context.construction_plane.x_axis().as_vector()
                }
            }
        };
        Frame3::try_from_directions(start, x, normal.cross(x)?, Tolerance::NUMERICAL_VALIDATION)?
    } else {
        radial_frame
    };
    Ok(TaperPointMorph::try_for_command_frame(
        frame,
        length,
        r0,
        r1,
        options.flat,
        options.infinite,
    )?
    .with_preserve_structure(options.preserve_structure))
}

/// Stage all sources before changing geometry, selection, attributes or groups.
#[allow(clippy::too_many_arguments)]
pub fn deformed_geometries(
    document: &Document,
    ids: &[ObjectId],
    start: Point3,
    end: Point3,
    initial: TaperDistance,
    target: TaperDistance,
    options: TaperOptions,
    context: CommandContext,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let morph = point_morph(start, end, initial, target, options, context)?;
    let fitting_tolerance = Tolerance::try_new(
        document
            .tolerance()
            .absolute()
            .max(MINIMUM_FITTING_TOLERANCE),
        document.tolerance().relative(),
        document.tolerance().angular(),
    )?;
    let mut group_bounds = BTreeMap::new();
    if options.rigid && !morph.is_identity() {
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
    let transforms = group_bounds
        .into_iter()
        .map(|(id, bounds)| Ok((id, morph.rigid_transform(bounds.center()?)?)))
        .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
    ids.iter()
        .map(|id| {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if morph.is_identity() {
                object.geometry().clone()
            } else if options.rigid {
                let transform = if let Some(group) = object.top_group() {
                    transforms[&group]
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
                let needs_fit = !options.preserve_structure
                    || matches!(object.geometry(), Geometry::Brep(b) if b.faces().len() > 1);
                let tolerance = if needs_fit
                    && !matches!(
                        object.geometry(),
                        Geometry::Point(_) | Geometry::PointCloud(_) | Geometry::Mesh(_)
                    ) {
                    fitting_tolerance
                } else {
                    document.tolerance()
                };
                object.geometry().morphed(&morph, tolerance)?
            };
            Ok((*id, geometry))
        })
        .collect()
}

#[derive(Default)]
pub(super) struct TaperPreferences {
    options: remembered::Remembered<TaperOptions>,
}
impl CommandRegistry {
    pub fn taper_options_default(&self) -> TaperOptions {
        TaperOptions {
            copy: self.copy_default("Taper").unwrap_or(false),
            ..self.taper_preferences.options.get()
        }
    }
    pub fn remember_taper_completion_options(&self, options: TaperOptions) {
        let mut stored = self.taper_preferences.options.get();
        stored.rigid = options.rigid;
        stored.flat = options.flat;
        stored.infinite = options.infinite;
        stored.preserve_structure = options.preserve_structure;
        self.taper_preferences.options.set(stored);
    }
}
pub(super) struct TaperCommand(pub(super) std::sync::Arc<TaperPreferences>);
impl Command for TaperCommand {
    fn name(&self) -> &'static str {
        "Taper"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (arguments, sources) = transform_arguments(document, arguments, USAGE)?;
        let (positional, options) = TaperOptions::from_arguments(&arguments, self.0.options.get())?;
        let (start, n) = parse_point(&positional)?;
        let (end, m) = parse_point(&positional[n..])?;
        require_consumed(&positional, n + m + 2, USAGE)?;
        let initial = TaperDistance::parse(positional[n + m])?;
        let target = TaperDistance::parse(positional[n + m + 1])?;
        let preserve_available = sources.ids.iter().any(|id| {
            document
                .object(*id)
                .is_some_and(|o| !matches!(o.geometry(),Geometry::Brep(b) if b.faces().len()>1))
        });
        let staged = deformed_geometries(
            document,
            &sources.ids,
            start,
            end,
            initial,
            target,
            options,
            context,
        )?;
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
            }
        }

        self.0.options.set(TaperOptions {
            copy: false,
            preserve_structure: if preserve_available {
                options.preserve_structure
            } else {
                self.0.options.get().preserve_structure
            },
            ..options
        });
        Ok(format!(
            "Tapered {count} object(s){}",
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
