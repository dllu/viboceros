//! Guide-backed Circle getters, preserving the source and one command owner.
use super::*;
use viboceros_command::{
    circle_around_input::{Input, Preferences},
    circle_input::CircleSizeMode as Mode,
    curve_reference::{CapturedCurve, perpendicular_frame},
    solid_axial::Kind as AxialKind,
    torus_input::Kind as TorusKind,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Circle,
    Sphere(sphere::Output),
    SubDSphere,
    Torus(TorusKind),
    SolidAxial(AxialKind),
    Pyramid,
}
impl Owner {
    fn name(self) -> &'static str {
        match self {
            Self::Circle => "Circle",
            Self::Sphere(output) => output.name(),
            Self::SubDSphere => "SubDSphere",
            Self::Torus(kind) => kind.name(),
            Self::SolidAxial(kind) => kind.name(),
            Self::Pyramid => "Pyramid",
        }
    }
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
    pub(super) fn sphere(&self) -> bool {
        matches!(self.owner, Owner::Sphere(_))
    }

    pub(super) fn snap_projection_plane(&self) -> Option<Frame3> {
        let input = self.input?;
        let projects_snaps = match self.owner {
            Owner::Circle | Owner::SolidAxial(_) => true,
            Owner::Pyramid => false,
            Owner::Torus(kind) => kind.projects_base_snaps(),
            _ => false,
        };
        (projects_snaps && matches!(input.size_mode(), Mode::Radius | Mode::Diameter))
            .then(|| input.frame())
    }

    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.curve
            .as_ref()
            .filter(|_| self.input.is_none())
            .map(CapturedCurve::curve)
    }

    pub(super) fn postselected_object(&self) -> Option<ObjectId> {
        self.curve
            .as_ref()
            .filter(|_| self.postselected)
            .map(CapturedCurve::object_id)
    }
}
impl VibocerosApp {
    pub(super) fn picking_circle_around_curve(&self) -> bool {
        self.circle_around_session
            .as_ref()
            .is_some_and(|s| s.curve.is_none())
    }
    pub(super) fn release_circle_around_picks(&mut self) {
        if let Some(id) = self
            .circle_around_session
            .as_ref()
            .and_then(Session::postselected_object)
            && self.document.is_selected(id)
        {
            let _ = self.document.select_object(id, SelectionMode::Remove);
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
    pub(super) fn start_sphere_around(&mut self, output: sphere::Output) {
        self.start_circle_around_for(Owner::Sphere(output), None);
    }
    pub(super) fn start_torus_around(&mut self, kind: TorusKind) {
        self.start_circle_around_for(Owner::Torus(kind), None);
    }
    pub(super) fn start_solid_axial_around(&mut self, kind: AxialKind) {
        self.start_circle_around_for(Owner::SolidAxial(kind), None);
    }

    pub(super) fn start_pyramid_around(&mut self) {
        self.start_circle_around_for(Owner::Pyramid, None);
    }

    fn start_circle_around_for(
        &mut self,
        owner: Owner,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        if matches!(owner, Owner::Circle | Owner::Sphere(_)) {
            self.cancel_interactive_command(false);
        }
        let mut preferences = if owner == Owner::Circle {
            self.commands.circle_preferences()
        } else if let Owner::Sphere(output) = owner {
            output.size_preferences(&self.commands).into()
        } else if let Owner::SolidAxial(kind) = owner {
            let p = kind.preferences(&self.commands);
            Preferences {
                radius: p.radius,
                diameter: p.diameter,
                ..Default::default()
            }
        } else if let Owner::Torus(kind) = owner {
            let p = self.torus_preferences(kind);
            Preferences {
                radius: p.radius,
                diameter: p.diameter,
                ..Default::default()
            }
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
    pub(super) fn circle_around_prompt(&mut self) {
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
            let snap_option = if s.snap_projection_plane().is_some() {
                if self.circle_project_osnap {
                    ", ProjectOsnap=Yes"
                } else {
                    ", ProjectOsnap=No"
                }
            } else {
                ""
            };
            format!(
                "{}{}; Radius, Diameter, Circumference, Area{snap_option}",
                input.size_mode().label(),
                default
            )
        } else if matches!(s.owner, Owner::SolidAxial(_)) {
            let input = self.solid_axial_session.as_ref().unwrap().input;
            if input.at_start() {
                format!("{}; pick center on curve", input.prompt())
            } else {
                input.prompt()
            }
        } else {
            "center on curve".into()
        };
        self.active_command = Some(match s.owner {
            Owner::Circle => InteractiveCommand::CircleAround { center },
            Owner::Sphere(output) => InteractiveCommand::SphereGuide {
                anchor: center,
                output,
            },
            Owner::Torus(kind) => InteractiveCommand::Torus {
                kind,
                anchor: center,
            },
            Owner::SubDSphere => InteractiveCommand::SubDSphere { anchor: center },
            Owner::SolidAxial(kind) => InteractiveCommand::SolidAxial {
                kind,
                anchor: center,
            },
            Owner::Pyramid => InteractiveCommand::Pyramid { anchor: center },
        });
        self.drafting_plane = s.input.map(Input::frame);
        let name = s.owner.name();
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
                if matches!(s.owner, Owner::SolidAxial(_)) {
                    self.solid_axial_session
                        .as_mut()
                        .unwrap()
                        .input
                        .select_guide();
                }
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
            .is_some_and(|s| matches!(s.owner, Owner::SolidAxial(_)))
            && !self.valid_solid_axial()
        {
            return false;
        }

        if self
            .circle_around_session
            .as_ref()
            .is_some_and(|s| matches!(s.owner, Owner::Torus(_)))
            && !self.valid_torus()
        {
            return false;
        }
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
            .is_some_and(|s| s.owner == Owner::Pyramid)
            && !self.valid_pyramid()
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
            let name = s.owner.name();
            self.push_log(format!(
                "{name} guide changed or became unavailable; select it again"
            ));
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
        if matches!(s.owner, Owner::SolidAxial(_)) {
            let release = s
                .postselected
                .then(|| curve.object_id())
                .into_iter()
                .collect();
            let preferences = s.input.unwrap().preferences(circle);
            if let Some(parent) = self.solid_axial_session.as_mut() {
                parent.input.preferences.diameter = preferences.diameter;
            }
            return self.accept_solid_axial_reference(circle, release);
        }
        if matches!(s.owner, Owner::Torus(_)) {
            let release = if s.postselected {
                vec![curve.object_id()]
            } else {
                vec![]
            };
            let diameter = s.input.unwrap().size_mode();
            let result = viboceros_command::circle_input::CircleDefinition::from_circle(circle);
            if let Some(parent) = self.torus_session.as_mut() {
                match diameter {
                    Mode::Radius => parent.input.preferences.diameter = false,
                    Mode::Diameter => parent.input.preferences.diameter = true,
                    _ => (),
                }
            }
            return match result {
                Ok(d) => self.accept_torus_reference(d, release),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            };
        }
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
        if let Owner::Sphere(output) = s.owner {
            let release = s.postselected.then(|| curve.object_id());
            let preferences = s.input.unwrap().preferences(circle).into();
            return match viboceros_command::circle_input::CircleDefinition::from_circle(circle) {
                Ok(definition) => self.finish_sphere(output, definition, preferences, release),
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
                        if s.owner == Owner::Pyramid {
                            let release = s.postselected_object();
                            return Some(self.accept_pyramid_guide(frame, release));
                        }
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
        let session = self.circle_around_session.as_ref().unwrap();
        let owner = session.owner;
        if matches!(owner, Owner::SolidAxial(_))
            && session.input.is_none()
            && self.try_edit_solid_axial_guide(text, session.curve.is_some())
        {
            return true;
        }
        if owner == Owner::Pyramid && self.try_edit_pyramid_guide(text) {
            return true;
        }
        if self.try_continue_circle_snap_projection(text) {
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
                let name = s.owner.name();
                self.push_log(format!(
                    "Error: Orientation is unavailable in {name} AroundCurve"
                ));
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
