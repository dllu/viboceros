//! Circular spine deformation with atomic geometry replacement.

use super::*;
use std::collections::BTreeMap;
use viboceros_document::{CopyGroupPolicy, ObjectId, ReplacementHistory};
use viboceros_geometry::{BendPointMorph, BoundingBox3};

pub const USAGE: &str = "Bend spine-start spine-end through-point [Copy=Yes|No] [Rigid=Yes|No] [LimitToSpine=Yes|No] [Symmetric=Yes|No] [PreserveStructure=Yes|No] [NonAttenuated=Yes|No] [Angle=nonnegative-degrees]";

// The actual Line/Curve/Surface/Box tolerance sweep repeats the same native
// fitting definitions at 1e-5 and 1e-9. The kernel retains its caller tolerance.
const MINIMUM_FITTING_TOLERANCE: Real = 1e-5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BendOptions {
    pub copy: bool,
    pub rigid: bool,
    pub limit_to_spine: bool,
    pub symmetric: bool,
    pub preserve_structure: bool,
    pub non_attenuated: bool,
    /// Degrees. None uses the remembered command angle; zero resets it and
    /// selects through-point construction. A fresh registry has no saved angle.
    pub angle: Option<Real>,
}

impl BendOptions {
    pub fn update(&mut self, input: &str) -> Result<(), CommandError> {
        let tokens = input.split_whitespace().collect::<Vec<_>>();
        let (name, value) = match tokens.as_slice() {
            [token] => token.split_once('=').ok_or(CommandError::Usage(USAGE))?,
            [name, value] => (*name, *value),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if option_name_eq(name, "Angle") {
            let value = parse_finite_real(value)?;
            if !(0. ..=360.).contains(&value) {
                return Err(CommandError::Usage(USAGE));
            }
            self.angle = Some(value);
            return Ok(());
        }
        let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        let target = if option_name_eq(name, "Copy") {
            &mut self.copy
        } else if option_name_eq(name, "Rigid") {
            &mut self.rigid
        } else if option_name_eq(name, "LimitToSpine") {
            &mut self.limit_to_spine
        } else if option_name_eq(name, "Symmetric") {
            &mut self.symmetric
        } else if option_name_eq(name, "PreserveStructure") {
            &mut self.preserve_structure
        } else if option_name_eq(name, "NonAttenuated") {
            &mut self.non_attenuated
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        *target = value;
        Ok(())
    }

    pub fn command_options(self) -> String {
        let yes_no = |v| if v { "Yes" } else { "No" };
        let mut text = format!(
            "Copy={} Rigid={} LimitToSpine={} Symmetric={} PreserveStructure={} NonAttenuated={}",
            yes_no(self.copy),
            yes_no(self.rigid),
            yes_no(self.limit_to_spine),
            yes_no(self.symmetric),
            yes_no(self.preserve_structure),
            yes_no(self.non_attenuated),
        );
        if let Some(angle) = self.angle {
            text.push_str(&format!(" Angle={angle}"));
        }
        text
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

/// Stage all sources before changing the document. Selected top-group peers
/// share one rigid pose about their combined bounds center.
pub fn deformed_geometries(
    document: &Document,
    ids: &[ObjectId],
    start: Point3,
    end: Point3,
    through: Point3,
    options: BendOptions,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let morph = BendPointMorph::try_for_command(
        start,
        end,
        through,
        options.angle.map(Real::to_radians),
        options.limit_to_spine,
        options.symmetric,
        document.tolerance(),
    )?
    .with_preserve_structure(options.preserve_structure)
    .with_non_attenuated(options.non_attenuated);
    let fitting_tolerance = Tolerance::try_new(
        document
            .tolerance()
            .absolute()
            .max(MINIMUM_FITTING_TOLERANCE),
        document.tolerance().relative(),
        document.tolerance().angular(),
    )?;
    let mut group_bounds = BTreeMap::new();
    if options.rigid {
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
            let geometry = if options.rigid {
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
pub(super) struct BendPreferences {
    options: remembered::Remembered<BendOptions>,
    scalar: remembered::Remembered<Option<Real>>,
}

impl CommandRegistry {
    pub fn bend_options_default(&self) -> BendOptions {
        BendOptions {
            copy: self.copy_default("Bend").unwrap_or(false),
            angle: None,
            ..self.bend_preferences.options.get()
        }
    }

    /// Native spine and attenuation choices are remembered on editing, even
    /// when the final through point is canceled. Other flags wait for completion.
    pub fn remember_bend_prompt_option(&self, options: BendOptions, name: &str) {
        let mut stored = self.bend_preferences.options.get();
        if option_name_eq(name, "LimitToSpine") {
            stored.limit_to_spine = options.limit_to_spine;
        } else if option_name_eq(name, "Symmetric") {
            stored.symmetric = options.symmetric;
        } else if option_name_eq(name, "NonAttenuated") {
            stored.non_attenuated = options.non_attenuated;
        } else if option_name_eq(name, "Angle")
            && let Some(angle) = options.angle
            && angle.is_finite()
            && (0. ..=360.).contains(&angle)
        {
            self.bend_preferences
                .scalar
                .set((angle != 0.).then_some(angle));
        }
        self.bend_preferences.options.set(stored);
    }

    /// Interactive Copy placements can survive an outer cancellation. The
    /// caller commits these two choices only when the outer command succeeds.
    pub fn remember_bend_completion_options(&self, options: BendOptions) {
        let mut stored = self.bend_preferences.options.get();
        stored.rigid = options.rigid;
        stored.preserve_structure = options.preserve_structure;
        self.bend_preferences.options.set(stored);
    }
}

pub(super) struct BendCommand(pub(super) std::sync::Arc<BendPreferences>);
impl Command for BendCommand {
    fn name(&self) -> &'static str {
        "Bend"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn scalar_default(&self) -> Option<Real> {
        self.0.scalar.get()
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (arguments, mut sources) = transform_arguments(document, arguments, USAGE)?;
        let default = BendOptions {
            angle: None,
            ..self.0.options.get()
        };
        let (positional, options) = BendOptions::from_arguments(&arguments, default)?;
        let (start, n) = parse_point(&positional)?;
        let (end, m) = parse_point(&positional[n..])?;
        let (through, k) = parse_point(&positional[n + m..])?;
        require_consumed(&positional, n + m + k, USAGE)?;
        let preserve_available = sources.ids.iter().any(|id| {
            document.object(*id).is_some_and(|object| {
                !matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() > 1)
            })
        });
        let effective = BendOptions {
            angle: options.angle.or(self.0.scalar.get()),
            ..options
        };
        let staged = deformed_geometries(document, &sources.ids, start, end, through, effective)?;
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
                document
                    .release_transform_selection_on_history_replay(sources.ids.iter().copied())?;
            }
        }
        self.0.options.set(BendOptions {
            copy: false,
            angle: None,
            preserve_structure: if preserve_available {
                options.preserve_structure
            } else {
                self.0.options.get().preserve_structure
            },
            ..options
        });
        if let Some(angle) = options.angle {
            self.0.scalar.set((angle != 0.).then_some(angle));
        }
        Ok(format!(
            "Bent {count} object(s){}",
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
