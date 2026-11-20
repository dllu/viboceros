//! Boundary selection, repeated curve-end picks, and dynamic Extend input.
use super::*;
use viboceros_command::curve_extend::{self, Boundary, Options, Source, Style, Target};
use viboceros_document::HistoryGroup;
use viboceros_drafting::PointInput;

mod preview;
mod typed;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    Extend,
    Dynamic,
}

impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::Extend => "Extend",
            Self::Dynamic => "ExtendDynamic",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Preferences {
    options: Options,
    last_length: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            options: Options::default(),
            last_length: 1.,
        }
    }
}

enum Method {
    Dynamic,
    Length(f64),
    Boundary(Vec<Boundary>),
}

enum Step {
    Boundaries,
    Source,
    End(Source, EndInput),
}

#[derive(Clone, Copy)]
enum EndInput {
    Along,
    ToPoint,
    Center,
    Centered(Point3),
}

impl EndInput {
    fn target(self, point: Point3) -> Option<Target> {
        match self {
            Self::Along => Some(Target::Projected(point)),
            Self::ToPoint => Some(Target::ToPoint(point)),
            Self::Centered(center) => Some(Target::ArcCenter { center, end: point }),
            Self::Center => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Getter {
    Type,
    Join,
    Length,
    SelId,
}

pub(super) struct Session {
    command: Command,
    prefs: Preferences,
    step: Step,
    method: Method,
    getter: Option<Getter>,
    typed_source: Option<(ObjectId, Option<usize>)>,
    tolerance: Tolerance,
    history: HistoryGroup,
    preview: preview::Preview,
}

impl Session {
    pub(super) fn hint(&self) -> &'static str {
        if let Some(getter) = self.getter {
            return match getter {
                Getter::Type => "Type: Natural, Arc, Line, or Smooth",
                Getter::Join => "Join: Merge, Yes, or No",
                Getter::Length => "Enter the extension or shortening length",
                Getter::SelId => "Enter the object ID",
            };
        }
        match &self.step {
            Step::Boundaries => {
                "Select boundary objects, enter a length, or press Enter for dynamic extend"
            }
            Step::Source if self.typed_source.is_some() => "Pick a curve end or enter Start or End",
            Step::Source => {
                "Select a curve or surface edge near the end to extend. Press Enter when done"
            }
            Step::End(_, EndInput::Center) => "Center of arc extension",
            Step::End(_, EndInput::ToPoint) => "End of extension",
            Step::End(_, _) => "End of extension or enter extension length",
        }
    }

    pub(super) fn wants_point(&self) -> bool {
        matches!(self.step, Step::End(..)) && self.getter.is_none()
    }

    pub(super) fn anchor(&self) -> Option<Point3> {
        let Step::End(source, input) = &self.step else {
            return None;
        };
        if let EndInput::Centered(center) = input {
            Some(*center)
        } else {
            source.endpoint().ok()
        }
    }

    pub(super) fn preview(&self) -> Option<&[viboceros_geometry::NurbsCurve]> {
        self.preview.curves()
    }

    fn current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && document.history_group_is_current(&self.history)
            && match &self.step {
                Step::End(source, _) => source.current(document),
                _ => true,
            }
            && match &self.method {
                Method::Boundary(boundaries) => {
                    boundaries.iter().all(|boundary| boundary.current(document))
                }
                _ => true,
            }
    }
}

impl VibocerosApp {
    pub(super) fn curve_extend_owns_keyword(&self, text: &str) -> bool {
        let word = text.trim().trim_start_matches('_');
        self.plane_prompt.is_none()
            && self.curve_extend_session.as_ref().is_some_and(|session| {
                (session.typed_source.is_some()
                    && matches!(session.step, Step::Source)
                    && (word.eq_ignore_ascii_case("Start") || word.eq_ignore_ascii_case("End")))
                    || (session.prefs.options.style == Style::Arc
                        && matches!(session.step, Step::End(..))
                        && word.eq_ignore_ascii_case("Center"))
            })
    }

    pub(super) fn cancel_curve_extend(&mut self, announce: bool) {
        if let Some(session) = self.curve_extend_session.take() {
            self.document.clear_selection();
            if announce {
                self.push_log(format!("Cancelled {}", session.command.name()));
            }
        }
    }

