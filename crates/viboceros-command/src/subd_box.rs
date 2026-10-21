//! SubD box construction shares the edit-free rectangle getter.
use crate::rectangle_input::{RectangleDefinition, RectangleInput};
use crate::{Command, CommandContext, CommandError, CommandRegistry, parse_point, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, GeometryError, Point3, SubD, Tolerance};

pub const USAGE: &str = "SubDBox [XCount=n YCount=n ZCount=n] [Center|3Point|Vertical|AroundCurve] base-points height-or-point";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub counts: [usize; 3],
}
impl Default for Preferences {
    fn default() -> Self {
        Self { counts: [2; 3] }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        let [x, y, z] = self.counts;
        if x == 0
            || y == 0
            || z == 0
            || x.checked_mul(y)
                .and_then(|xy| x.checked_mul(z).and_then(|xz| xy.checked_add(xz)))
                .and_then(|s| y.checked_mul(z).and_then(|yz| s.checked_add(yz)))
                .and_then(|s| s.checked_mul(2))
                .and_then(|s| s.checked_add(2))
                .is_none_or(|n| n > 100_000)
        {
            return Err(CommandError::Usage(
                "SubDBox needs positive counts and at most 100000 control vertices",
            ));
        }
        Ok(())
    }
    /// One leading face-count option, including the native uniform-count spelling.
    /// A leading zero is a point at the CPlane origin and is left for the caller.
    pub fn option(&mut self, args: &[&str]) -> Result<Option<usize>, CommandError> {
        let Some(word) = args.first() else {
            return Ok(None);
        };
        let word = word.trim_start_matches('_');
        let (name, value, consumed) = if let Some((key, value)) = word.split_once('=') {
            (key, value, 1)
        } else if ["XCount", "YCount", "ZCount"]
            .iter()
            .any(|n| word.eq_ignore_ascii_case(n))
        {
            (word, *args.get(1).ok_or(CommandError::Usage(USAGE))?, 2)
        } else if let Ok(count) = word.parse::<usize>() {
            if count == 0 {
                return Ok(None);
            }
            let candidate = Self { counts: [count; 3] };
            candidate.validate()?;
            *self = candidate;
            return Ok(Some(1));
        } else {
            return Ok(None);
        };
        let Some(axis) = ["XCount", "YCount", "ZCount"]
            .iter()
            .position(|n| name.eq_ignore_ascii_case(n))
        else {
            return Ok(None);
        };
        let count = value
            .trim_start_matches('_')
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        if count == 0 || count > 100_000 {
            return Err(CommandError::Usage(USAGE));
        }
        self.counts[axis] = count;
        Ok(Some(consumed))
    }
}

pub fn rectangle_option(input: &mut RectangleInput, word: &str) -> bool {
    let name = word.trim_start_matches(['_', '-']);
    ["3Point", "Vertical", "Center", "AroundCurve"]
        .iter()
        .any(|s| name.eq_ignore_ascii_case(s))
        && input.option(word)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub base: RectangleDefinition,
    pub height: f64,
}
impl Definition {
    pub fn plane(self) -> Result<Frame3, GeometryError> {
        base_plane(self.base)
    }
    pub fn corners(self) -> Result<[Point3; 8], GeometryError> {
        if !self.height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD box height",
            });
        }
        if self.height == 0. {
            return Err(GeometryError::Degenerate {
                context: "SubD box height",
            });
        }
        let offset = self.plane()?.z_axis().as_vector().scaled(self.height)?;
        let mut result = [self.base.corners[0]; 8];
        for i in 0..4 {
            let shifted = self.base.corners[i].translated(offset)?;
            if self.height > 0. {
                result[i] = self.base.corners[i];
                result[i + 4] = shifted;
            } else {
                result[i] = shifted;
                result[i + 4] = self.base.corners[i];
            }
        }
        Ok(result)
    }
    pub fn geometry(self, preferences: Preferences) -> Result<Geometry, CommandError> {
        preferences.validate()?;
        let frame = self.plane()?.with_origin(self.corners()?[0]);
        let dimensions = [
            self.base.corners[0].distance_to(self.base.corners[1])?,
            self.base.corners[0].distance_to(self.base.corners[3])?,
            self.height.abs(),
        ];
        Ok(Geometry::SubD(SubD::try_box(
            frame,
            dimensions,
            preferences.counts,
        )?))
    }
}
pub fn base_plane(base: RectangleDefinition) -> Result<Frame3, GeometryError> {
    Frame3::try_from_directions(
        base.corners[0],
        base.corners[0].vector_to(base.corners[1])?,
        base.corners[0].vector_to(base.corners[3])?,
        Tolerance::NUMERICAL_VALIDATION,
    )
}
pub fn height_at(base: RectangleDefinition, point: Point3) -> Result<f64, GeometryError> {
    Ok(base_plane(base)?.coordinates_of(point)?[2])
}
pub fn default_height(base: RectangleDefinition) -> Result<f64, GeometryError> {
    base.corners[0].distance_to(base.corners[3])
}

pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    mut preferences: Preferences,
) -> Result<(Definition, Preferences), CommandError> {
    let mut script = crate::rectangle_input::RectangleScript::new(RectangleInput::new(plane));
    let mut base = None;
    let mut definition = None;
    let mut i = 0;
    while i < args.len() {
        if definition.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i];
        if base.is_none() && !script.has_curve() {
            if script.input.anchor().is_none()
                && let Some(n) = preferences.option(&args[i..])?
            {
                i += n;
                continue;
            }
            if rectangle_option(&mut script.input, word) {
                i += 1;
                continue;
            }
        }
        if let Some(base) = base {
            let height = if word.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
                i += 1;
                default_height(base)?
            } else if word.contains(',') {
                let (p, n) = parse_point(&args[i..])?;
                i += n;
                height_at(base, p)?
            } else {
                i += 1;
                crate::parse_finite_real(word)?
            };
            let value = Definition { base, height };
            value.corners()?;
            definition = Some(value);
            continue;
        }
        let (n, rectangle) = script.value(doc, &args[i..], false).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            e => e,
        })?;
        i += n;
        base = rectangle;
    }
    Ok((definition.ok_or(CommandError::Usage(USAGE))?, preferences))
}

#[derive(Default)]
pub(super) struct SubDBoxCommand(remembered::Remembered<Preferences>);
impl Command for SubDBoxCommand {
    fn name(&self) -> &'static str {
        "SubDBox"
    }
    fn subd_box_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_box_preferences(&self, value: Preferences) -> bool {
        self.0.set(value);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (definition, preferences) =
            from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let id = doc.add_geometry(definition.geometry(preferences)?)?;
        self.0.set(preferences);
        Ok(format!("Added SubD box {id}"))
    }
}
impl CommandRegistry {
    pub fn subd_box_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDBox"))
            .and_then(|i| self.commands[*i].subd_box_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_box_preferences(&self, value: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDBox"))
            .is_some_and(|i| self.commands[*i].remember_subd_box_preferences(value))
    }
}
