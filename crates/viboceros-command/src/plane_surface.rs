//! Rectangular planar NURBS creation; rectangle input owns construction modes.
use crate::rectangle_input::{RectangleDefinition, RectangleInput, RectangleScript};
use crate::{Command, CommandContext, CommandError, CommandRegistry, option_name_eq, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, MAX_PLANE_CONTROL_POINTS, NurbsSurface, Tolerance};
mod options;
pub use options::{Options, Preferences};

pub const USAGE: &str = "Plane [Deformable UDegree=n VDegree=n UPointCount=n VPointCount=n] [Center|3Point|EdgeMidpoint|Vertical|AroundCurve] rectangle-points-or-dimensions";

pub fn geometry(base: RectangleDefinition, options: Options) -> Result<Geometry, CommandError> {
    options.preferences.validate()?;
    if base.rounding.is_some() || options.pending() {
        return Err(CommandError::Usage(USAGE));
    }
    let [a, b, c] = base.plane_points();
    let x = a.vector_to(b)?;
    let y = a.vector_to(c)?;
    let frame = Frame3::try_from_directions(a, x, y, Tolerance::NUMERICAL_VALIDATION)?;
    let degrees = if options.deformable {
        options.preferences.degrees
    } else {
        [1; 2]
    };
    let counts = if options.deformable {
        options.preferences.counts
    } else {
        [2; 2]
    };
    Ok(Geometry::NurbsSurface(NurbsSurface::try_rectangular_plane(
        frame,
        [x.length()?, y.length()?],
        degrees,
        counts,
    )?))
}

pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    frame: Frame3,
    preferences: Preferences,
) -> Result<(RectangleDefinition, Options, bool), CommandError> {
    let mut script = RectangleScript::new(RectangleInput::new(frame).with_height_line_constraint());
    let mut options = Options::new(preferences);
    let mut result = None;
    let mut release_selection = false;
    let mut i = 0;
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if !script.has_curve()
            && (options.pending() || script.input.anchor().is_none())
            && options.option(args[i])?
        {
            i += 1;
            continue;
        }
        if script.input.option(args[i]) {
            i += 1;
            continue;
        }
        if script.input.requests_curve() && !script.has_curve() && !args[i].contains('=') {
            let selected = doc
                .selected_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .map(|o| o.id())
                .take(2)
                .collect::<Vec<_>>();
            if let [id] = selected.as_slice() {
                script.value(doc, &[&format!("Curve={id}")], false)?;
                continue;
            }
        }
        let picking_guide = script.input.requests_curve() && !script.has_curve();
        let (n, base) = script.value(doc, &args[i..], false).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            e => e,
        })?;
        if picking_guide {
            // The shared script has already validated this reference.
            let (kind, value) = args[i].split_once('=').unwrap();
            let id = value.split(',').next().unwrap().parse().unwrap();
            release_selection = kind.eq_ignore_ascii_case("Edge") || !doc.is_selected(id);
        }
        i += n;
        result = base;
    }
    Ok((
        result.ok_or(CommandError::Usage(USAGE))?,
        options,
        release_selection,
    ))
}

#[derive(Default)]
pub(super) struct PlaneCommand(remembered::Remembered<Preferences>);
impl Command for PlaneCommand {
    fn name(&self) -> &'static str {
        "Plane"
    }
    fn plane_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_plane_preferences(&self, p: Preferences) -> bool {
        self.0.set(p);
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
        let (base, options, release_selection) =
            from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let geometry = geometry(base, options)?;
        if release_selection {
            doc.release_command_selection_on_history_replay(
                doc.selected_object_ids().collect::<Vec<_>>(),
            )?;
            doc.clear_selection();
        }
        let id = doc.add_geometry(geometry)?;
        self.0.set(options.preferences);
        Ok(format!("Added plane {id}"))
    }
}
impl CommandRegistry {
    pub fn plane_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("Plane"))
            .and_then(|i| self.commands[*i].plane_preferences())
            .unwrap_or_default()
    }
    pub fn remember_plane_preferences(&self, p: Preferences) -> bool {
        p.validate().is_ok()
            && self
                .lookup
                .get(&crate::normalize_command_name("Plane"))
                .is_some_and(|i| self.commands[*i].remember_plane_preferences(p))
    }
}
