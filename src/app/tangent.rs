//! Circle, Arc and Splop share one read-only tangent-contact picker.
use super::*;
use viboceros_command::circle_tangent_input::Input;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Circle,
    Arc,
    Splop,
    SubDCylinder,
    SubDSphere,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::Splop => "Splop",
            Self::SubDCylinder => "SubDCylinder",
            Self::SubDSphere => "SubDSphere",
        }
    }
}
pub(super) struct Session {
    kind: Kind,
    pub(super) support: Option<viboceros_command::arc_tangent_input::Support>,
    pub(super) input: Input,
    pub(super) output: Option<viboceros_command::circle_deform_input::Output>,
    plane: Frame3,
    pub(super) pointer: Option<egui::Pos2>,
}
impl Session {
    pub(super) fn sphere(&self) -> bool {
        self.kind == Kind::Splop
    }
    pub(super) fn wants_point(&self) -> bool {
        self.support.is_some() || self.input.wants_point()
    }
}
impl VibocerosApp {
    pub(super) fn picking_tangent_curve(&self) -> bool {
        self.tangent_session
            .as_ref()
            .is_some_and(|s| s.support.is_none() && s.input.wants_curve())
    }
    pub(super) fn start_tangent_circle(
        &mut self,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        self.start_tangent(Kind::Circle, output);
    }
    pub(super) fn start_subd_sphere_tangent(&mut self) {
        self.start_tangent(Kind::SubDSphere, None);
    }
    pub(super) fn start_subd_cylinder_tangent(&mut self) {
        self.start_tangent(Kind::SubDCylinder, None);
    }
    pub(super) fn start_tangent_arc(&mut self) {
        self.start_tangent(Kind::Arc, None);
    }
    pub(super) fn start_splop_tangent(&mut self) {
        self.start_tangent(Kind::Splop, None);
    }
    pub(super) fn picking_splop_tangent(&self) -> bool {
        self.tangent_session.as_ref().is_some_and(Session::sphere)
    }
    fn start_tangent(
        &mut self,
        kind: Kind,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        if !matches!(kind, Kind::Splop | Kind::SubDCylinder | Kind::SubDSphere) {
            self.cancel_interactive_command(false);
        }
        let preferences = if kind == Kind::Arc {
            viboceros_command::circle_around_input::Preferences {
                radius: self.commands.arc_tangent_radius(),
                ..Default::default()
            }
        } else if kind == Kind::SubDCylinder {
            viboceros_command::circle_around_input::Preferences {
                radius: self.commands.subd_cylinder_preferences().radius,
                ..Default::default()
            }
        } else if matches!(kind, Kind::Splop | Kind::SubDSphere) {
            Default::default()
        } else {
            self.commands.circle_preferences()
        };
        self.tangent_session = Some(Session {
            kind,
            support: None,
            input: if kind == Kind::Splop {
                Input::reference_sphere(plane, self.document.tolerance())
            } else {
                Input::new(plane, self.document.tolerance(), preferences)
            },
            plane,
            output,
            pointer: None,
        });
        self.tangent_prompt();
    }
    pub(super) fn try_start_tangent(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        let [command, mode] = words.as_slice() else {
            return false;
        };
        if !mode
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Tangent")
        {
            return false;
        }
        let command = command.trim_start_matches(['_', '-']);
        if ["Circle", "C"]
            .iter()
            .any(|w| command.eq_ignore_ascii_case(w))
        {
            self.start_tangent_circle(None);
        } else if ["Arc", "A"].iter().any(|w| command.eq_ignore_ascii_case(w)) {
            self.start_tangent_arc();
        } else {
            return false;
        }
        true
    }
    fn tangent_prompt(&mut self) {
        let Some(s) = &self.tangent_session else {
            return;
        };
        let anchor = s.input.anchor();
        self.active_command = Some(match s.kind {
            Kind::Arc => InteractiveCommand::ArcTangent { anchor },
            Kind::Circle => InteractiveCommand::CircleTangent { anchor },
            Kind::Splop => InteractiveCommand::Splop { last: anchor },
            Kind::SubDCylinder => InteractiveCommand::SubDCylinder { anchor },
            Kind::SubDSphere => InteractiveCommand::SubDSphere { anchor },
        });
        self.drafting_plane = s
            .support
            .as_ref()
            .and_then(|s| {
                Frame3::try_from_directions(
                    s.circle.center(),
                    s.circle.x_axis().as_vector(),
                    s.circle.y_axis().as_vector(),
                    self.document.tolerance(),
                )
                .ok()
            })
            .or(Some(s.plane));
        let prompt = if s.support.is_some() {
            "Choose arc".into()
        } else {
            s.input.prompt()
        };
        self.push_log(format!("{} Tangent: {prompt}; Esc cancels", s.kind.name()));
    }
    pub(super) fn validate_tangent_sources(&mut self) -> bool {
        if self
            .tangent_session
            .as_ref()
            .is_some_and(|s| s.kind == Kind::SubDSphere)
            && !self.valid_subd_sphere()
        {
            return false;
        }

        if self
            .tangent_session
            .as_ref()
            .is_some_and(|s| s.kind == Kind::SubDCylinder)
            && !self.valid_subd_cylinder()
        {
            return false;
        }
        if self
            .tangent_session
            .as_ref()
            .is_some_and(|s| !s.input.current(&self.document))
        {
            let session = self.tangent_session.as_mut().unwrap();
            session.input.reset_sources(self.document.tolerance());
            session.support = None;
            if session.sphere() {
                self.refresh_splop_reference_history();
            }
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log(
                "Tangent source changed or became unavailable; pick contacts again".into(),
            );
            self.tangent_prompt();
        }
        true
    }
    fn finish_tangent(&mut self, choice: Option<Point3>) -> bool {
        let s = self.tangent_session.as_ref().unwrap();
        if s.kind == Kind::SubDSphere {
            let result = s.input.finish().and_then(|c| {
                viboceros_command::circle_input::CircleDefinition::from_circle(c)
                    .map_err(Into::into)
            });
            return match result {
                Ok(d) => self.accept_subd_sphere_reference(d, vec![]),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            };
        }
        if s.kind == Kind::SubDCylinder {
            let result = s.input.finish();
            return match result {
                Ok(circle) => self.accept_subd_cylinder_reference(circle),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            };
        }
        if s.sphere() {
            let result = s.input.finish();
            return match result {
                Ok(circle) => self.accept_splop_tangent_circle(circle),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            };
        }
        let command = if s.kind == Kind::Arc {
            viboceros_command::arc_tangent_input::command(
                &s.input,
                choice.expect("completed arc choice"),
            )
        } else {
            let command = s.input.command();
            s.output
                .map_or(command.clone(), |o| o.wrap_command(&command))
        };
        if s.kind == Kind::Arc {
            self.finish_arc_command(&command, Some(s.plane))
        } else {
            self.try_execute_command_with_plane(&command, Some(s.plane))
        }
    }
    fn tangent_result(
        &mut self,
        result: Result<Option<viboceros_geometry::Circle3>, viboceros_command::CommandError>,
    ) -> bool {
        match result {
            Ok(Some(_)) => {
                let s = self.tangent_session.as_mut().unwrap();
                if s.kind == Kind::Arc {
                    match viboceros_command::arc_tangent_input::Support::from_contacts(&s.input) {
                        Ok(support) => {
                            s.support = Some(support);
                            self.tangent_prompt();
                            true
                        }
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            false
                        }
                    }
                } else {
                    self.finish_tangent(None)
                }
            }
            Ok(None) => {
                self.tangent_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn pick_tangent_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        if !self.picking_tangent_curve() {
            return false;
        }
        if !self.validate_tangent_sources() {
            return true;
        }
        let result =
            self.tangent_session
                .as_mut()
                .unwrap()
                .input
                .curve(&self.document, id, edge, point);
        self.component_selection.clear_choices();
        self.tangent_result(result);
        true
    }
    pub(super) fn pick_tangent_curve(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if self.tangent_session.is_none() {
            return false;
        }
        if !self.picking_tangent_curve() {
            return true;
        }
        if !self.validate_tangent_sources() {
            return true;
        }
        let point = id.and_then(|id| {
            let object = self.document.object(id)?;
            let pointer = self.tangent_session.as_ref()?.pointer?;
            self.viewports[self.active_viewport].curve_pick_point(
                object.geometry(),
                edge,
                pointer,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(point)) = (id, point) {
            self.pick_tangent_at(id, edge, point);
        } else {
            self.push_log("Tangent: pick a location on a curve or surface edge".into());
        }
        true
    }
    pub(super) fn accept_tangent_point(&mut self, point: Point3) -> Option<bool> {
        self.tangent_session.as_ref()?;
        if !self.validate_tangent_sources() {
            return Some(false);
        }
        if let Some(support) = self
            .tangent_session
            .as_ref()
            .and_then(|s| s.support.as_ref())
        {
            return Some(match support.choose(point, self.document.tolerance()) {
                Ok(_) => self.finish_tangent(Some(point)),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            });
        }
        let result = self.tangent_session.as_mut().unwrap().input.point(point);
        Some(self.tangent_result(result))
    }
    pub(super) fn try_continue_tangent(&mut self, text: &str) -> bool {
        if self.tangent_session.is_none() {
            return false;
        }
        if !self.validate_tangent_sources() {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let s = self.tangent_session.as_mut().unwrap();
            if s.support.is_some() {
                self.tangent_prompt();
            } else if s.input.accepts_default() {
                let result = s.input.enter();
                self.tangent_result(result);
            } else {
                self.cancel_interactive_command(true);
            }
        } else if self.tangent_session.as_ref().unwrap().support.is_some() {
            return false;
        } else if self.tangent_session.as_mut().unwrap().input.option(word) {
            self.point_filter = None;
            self.point_constraint = None;
            self.tangent_prompt();
        } else {
            let s = self.tangent_session.as_mut().unwrap();
            if !s.input.wants_number() {
                return false;
            }
            let value = word
                .split_once('=')
                .filter(|(key, _)| key.eq_ignore_ascii_case("Radius"))
                .map_or(text, |(_, value)| value);
            let Some(n) = viboceros_drafting::PointInput::parse_length_with_units(
                value,
                self.document.units(),
            ) else {
                return false;
            };
            let result = n
                .map_err(|e| e.to_string())
                .and_then(|n| s.input.number(n).map_err(|e| e.to_string()));
            match result {
                Ok(result) => {
                    self.tangent_result(Ok(result));
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.command_input.clear();
        true
    }
}
