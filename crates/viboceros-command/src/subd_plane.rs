//! Rectangular subdivision grids reuse rectangle admission and mesh grid sampling.
use crate::rectangle_input::{RectangleDefinition, RectangleInput, RectangleScript};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, SubD};
pub const USAGE: &str =
    "SubDPlane [XCount=n YCount=n] [Center|3Point|Vertical|AroundCurve] rectangle-points";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub counts: [usize; 2],
}
impl Default for Preferences {
    fn default() -> Self {
        Self { counts: [2; 2] }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        let [x, y] = self.counts;
        if x == 0
            || y == 0
            || x.checked_add(1)
                .and_then(|x| y.checked_add(1).and_then(|y| x.checked_mul(y)))
                .is_none_or(|n| n > 100_000)
        {
            return Err(CommandError::Usage(
                "SubDPlane needs positive counts and at most 100000 control vertices",
            ));
        }
        Ok(())
    }
    pub fn option(&mut self, args: &[&str]) -> Result<Option<usize>, CommandError> {
        let Some(word) = args.first() else {
            return Ok(None);
        };
        let word = word.trim_start_matches('_');
        if let Ok(count) = word.parse::<usize>() {
            if count == 0 {
                return Ok(None);
            }
            let next = Self { counts: [count; 2] };
            next.validate()?;
            *self = next;
            return Ok(Some(1));
        }
        if word
            .split('=')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("ZCount"))
        {
            return Ok(None);
        }
        let mut box_options = crate::subd_box::Preferences {
            counts: [self.counts[0], self.counts[1], 1],
        };
        let result = box_options.option(args).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            e => e,
        })?;
        self.counts = [box_options.counts[0], box_options.counts[1]];
        Ok(result)
    }
}
pub fn geometry(
    base: RectangleDefinition,
    preferences: Preferences,
) -> Result<Geometry, CommandError> {
    preferences.validate()?;
    let frame = crate::subd_box::base_plane(base)?;
    let dimensions = [
        base.corners[0].distance_to(base.corners[1])?,
        base.corners[0].distance_to(base.corners[3])?,
    ];
    Ok(Geometry::SubD(SubD::try_plane_grid(
        frame,
        dimensions,
        preferences.counts,
    )?))
}
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    mut preferences: Preferences,
) -> Result<(RectangleDefinition, Preferences), CommandError> {
    let mut script = RectangleScript::new(RectangleInput::new(plane));
    let mut result = None;
    let mut i = 0;
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if !script.has_curve() {
            if script.input.anchor().is_none()
                && let Some(n) = preferences.option(&args[i..])?
            {
                i += n;
                continue;
            }
            if crate::subd_box::rectangle_option(&mut script.input, args[i]) {
                i += 1;
                continue;
            }
        }
        let (n, rectangle) = script.value(doc, &args[i..], false).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            e => e,
        })?;
        i += n;
        result = rectangle;
    }
    Ok((result.ok_or(CommandError::Usage(USAGE))?, preferences))
}
#[derive(Default)]
pub(super) struct SubDPlaneCommand(remembered::Remembered<Preferences>);
impl Command for SubDPlaneCommand {
    fn name(&self) -> &'static str {
        "SubDPlane"
    }
    fn subd_plane_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_plane_preferences(&self, value: Preferences) -> bool {
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
        let (base, preferences) =
            from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let id = doc.add_geometry(geometry(base, preferences)?)?;
        self.0.set(preferences);
        Ok(format!("Added SubD plane {id}"))
    }
}
impl CommandRegistry {
    pub fn subd_plane_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDPlane"))
            .and_then(|i| self.commands[*i].subd_plane_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_plane_preferences(&self, value: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDPlane"))
            .is_some_and(|i| self.commands[*i].remember_subd_plane_preferences(value))
    }
}