    pub(super) fn start_curve_extend(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let Some(word) = words.next() else {
            return false;
        };
        let command = if word
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Extend")
        {
            Command::Extend
        } else if word
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("ExtendDynamic")
        {
            Command::Dynamic
        } else {
            return false;
        };
        let mut prefs = self.curve_extend_preferences[command as usize];
        let mut fixed = None;
        while let Some(arg) = words.next() {
            let (name, value) = arg.split_once('=').unwrap_or((arg, ""));
            let key = name.trim_start_matches('_');
            if key.eq_ignore_ascii_case("Type") || key.eq_ignore_ascii_case("Join") {
                let value = if value.is_empty() {
                    words.next().unwrap_or("")
                } else {
                    value
                };
                if let Err(error) = prefs.options.set(key, value) {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            } else if command == Command::Extend
                && fixed.is_none()
                && let Some(Ok(length)) =
                    PointInput::parse_length_with_units(arg, self.document.units())
            {
                fixed = Some(length);
            } else {
                return false;
            }
        }
        let preselected: Vec<_> = self.document.selected_object_ids().collect();
        self.cancel_interactive_command(false);
        let Ok(history) = self.document.begin_history_group(command.name()) else {
            self.push_log("Finish the current edit before extending a curve".into());
            return true;
        };
        let picked = if command == Command::Extend && fixed.is_none() {
            self.capture_extend_boundaries(preselected)
        } else {
            Vec::new()
        };
        let (step, method) = if let Some(length) = fixed {
            prefs.last_length = length;
            (Step::Source, Method::Length(length))
        } else if !picked.is_empty() {
            (Step::Source, Method::Boundary(picked))
        } else if command == Command::Dynamic {
            (Step::Source, Method::Dynamic)
        } else {
            (Step::Boundaries, Method::Dynamic)
        };
        self.document.clear_selection();
        self.curve_extend_preferences[command as usize] = prefs;
        self.curve_extend_session = Some(Session {
            command,
            prefs,
            step,
            method,
            getter: None,
            typed_source: None,
            tolerance: self.document.tolerance(),
            history,
            preview: preview::Preview::default(),
        });
        self.command_input.clear();
        self.push_log(format!("> {text}"));
        self.log_curve_extend();
        true
    }

    fn log_curve_extend(&mut self) {
        if let Some(session) = &self.curve_extend_session {
            let mut options = format!(
                "Type={:?} Join={:?}",
                session.prefs.options.style, session.prefs.options.join
            );
            if let Method::Length(length) = session.method {
                options.push_str(&format!(" ExtensionLength={length}"));
            }
            if session.history.can_undo() {
                options.push_str(" Undo");
            }
            if matches!(session.step, Step::End(_, EndInput::Along)) {
                match session.prefs.options.style {
                    Style::Arc => options.push_str(" Center ToPoint"),
                    Style::Smooth => options.push_str(" ToPoint"),
                    _ => {}
                }
            }
            if session.command == Command::Dynamic && !matches!(session.step, Step::Boundaries) {
                options.push_str(" ToBoundary");
            }
            self.push_log(format!("{} ({options})", session.hint()));
        }
    }

    pub(super) fn validate_curve_extend(&mut self) {
        if self
            .curve_extend_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log("Extend sources, tolerance, or history changed".into());
        }
    }

    pub(super) fn picking_curve_extend_source(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curve_extend_session
                .as_ref()
                .is_some_and(|s| matches!(s.step, Step::Source) && s.getter.is_none())
    }

    pub(super) fn picking_curve_extend_boundaries(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curve_extend_session
                .as_ref()
                .is_some_and(|s| matches!(s.step, Step::Boundaries))
    }

    pub(super) fn pick_curve_extend(
        &mut self,
        click: SelectionClick,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        if self.curve_extend_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if self.picking_curve_extend_boundaries() {
            if let Some(id) = click.object_id {
                self.select_extend_boundaries([id], click.mode);
            }
        } else if self.picking_curve_extend_source()
            && let Some(id) = click.object_id
        {
            let point = pick.and_then(|(viewport, pointer)| {
                self.viewports[viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    None,
                    pointer,
                    self.document.tolerance(),
                )
            });
            if pick.is_none() || point.is_some() {
                self.take_extend_source(id, None, point);
            }
        }
        true
    }

