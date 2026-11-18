//! Pipe rail selection, staged sizing and grouped document edits.
use super::*;
use viboceros_command::{
    CommandError,
    curve_reference::parse_edge,
    pipe::{Input, Rail, RailMode},
};
use viboceros_document::HistoryGroup;
use viboceros_geometry::{Frame3, LengthUnitSystem};

pub(super) struct Session {
    input: Input,
    pub(super) rails: Vec<Rail>,
    undo: Vec<Input>,
    group: HistoryGroup,
    highlighted: Vec<ObjectId>,
}

impl Session {
    pub(super) fn preview(&self) -> Option<(&Input, &[Rail])> {
        (!self.input.selecting()).then_some((&self.input, &self.rails))
    }

    pub(super) fn radius_plane(&self) -> Option<Frame3> {
        self.input.radius_plane(self.rails.first()?)
    }
}

impl VibocerosApp {
    pub(super) fn try_start_pipe(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|word| command_is(word, "Pipe")) {
            return false;
        }
        let mut input = Input::new(self.commands.pipe_preferences());
        let mut source = None;
        for word in words {
            if let Some(reference) = rail_argument(word) {
                if source.replace(reference).is_some() {
                    return false;
                }
            } else if !input.option(word).is_ok_and(|result| result == Some(false)) {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Pipe") {
            Ok(group) => group,
            Err(error) => {
                self.pipe_error(error);
                return true;
            }
        };
        self.pipe_session = Some(Session {
            input,
            rails: vec![],
            undo: vec![],
            group,
            highlighted: vec![],
        });
        self.pipe_prompt();
        if let Some((id, edge)) = source {
            self.pick_pipe_rail(id, edge);
        } else {
            let selected = self
                .document
                .selected_objects()
                .filter(|object| object.geometry().curve_ref().is_some())
                .map(|object| object.id())
                .collect::<Vec<_>>();
            let mode = self.pipe_session.as_ref().unwrap().input.rail_mode();
            if mode == RailMode::Multiple || (selected.len() == 1 && mode == RailMode::Single) {
                self.pick_pipe_rails(selected.into_iter().map(|id| (id, None)));
            }
        }
        true
    }

    pub(super) fn picking_pipe_rail(&self) -> bool {
        self.pipe_session
            .as_ref()
            .is_some_and(|s| s.input.selecting())
    }

    pub(super) fn picking_pipe_chain(&self) -> bool {
        self.pipe_session
            .as_ref()
            .is_some_and(|s| s.input.selecting() && s.input.rail_mode() == RailMode::Chain)
    }

