//! Stretch axis inputs, remembered factors and atomic object/grip deformation.
use super::*;
use viboceros_document::{CopyGroupPolicy, ReplacementHistory};
use viboceros_geometry::StretchPointMorph;

pub const USAGE: &str =
    "Stretch axis-start axis-end (factor|target|Length=value) [Copy=Yes|No] [Rigid=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
}
impl Options {
    pub fn accept(&mut self, word: &str) -> Result<bool, CommandError> {
        let Some((key, value)) = word.split_once('=') else {
            return Ok(false);
        };
        let target = if option_name_eq(key, "Copy") {
            &mut self.copy
        } else if option_name_eq(key, "Rigid") {
            &mut self.rigid
        } else {
            return Ok(false);
        };
        *target = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        Ok(true)
    }
    pub fn arguments(self) -> String {
        format!(
            "Copy={} Rigid={}",
            if self.copy { "Yes" } else { "No" },
            if self.rigid { "Yes" } else { "No" }
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Factor(Real),
    Length(Real),
    Point(Point3),
}
impl Target {
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        if input.contains(',') {
            let (p, n) = parse_point(&[input])?;
            require_consumed(&[input], n, USAGE)?;
            Ok(Self::Point(p))
        } else {
            Ok(Self::Factor(parse_finite_real(input)?))
        }
    }
    pub fn argument(self) -> String {
        match self {
            Self::Factor(v) => v.to_string(),
            Self::Length(v) => format!("Length={v}"),
            Self::Point(p) => p.to_array().map(|v| v.to_string()).join(","),
        }
    }
    pub fn factor(self, start: Point3, end: Point3) -> Result<Real, GeometryError> {
        let value = if let Self::Factor(value) = self {
            value
        } else {
            let morph = self.morph(start, end)?;
            morph.requested_length() / morph.length()
        };
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "stretch factor",
            });
        }
        Ok(value)
    }
    pub fn morph(self, start: Point3, end: Point3) -> Result<StretchPointMorph, GeometryError> {
        let result = match self {
            Self::Factor(factor) => {
                if !factor.is_finite() {
                    return Err(GeometryError::NonFinite {
                        context: "stretch factor",
                    });
                }
                StretchPointMorph::try_new(start, end, start.distance_to(end)? * factor)?
            }
            Self::Length(length) => StretchPointMorph::try_new(start, end, length)?,
            Self::Point(p) => StretchPointMorph::try_with_target(start, end, p)?,
        };
        Ok(if let Some(solver) = crate::morph_interpolation_solver() {
            result.with_interpolation_solver(solver)
        } else {
            result
        })
    }
}

#[derive(Clone, Copy)]
struct Preferences {
    options: Options,
    factor: Real,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            options: Options::default(),
            factor: 1.,
        }
    }
}
#[derive(Default)]
pub(super) struct StretchPreferences(remembered::Remembered<Preferences>);
impl CommandRegistry {
    pub fn stretch_options_default(&self) -> Options {
        Options {
            copy: self.copy_default("Stretch").unwrap_or(false),
            ..self.stretch_preferences.0.get().options
        }
    }
    pub fn stretch_factor_default(&self) -> Real {
        self.stretch_preferences.0.get().factor
    }
}