    pub(super) fn pick_curve_extend_edge(
        &mut self,
        contacts: &[(crate::viewport::EdgePick, Point3)],
    ) -> bool {
        if !self.picking_curve_extend_source() {
            return false;
        }
        if let Some((edge, point)) = contacts.first() {
            self.take_extend_source(edge.object, Some(edge.edge), Some(*point));
        }
        true
    }

    pub(super) fn select_extend_boundaries(
        &mut self,
        objects: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let ids: Vec<_> = objects
            .into_iter()
            .filter(|&id| {
                self.document
                    .object(id)
                    .is_some_and(|object| Boundary::accepts(object.geometry()))
            })
            .collect();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        match self
            .document
            .select_objects_direct(ids.iter().copied(), mode)
        {
            Ok(_) => self.document.mark_object_selection_temporary(ids),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn capture_extend_boundaries(&self, ids: Vec<ObjectId>) -> Vec<Boundary> {
        ids.into_iter()
            .filter_map(|id| Boundary::capture(&self.document, id).ok())
            .collect()
    }

    fn finish_extend_boundaries(&mut self) {
        let boundaries =
            self.capture_extend_boundaries(self.document.selected_object_ids().collect());
        let Some(session) = &mut self.curve_extend_session else {
            return;
        };
        self.document.clear_selection();
        session.step = Step::Source;
        session.method = if boundaries.is_empty() {
            Method::Dynamic
        } else {
            Method::Boundary(boundaries)
        };
        self.log_curve_extend();
    }

    fn take_extend_source(&mut self, id: ObjectId, edge: Option<usize>, point: Option<Point3>) {
        self.validate_curve_extend();
        let Some(session) = &mut self.curve_extend_session else {
            return;
        };
        let source = match Source::capture(&self.document, id, edge, point) {
            Ok(source) => source,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        session.typed_source = None;
        match &session.method {
            Method::Length(length) => {
                let length = *length;
                self.finish_extend_step(&source, &[], Target::Length(length));
            }
            Method::Boundary(boundaries) => {
                let boundaries = boundaries.clone();
                if self.finish_extend_step(&source, &boundaries, Target::Boundary)
                    && let Some(session) = &mut self.curve_extend_session
                    && let Method::Boundary(boundaries) = &mut session.method
                {
                    boundaries.retain(|boundary| boundary.id() != id);
                }
            }
            Method::Dynamic => {
                let _ = self.document.set_transient_selection([id]);
                self.document.mark_object_selection_temporary([id]);
                session.step = Step::End(source, EndInput::Along);
            }
        }
        self.log_curve_extend();
    }

    pub(super) fn accept_curve_extend_point(&mut self, point: Point3) -> Option<bool> {
        self.validate_curve_extend();
        if self.plane_prompt.is_some() {
            return None;
        }
        let session = self.curve_extend_session.as_mut()?;
        let Step::End(source, input) = &mut session.step else {
            return None;
        };
        if matches!(input, EndInput::Center) {
            let projected = (|| -> Result<Point3, viboceros_geometry::GeometryError> {
                let end = source.endpoint()?;
                let normal = source.tangent()?.as_vector();
                let offset = normal.dot(end.vector_to(point)?)?;
                point.translated(normal.scaled(-offset)?)
            })();
            return Some(match projected {
                Ok(center)
                    if source
                        .endpoint()
                        .is_ok_and(|end| !end.is_near(center, self.document.tolerance())) =>
                {
                    *input = EndInput::Centered(center);
                    session.preview = preview::Preview::default();
                    self.log_curve_extend();
                    true
                }
                Ok(_) => {
                    self.push_log("Pick an arc center away from the curve end".into());
                    false
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    false
                }
            });
        }
        let target = input.target(point)?;
        let source = source.clone();
        let accepted = self.finish_extend_step(&source, &[], target);
        self.log_curve_extend();
        Some(accepted)
    }

    fn finish_extend_step(
        &mut self,
        source: &Source,
        boundaries: &[Boundary],
        target: Target,
    ) -> bool {
        let Some(session) = &mut self.curve_extend_session else {
            return false;
        };
        let prepared = session
            .preview
            .take(target, session.prefs.options)
            .map_or_else(
                || curve_extend::prepare(source, boundaries, target, session.prefs.options),
                Ok,
            );
        let result =
            prepared.and_then(|prepared| prepared.commit(&mut self.document, &mut session.history));
        match result {
            Ok(_) => {
                session.step = Step::Source;
                session.preview = preview::Preview::default();
                self.push_log("Extended curve".into());
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
