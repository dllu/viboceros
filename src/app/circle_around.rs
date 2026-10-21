//! Guide-backed Circle getters, preserving the source and one command owner.
use super::*;
use viboceros_command::{
    circle_around_input::{Input, Preferences},
    circle_input::CircleSizeMode as Mode,
    curve_reference::{CapturedCurve, perpendicular_frame},
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Circle,
    SubDSphere,
}
pub(super) struct Session {
    owner: Owner,
    pub(super) output: Option<viboceros_command::circle_deform_input::Output>,
    curve: Option<CapturedCurve>,
    pub(super) input: Option<Input>,
    postselected: bool,
    preferences: Preferences,
    mode: Mode,
}
impl Session {
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.curve
            .as_ref()
            .filter(|_| self.input.is_none())
            .map(CapturedCurve::curve)
    }
}
impl VibocerosApp {
    pub(super) fn picking_circle_around_curve(&self) -> bool {
        self.circle_around_session
            .as_ref()
            .is_some_and(|s| s.curve.is_none())
    }
    pub(super) fn release_circle_around_picks(&mut self) {
        if let Some(s) = &self.circle_around_session
            && s.postselected
            && let Some(c) = &s.curve
            && self.document.is_selected(c.object_id())
        {
            let _ = self
                .document
                .select_object(c.object_id(), SelectionMode::Remove);
        }
    }
    pub(super) fn start_circle_around(&mut self) {
        self.start_circle_around_with_output(None);
    }
    pub(super) fn start_circle_around_with_output(
        &mut self,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        self.start_circle_around_for(Owner::Circle, output);
    }
    pub(super) fn start_subd_sphere_around(&mut self) {
        self.start_circle_around_for(Owner::SubDSphere, None);
    }
    fn start_circle_around_for(
        &mut self,
        owner: Owner,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        if owner == Owner::Circle {
            self.cancel_interactive_command(false);
        }
        let mut preferences = if owner == Owner::Circle {
            self.commands.circle_preferences()
        } else {
            Preferences::default()
        };
        if let Some(output) = output {
            preferences.deformation = output.deformation;
        }
        self.circle_around_session = Some(Session {
            owner,
            output,
            curve: None,
            input: None,
            postselected: false,
            preferences,
            mode: preferences.mode(),
        });
        let selected = self.document.selected_object_ids().collect::<Vec<_>>();
        if let [id] = selected.as_slice()
            && self
                .document
                .object(*id)
                .is_some_and(|o| o.geometry().curve_ref().is_some())
        {
            self.pick_circle_around_curve(Some(*id), None);
        }
        self.circle_around_prompt();
    }
    pub(super) fn try_start_circle_around(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if !matches!(words.as_slice(),[command,mode]if ["Circle","C"].iter().any(|n|command.trim_start_matches(['_','-']).eq_ignore_ascii_case(n))&&mode.trim_start_matches(['_','-']).eq_ignore_ascii_case("AroundCurve"))
        {
            return false;
        }
        self.start_circle_around();
        true
    }
    fn circle_around_prompt(&mut self) {
        let Some(s) = &self.circle_around_session else {
            return;
        };
        let center = s.input.map(Input::center);
        let prompt = if s.curve.is_none() {
            "select curve or surface edge".into()
        } else if let Some(input) = s.input {
            let default = match s.mode {
                Mode::Radius => format!(" <{}>", s.preferences.radius),
                Mode::Diameter => format!(" <{}>", 2. * s.preferences.radius),
                _ => String::new(),
            };
            format!(
                "{}{}; Radius, Diameter, Circumference, Area",
                input.size_mode().label(),
                default
            )
        } else {
            "center on curve".into()
        };
        self.active_command = Some(if s.owner == Owner::Circle {
            InteractiveCommand::CircleAround { center }
        } else {
            InteractiveCommand::SubDSphere { anchor: center }
        });
        self.drafting_plane = s.input.map(Input::frame);
        let name = if s.owner == Owner::Circle {
            "Circle"
        } else {
            "SubDSphere"
        };
        self.push_log(format!("{name} AroundCurve: {prompt}; Esc cancels"));
    }
    pub(super) fn pick_circle_around_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_circle_around_curve() {
            return false;
        }
        if !self.validate_circle_around_curve() {
            return true;
        }
        let result = id
            .ok_or_else(|| "Select a curve or surface edge".to_owned())
            .and_then(|id| {
                CapturedCurve::capture(&self.document, id, edge).map_err(|e| e.to_string())
            });
        match result {
            Ok(curve) => {
                let id = curve.object_id();
                let s = self.circle_around_session.as_mut().unwrap();
                s.postselected = edge.is_none() && !self.document.is_selected(id);
                s.curve = Some(curve);
                if edge.is_none() {
                    self.document
                        .select_object(id, SelectionMode::Add)
                        .expect("selectable Circle guide");
                }
                self.component_selection.clear_choices();
                self.circle_around_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(super) fn validate_circle_around_curve(&mut self) -> bool {
        if self
            .circle_around_session
            .as_ref()
            .is_some_and(|s| s.owner == Owner::SubDSphere)
            && !self.valid_subd_sphere()
        {
            return false;
        }

        if self
            .circle_around_session
            .as_ref()
            .and_then(|s| s.curve.as_ref())
            .is_some_and(|c| !c.current(&self.document))
        {
            self.release_circle_around_picks();
            let s = self.circle_around_session.as_mut().unwrap();
            s.curve = None;
            s.input = None;
            s.postselected = false;
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log("Circle guide changed or became unavailable; select it again".into());
            self.circle_around_prompt();
        }
        true
    }
    pub(super) fn resolve_circle_around_point(
        &mut self,
        point: Point3,
    ) -> Result<Point3, viboceros_geometry::GeometryError> {
        self.validate_circle_around_curve();
        let Some(s) = &self.circle_around_session else {
            return Ok(point);
        };
        if let Some(input) = s.input {
            return input.resolve_point(point);
        }
        s.curve.as_ref().map_or(Ok(point), |c| {
            perpendicular_frame(c.curve_ref(), point).map(|f| f.origin())
        })
    }
    pub(super) fn accept_circle_around_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .circle_around_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(p) = point {
            self.accept_filtered_drafting_point(p, false);
            true
        } else {
            false
        }
    }
    fn commit_circle_around(&mut self, circle: viboceros_geometry::Circle3) -> bool {
        let s = self.circle_around_session.as_ref().unwrap();
        let curve = s.curve.as_ref().unwrap();
        if s.owner == Owner::SubDSphere {
            let release = if s.postselected {
                vec![curve.object_id()]
            } else {
                vec![]
            };
            return match viboceros_command::circle_input::CircleDefinition::from_circle(circle) {
                Ok(d) => self.accept_subd_sphere_reference(d, release),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            };
        }
        let command = s
            .input
            .unwrap()
            .command(circle, curve.object_id(), curve.source().1, s.postselected)
            .expect("complete Circle AroundCurve");
        let command = s
            .output
            .map_or(command.clone(), |output| output.wrap_command(&command));
        self.try_execute_command(&command)
    }
    pub(super) fn accept_circle_around_point(&mut self, point: Point3) -> Option<bool> {
        self.circle_around_session.as_ref()?;
        if !self.validate_circle_around_curve() {
            return Some(false);
        }
        if self.picking_circle_around_curve() {
            self.circle_around_prompt();
            return Some(false);
        }
        let s = self.circle_around_session.as_mut().unwrap();
        if let Some(input) = &mut s.input {
            Some(match input.point(point) {
                Ok(circle) => self.commit_circle_around(circle),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            })
        } else {
            Some(
                match perpendicular_frame(s.curve.as_ref().unwrap().curve_ref(), point) {
                    Ok(frame) => {
                        let mut input = Input::new(frame, s.preferences, self.document.tolerance());
                        input.option(s.mode.label());
                        s.input = Some(input);
                        self.circle_around_prompt();
                        true
                    }
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        false
                    }
                },
            )
        }
    }
    pub(super) fn try_continue_circle_around(&mut self, text: &str) -> bool {
        if self.circle_around_session.is_none() {
            return false;
        }
        if !self.validate_circle_around_curve() {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let s = self.circle_around_session.as_mut().unwrap();
            if let Some(input) = &mut s.input
                && matches!(input.size_mode(), Mode::Radius | Mode::Diameter)
            {
                match input.default_size(s.preferences) {
                    Ok(circle) => {
                        self.commit_circle_around(circle);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        if self.picking_circle_around_curve() {
            let words = word.split_whitespace().collect::<Vec<_>>();
            let reference = match words.as_slice() {
                [key, id] if key.eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| (id, None))
                }
                [one] => one.parse().ok().map(|id| (id, None)).or_else(|| {
                    let (k, v) = one.split_once('=')?;
                    if k.eq_ignore_ascii_case("Curve") {
                        v.parse().ok().map(|id| (id, None))
                    } else if k.eq_ignore_ascii_case("Edge") {
                        viboceros_command::curve_reference::parse_edge(v)
                            .map(|(id, i)| (id, Some(i)))
                    } else {
                        None
                    }
                }),
                _ => None,
            };
            if let Some((id, edge)) = reference {
                self.pick_circle_around_curve(Some(id), edge);
                self.command_input.clear();
                return true;
            }
            if self
                .commands
                .recognizes(word.split_whitespace().next().unwrap_or(""))
            {
                return false;
            }
            self.circle_around_prompt();
            self.command_input.clear();
            return true;
        }
        let s = self.circle_around_session.as_mut().unwrap();
        if let Some(input) = &mut s.input {
            if word.eq_ignore_ascii_case("Orientation") {
                self.push_log("Error: Orientation is unavailable in Circle AroundCurve".into());
                self.command_input.clear();
                return true;
            }
            let (key, value) = word
                .split_once('=')
                .map_or((word, None), |(k, v)| (k, Some(v)));
            let size = Mode::parse(key);
            if let Some(mode) = size {
                input.option(key);
                s.mode = mode;
                if matches!(mode, Mode::Radius | Mode::Diameter) {
                    s.preferences.diameter = mode == Mode::Diameter;
                }
                self.point_filter = None;
                self.point_constraint = None;
                if value.is_none() {
                    self.circle_around_prompt();
                    self.command_input.clear();
                    return true;
                }
            }
            if size.is_none()
                && self
                    .commands
                    .recognizes(word.split_whitespace().next().unwrap_or(""))
            {
                return false;
            }
            let candidate = value.unwrap_or(text);
            // Area values are scalars in squared document units. Reject
            // physical length suffixes instead of converting them as lengths.
            let scalar_units = viboceros_geometry::LengthUnitSystem::Unset;
            let units = if input.size_mode() == Mode::Area {
                &scalar_units
            } else {
                self.document.units()
            };
            if let Some(value) =
                viboceros_drafting::PointInput::parse_length_with_units(candidate, units)
            {
                match value
                    .map_err(|e| e.to_string())
                    .and_then(|v| input.number(v).map_err(|e| e.to_string()))
                {
                    Ok(circle) => {
                        self.commit_circle_around(circle);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                return true;
            }
            if size.is_some() {
                self.push_log("Error: circle size requires a finite number".into());
                self.command_input.clear();
                return true;
            }
        }
        false
    }
}