    pub(super) fn pipe_edge_highlights(
        &self,
    ) -> impl Iterator<Item = crate::viewport::EdgePick> + '_ {
        self.pipe_session
            .iter()
            .flat_map(|s| &s.rails)
            .flat_map(Rail::references)
            .filter_map(|(object, edge)| {
                Some(crate::viewport::EdgePick {
                    object,
                    edge: edge?,
                })
            })
    }

    fn pipe_error(&mut self, error: impl std::fmt::Display) {
        self.command_file.fail_dispatch();
        self.push_log(format!("Error: {error}"));
    }

    fn pipe_prompt(&mut self) {
        let session = self.pipe_session.as_ref().unwrap();
        self.commands
            .remember_pipe_preferences(session.input.preferences());
        let selecting = session.input.selecting();
        let anchor = session.radius_plane().map(Frame3::origin);
        let prompt = session.input.prompt();
        self.active_command = Some(InteractiveCommand::Pipe { selecting, anchor });
        self.command_input.clear();
        self.point_filter = None;
        self.point_constraint = None;
        self.push_log(prompt);
    }

    fn valid_pipe(&mut self) -> bool {
        if let Some(session) = &self.pipe_session
            && (!self.document.history_group_is_current(&session.group)
                || session
                    .rails
                    .iter()
                    .any(|rail| !rail.current(&self.document)))
        {
            self.cancel_interactive_command(false);
            self.pipe_error("Pipe rails or history changed. Restart the command.");
            return false;
        }
        true
    }

    pub(super) fn pick_pipe_rail(&mut self, source: ObjectId, edge: Option<usize>) {
        self.pick_pipe_rails(std::iter::once((source, edge)));
    }

    fn pick_pipe_rails(&mut self, picks: impl Iterator<Item = (ObjectId, Option<usize>)>) {
        if !self.valid_pipe() || !self.picking_pipe_rail() {
            return;
        }
        let mut seen = self
            .pipe_session
            .as_ref()
            .unwrap()
            .rails
            .iter()
            .flat_map(Rail::references)
            .collect::<std::collections::HashSet<_>>();
        let mut rails = vec![];
        let mut highlighted = vec![];
        for (id, edge) in picks.filter(|pick| seen.insert(*pick)) {
            match Rail::capture(&self.document, id, edge) {
                Ok(rail) => {
                    if edge.is_none() && !self.document.is_selected(id) {
                        highlighted.push(id);
                    }
                    rails.push(rail);
                }
                Err(error) => self.pipe_error(error),
            }
        }
        if rails.is_empty() {
            return;
        }
        let session = self.pipe_session.as_mut().unwrap();
        if self
            .document
            .select_objects_direct(highlighted.iter().copied(), SelectionMode::Add)
            .is_ok()
        {
            session.highlighted.extend(highlighted);
        }
        session.rails.extend(rails);
        if session.input.rail_mode() == RailMode::Single {
            self.finish_pipe_rail_selection();
        } else {
            self.pipe_prompt();
        }
    }

    fn finish_pipe_rail_selection(&mut self) {
        let session = self.pipe_session.as_mut().unwrap();
        if session.rails.is_empty() {
            self.pipe_error("Select a pipe rail first");
            return;
        }
        if session.input.rail_mode() == RailMode::Chain {
            match Rail::chain(session.rails.clone(), self.document.tolerance()) {
                Ok(rail) => session.rails = vec![rail],
                Err(error) => {
                    self.pipe_error(error);
                    return;
                }
            }
        }
        let closed =
            session.input.rail_mode() != RailMode::Multiple && session.rails[0].is_closed();
        session.input.start_sizes(closed);
        self.pipe_prompt();
    }

    pub(super) fn try_continue_pipe(&mut self, text: &str) -> bool {
        if self.pipe_session.is_none() || command_is(text, "Cancel") {
            return false;
        }
        if !self.valid_pipe() {
            return true;
        }
        let word = text.trim();
        let session = self.pipe_session.as_ref().unwrap();
        let mut next = session.input.clone();
        let option = normalized_size_option(word, self.document.units());
        match next.option(option.as_deref().unwrap_or(word)) {
            Ok(Some(ready)) => {
                self.advance_pipe(next, Ok(ready));
                return true;
            }
            Err(error) => {
                self.advance_pipe(next, Err(error));
                return true;
            }
            Ok(None) => {}
        }
        if command_is(word, "Undo") {
            if let Some(previous) = self.pipe_session.as_mut().unwrap().undo.pop() {
                self.pipe_session.as_mut().unwrap().input = previous;
                self.pipe_prompt();
            } else if self.picking_pipe_rail() {
                self.remove_pipe_rail();
            }
            return true;
        }
        if self.picking_pipe_rail() {
            if word.is_empty() || command_is(word, "Enter") {
                self.finish_pipe_rail_selection();
            } else if let Some((id, edge)) = rail_argument(word) {
                self.pick_pipe_rail(id, edge);
            } else if next.rail_mode() == RailMode::Multiple
                && (command_is(word, "SelCrv") || command_is(word, "SelAll"))
            {
                let ids = self
                    .document
                    .selectable_objects()
                    .filter(|object| object.geometry().curve_ref().is_some())
                    .map(|object| object.id())
                    .collect::<Vec<_>>();
                self.pick_pipe_rails(ids.into_iter().map(|id| (id, None)));
            } else {
                return false;
            }
            self.command_input.clear();
            return true;
        }
        let completed = if word.is_empty() || command_is(word, "Enter") {
            next.enter()
        } else if next.wants_number() {
            let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                word,
                self.document.units(),
            ) else {
                return false;
            };
            value
                .map_err(|_| CommandError::InvalidNumber(word.to_owned()))
                .and_then(|value| next.number(value))
        } else {
            return false;
        };
        self.advance_pipe(next, completed);
        true
    }

    fn remove_pipe_rail(&mut self) {
        let session = self.pipe_session.as_mut().unwrap();
        if let Some(rail) = session.rails.pop() {
            let id = rail.references().next().unwrap().0;
            if !session
                .rails
                .iter()
                .flat_map(Rail::references)
                .any(|(other, _)| other == id)
                && let Some(index) = session
                    .highlighted
                    .iter()
                    .position(|&selected| selected == id)
            {
                session.highlighted.remove(index);
                let _ = self
                    .document
                    .select_objects_direct([id], SelectionMode::Remove);
            }
        }
        self.pipe_prompt();
    }

    pub(super) fn accept_pipe_point(&mut self, point: Point3) -> Option<bool> {
        self.pipe_session.as_ref()?;
        if !self.valid_pipe() {
            return Some(false);
        }
        let session = self.pipe_session.as_ref().unwrap();
        let Some(rail) = session.rails.first().filter(|_| !session.input.selecting()) else {
            self.pipe_error("Select a pipe rail first");
            return Some(false);
        };
        let mut next = session.input.clone();
        let completed = next.point(rail, point, self.document.tolerance());
        Some(self.advance_pipe(next, completed))
    }

    fn advance_pipe(&mut self, next: Input, completed: Result<bool, CommandError>) -> bool {
        self.command_input.clear();
        match completed {
            Err(error) => {
                self.pipe_error(error);
                false
            }
            Ok(false) => {
                let session = self.pipe_session.as_mut().unwrap();
                let previous = std::mem::replace(&mut session.input, next);
                if !previous.selecting() {
                    session.undo.push(previous);
                }
                self.pipe_prompt();
                true
            }
            Ok(true) => self.finish_pipe(next),
        }
    }

    fn finish_pipe(&mut self, input: Input) -> bool {
        let session = self.pipe_session.as_ref().unwrap();
        let geometries = match input.geometries(
            &session.rails,
            self.commands.use_extrusions(),
            self.document.tolerance(),
        ) {
            Ok(geometries) => geometries,
            Err(error) => {
                self.pipe_error(error);
                return false;
            }
        };
        let count = geometries.len();
        let mut session = self.pipe_session.take().unwrap();
        let result = (|| -> Result<(), DocumentError> {
            self.document.begin_group_transaction(&session.group)?;
            let pending = self
                .document
                .release_command_selection_on_history_replay(session.highlighted.iter().copied())
                .and_then(|()| {
                    geometries
                        .into_iter()
                        .try_for_each(|geometry| self.document.add_geometry(geometry).map(|_| ()))
                });
            if let Err(error) = pending {
                self.document.rollback_transaction()?;
                return Err(error);
            }
            self.document.commit_group_transaction(&mut session.group)?;
            Ok(())
        })();
        if let Err(error) = result {
            self.pipe_session = Some(session);
            self.pipe_error(error);
            return false;
        }
        self.release_pipe_highlights(session.highlighted);
        self.cancel_interactive_command(false);
        self.commands.remember_pipe_preferences(input.preferences());
        self.push_log(format!("Created {count} Pipe object(s)"));
        true
    }

    fn release_pipe_highlights(&mut self, ids: Vec<ObjectId>) {
        let ids = ids
            .into_iter()
            .filter(|&id| self.document.is_object_selectable(id))
            .collect::<Vec<_>>();
        let _ = self
            .document
            .select_objects_direct(ids, SelectionMode::Remove);
    }

    pub(super) fn cancel_pipe(&mut self) {
        if let Some(session) = self.pipe_session.take() {
            self.release_pipe_highlights(session.highlighted);
        }
    }
}

fn command_is(text: &str, name: &str) -> bool {
    text.trim()
        .trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case(name)
}

fn rail_argument(text: &str) -> Option<(ObjectId, Option<usize>)> {
    let (name, value) = text.trim().split_once('=')?;
    if command_is(name, "Curve") {
        return Some((value.trim().parse().ok()?, None));
    }
    if command_is(name, "Edge") {
        let (object, index) = parse_edge(value.trim())?;
        return Some((object, Some(index)));
    }
    None
}

fn normalized_size_option(text: &str, units: &LengthUnitSystem) -> Option<String> {
    let (name, value) = text
        .split_once('=')
        .or_else(|| text.split_once(char::is_whitespace))?;
    if ["Diameter", "Radius", "WallThickness"]
        .iter()
        .any(|option| command_is(name, option))
    {
        let value =
            viboceros_drafting::PointInput::parse_length_with_units(value.trim(), units)?.ok()?;
        Some(format!("{}={value}", name.trim()))
    } else {
        None
    }
}