pub fn deformed_geometries(
    doc: &Document,
    ids: &[ObjectId],
    start: Point3,
    end: Point3,
    target: Target,
    options: Options,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let morph = target.morph(start, end)?;
    let mut group_bounds = BTreeMap::new();
    if options.rigid && !morph.is_identity() {
        for id in ids {
            let object = doc.object(*id).ok_or(DocumentError::ObjectNotFound(*id))?;
            if let Some(group) = object.top_group() {
                let bounds = object.geometry().tight_bounds(doc.tolerance())?;
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
            let object = doc.object(*id).ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if morph.is_identity() {
                object.geometry().clone()
            } else if options.rigid {
                let transform = if let Some(group) = object.top_group() {
                    transforms[&group]
                } else {
                    morph.rigid_transform(
                        object.geometry().tight_bounds(doc.tolerance())?.center()?,
                    )?
                };
                object.geometry().transformed(transform, doc.tolerance())?
            } else {
                // Rhino's SDK produces the same fits at 1e-6 and 1e-5.
                // Match that fitting floor without changing document topology
                // or the exact point/grip map.
                let t = doc.tolerance();
                // B-rep fitting divides this budget equally among components.
                let floor = if matches!(object.geometry(), Geometry::Brep(_)) {
                    4e-5
                } else {
                    1e-5
                };
                let fitting =
                    Tolerance::try_new(t.absolute().max(floor), t.relative(), t.angular())?;
                object.geometry().morphed(&morph, fitting)?
            };
            Ok((*id, geometry))
        })
        .collect()
}

pub(super) struct StretchCommand(pub(super) std::sync::Arc<StretchPreferences>);
impl Command for StretchCommand {
    fn name(&self) -> &'static str {
        "Stretch"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (args, sources) = affine_transform_arguments(doc, args, USAGE)?;
        let (positional, options, target) = parse(&args, self.0.0.get().options)?;
        let (start, n) = parse_point(&positional)?;
        let (end, target) = if target.is_none()
            && positional.get(n).is_some_and(|w| !w.contains(','))
            && positional.get(n + 1).is_some_and(|w| w.contains(','))
        {
            let target = Target::parse(positional[n])?;
            let (end, m) = parse_point(&positional[n + 1..])?;
            require_consumed(&positional, n + 1 + m, USAGE)?;
            (end, target)
        } else {
            let (end, m) = parse_point(&positional[n..])?;
            let target = if let Some(target) = target {
                require_consumed(&positional, n + m, USAGE)?;
                target
            } else {
                let tail = &positional[n + m..];
                if tail.len() == 1 {
                    Target::parse(tail[0])?
                } else {
                    let (p, k) = parse_point(tail)?;
                    require_consumed(tail, k, USAGE)?;
                    Target::Point(p)
                }
            };
            (end, target)
        };
        let morph = target.morph(start, end)?;
        let factor = target.factor(start, end)?;
        if morph.is_identity() {
            self.0.0.set(Preferences {
                options: Options {
                    copy: false,
                    rigid: false,
                },
                factor,
            });
            return Ok("Stretch left the objects unchanged".into());
        }
        let grip_owners = sources
            .grips
            .iter()
            .map(|g| g.object)
            .collect::<BTreeSet<_>>();
        let ids = sources
            .ids
            .iter()
            .copied()
            .filter(|id| !grip_owners.contains(id))
            .collect::<Vec<_>>();
        let staged = deformed_geometries(doc, &ids, start, end, target, options)?;
        if sources.release_on_replay {
            doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
        }
        let count = staged.len() + grip_owners.len();
        if !sources.grips.is_empty() {
            doc.morph_grips_with_object_geometries(
                staged,
                sources.grips.iter().copied(),
                &morph,
                options.copy,
                CopyGroupPolicy::Preserve,
            )?;
            if options.copy {
                doc.morph_copy_grip_display(sources.grips.iter().copied(), &morph)?;
            }
        } else if options.copy {
            doc.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
        } else {
            doc.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            doc.move_objects_to_end_in_order(ids)?;
        }
        if options.copy {
            doc.select_command_results(sources.ids.iter().copied())?;
        } else if sources.postselected {
            doc.clear_selection();
        }
        self.0.0.set(Preferences {
            options: Options {
                copy: false,
                rigid: false,
            },
            factor,
        });
        Ok(format!(
            "Stretched {count} object(s){}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}

fn parse<'a>(
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options, Option<Target>), CommandError> {
    let mut positional = Vec::new();
    let mut target = None;
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let word = args[i];
        if let Some((key, value)) = word.split_once('=') {
            if !seen.insert(key.trim_start_matches(['_', '-']).to_ascii_lowercase()) {
                return Err(CommandError::Usage(USAGE));
            }
            if option_name_eq(key, "Length") {
                target = Some(Target::Length(parse_finite_real(value)?));
            } else if !options.accept(word)? {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(word, "Length") {
            if !seen.insert("length".into()) {
                return Err(CommandError::Usage(USAGE));
            }
            i += 1;
            let value = *args.get(i).ok_or(CommandError::Usage(USAGE))?;
            target = Some(Target::Length(parse_finite_real(value)?));
        } else if option_name_eq(word, "Copy") || option_name_eq(word, "Rigid") {
            let key = word.trim_start_matches(['_', '-']).to_ascii_lowercase();
            if !seen.insert(key) {
                return Err(CommandError::Usage(USAGE));
            }
            i += 1;
            options.accept(&format!(
                "{word}={}",
                args.get(i).ok_or(CommandError::Usage(USAGE))?
            ))?;
        } else {
            positional.push(word);
        }
        i += 1;
    }
    Ok((positional, options, target))
}
