//! Mesh-face drafts, point Undo, boundary previews and append target picking.
use super::*;
use viboceros_command::face_create as solver;
use viboceros_document::{GeometrySnapshot, HistoryGroup};
use viboceros_geometry::{NurbsCurve, Polyline3, Tolerance};
#[cfg(test)]
mod tests;

pub(super) struct Session {
    preferences: solver::Preferences,
    draft: Option<solver::Draft>,
    target: Option<ObjectId>,
    original: Option<GeometrySnapshot>,
    edge: Option<usize>,
    append_pick: bool,
    value: Option<&'static str>,
    group: HistoryGroup,
    tolerance: Tolerance,
    preview: Vec<NurbsCurve>,
}
impl Session {
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    fn refresh(&mut self, cursor: Option<Point3>) {
        self.preview.clear();
        let Some(draft) = &self.draft else {
            return;
        };
        for (points, current) in draft
            .completed()
            .iter()
            .map(|f| (&f.points, false))
            .chain(std::iter::once((&draft.points, true)))
        {
            let mut points = points.clone();
            if let Some(p) = cursor
                .filter(|_| current)
                .and_then(|p| draft.constrained_corner(p).ok())
                && points.last() != Some(&p)
            {
                points.push(p);
            }
            if points.len() < 2 {
                continue;
            }
            if points.len() > 2 && points.first() != points.last() {
                points.push(points[0]);
            }
            if let Ok(curve) = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)
                .and_then(|p| p.to_nurbs())
            {
                self.preview.push(curve);
            }
        }
    }
}
impl VibocerosApp {
    pub(super) fn start_face_create(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("3DFace")
        }) {
            return false;
        }
        let input = match self.commands.face_create_input(&words[1..]) {
            Ok(i) => i,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let target = match solver::target(&self.document, input.append) {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("3DFace") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let original = target
            .and_then(|id| self.document.object(id))
            .map(|o| o.geometry_snapshot().clone());
        let draft = if input.preferences.from_edge && input.edge.is_none() {
            None
        } else {
            match solver::Draft::new(
                input.preferences,
                solver::source(&self.document, target),
                input.edge,
            ) {
                Ok(d) => Some(d),
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        };
        self.face_create_session = Some(Session {
            preferences: input.preferences,
            draft,
            target,
            original,
            edge: input.edge,
            append_pick: input.preferences.from_edge && target.is_none(),
            value: None,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
        });
        self.active_command = Some(InteractiveCommand::FaceCreate { anchor: None });
        for token in input.tokens {
            self.accept_face_create_token(token);
            if self.face_create_session.is_none() {
                break;
            }
        }
        self.command_input.clear();
        self.face_create_prompt();
        true
    }
    fn valid_face_create(&mut self) -> bool {
        let valid = self.face_create_session.as_ref().is_none_or(|s| {
            self.document.history_group_is_current(&s.group)
                && s.tolerance == self.document.tolerance()
                && s.target.is_none_or(|id| {
                    self.document.object(id).is_some_and(|o| {
                        self.document.is_object_selectable(id)
                            && s.original
                                .as_ref()
                                .is_some_and(|g| g.shares_storage_with(o.geometry_snapshot()))
                    })
                })
        });
        if !valid {
            self.face_create_session = None;
            self.active_command = None;
            self.push_log("3DFace inputs or history changed; restart the command".into());
        }
        valid
    }
    fn face_create_prompt(&mut self) {
        let Some(s) = self.face_create_session.as_mut() else {
            return;
        };
        s.refresh(None);
        self.active_command = Some(InteractiveCommand::FaceCreate {
            anchor: s.draft.as_ref().and_then(solver::Draft::anchor),
        });
        let message = if let Some(value) = s.value {
            format!("3DFace: enter {value}")
        } else if s.append_pick {
            "3DFace: select a mesh to append".into()
        } else if s.draft.is_none() {
            "3DFace: select a boundary edge".into()
        } else {
            "3DFace: pick a corner; Enter accepts (PolygonType InPlane WeldAngle Mode Append Undo)"
                .into()
        };
        self.push_log(message);
    }
    pub(super) fn picking_face_append(&self) -> bool {
        self.face_create_session
            .as_ref()
            .is_some_and(|s| s.append_pick)
    }
    pub(super) fn picking_face_edge(&self) -> bool {
        self.face_create_session
            .as_ref()
            .is_some_and(|s| !s.append_pick && s.draft.is_none())
    }
    pub(super) fn pick_face_append(&mut self, id: Option<ObjectId>) -> bool {
        if !self.picking_face_append() {
            return false;
        }
        if !self.valid_face_create() {
            return true;
        }
        let Some(id) = id else {
            return true;
        };
        match solver::target(&self.document, Some(id)) {
            Ok(Some(id)) => {
                let s = self.face_create_session.as_mut().unwrap();
                s.target = Some(id);
                s.original = Some(
                    self.document
                        .object(id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone(),
                );
                s.append_pick = false;
                s.draft = if s.preferences.from_edge {
                    None
                } else {
                    solver::Draft::new(
                        s.preferences,
                        solver::source(&self.document, Some(id)),
                        None,
                    )
                    .ok()
                };
                self.face_create_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
            _ => (),
        }
        true
    }
    fn pick_face_edge(&mut self, index: usize) {
        if !self.valid_face_create() {
            return;
        }
        let s = self.face_create_session.as_mut().unwrap();
        match solver::Draft::new(
            s.preferences,
            solver::source(&self.document, s.target),
            Some(index),
        ) {
            Ok(d) => {
                s.draft = Some(d);
                s.edge = Some(index);
                self.face_create_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn pick_face_edge_click(&mut self, picks: &[crate::viewport::EdgePick]) -> bool {
        if !self.picking_face_edge() {
            return false;
        }
        let target = self.face_create_session.as_ref().unwrap().target;
        if let Some(pick) = picks.iter().find(|p| Some(p.object) == target) {
            self.pick_face_edge(pick.edge);
        }
        true
    }
    pub(super) fn accept_face_create_point(&mut self, point: Point3) -> Option<bool> {
        self.face_create_session.as_ref()?;
        if self.picking_face_append() || self.picking_face_edge() {
            return Some(false);
        }
        Some(self.accept_face_create_token(solver::Token::Point(point)))
    }
    fn accept_face_create_token(&mut self, token: solver::Token) -> bool {
        if !self.valid_face_create() {
            return false;
        }
        let Some(s) = self.face_create_session.as_mut() else {
            return false;
        };
        let Some(draft) = s.draft.as_mut() else {
            return false;
        };
        match draft.accept(token) {
            Ok(()) => {
                self.last_point = draft.anchor().or(self.last_point);
                if draft.finished {
                    self.finish_face_create();
                    self.active_command = None;
                } else {
                    self.face_create_prompt();
                }
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn update_face_create_hover(&mut self, point: Point3) {
        if let Some(s) = self.face_create_session.as_mut() {
            s.refresh(Some(point));
        }
    }
    pub(super) fn continue_face_create(&mut self, input: &str) -> bool {
        if self.face_create_session.is_none() {
            return false;
        }
        if !self.valid_face_create() {
            return true;
        }
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            if !self.cancel_face_corner() {
                self.cancel_interactive_command(false);
            }
            return true;
        }
        if input.is_empty() {
            self.accept_face_create_token(solver::Token::Enter);
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.accept_face_create_token(solver::Token::Undo);
            self.command_input.clear();
            return true;
        }
        if self.picking_face_append() {
            self.pick_face_append(input.parse().ok());
            self.command_input.clear();
            return true;
        }
        if self.picking_face_edge()
            && let Ok(index) = word.parse()
        {
            self.pick_face_edge(index);
            self.command_input.clear();
            return true;
        }
        let mut option = input.to_owned();
        let s = self.face_create_session.as_mut().unwrap();
        if let Some(value) = s.value {
            option = format!("{value}={input}");
        } else if word.eq_ignore_ascii_case("Append") {
            if s.draft
                .as_ref()
                .is_some_and(|d| d.face_count() > 0 || !d.points.is_empty())
            {
                self.push_log("Undo the current corners before choosing Append".into());
                return true;
            }
            s.append_pick = true;
            s.draft = None;
            self.face_create_prompt();
            self.command_input.clear();
            return true;
        } else if ["PolygonType", "WeldAngle", "Mode", "Output"]
            .iter()
            .any(|v| word.eq_ignore_ascii_case(v))
        {
            s.value = Some(if word.eq_ignore_ascii_case("PolygonType") {
                "PolygonType"
            } else if word.eq_ignore_ascii_case("WeldAngle") {
                "WeldAngle"
            } else if word.eq_ignore_ascii_case("Mode") {
                "Mode"
            } else {
                "Output"
            });
            self.face_create_prompt();
            self.command_input.clear();
            return true;
        } else if word.eq_ignore_ascii_case("InPlane") {
            option = format!(
                "InPlane={}",
                if s.preferences.in_plane { "No" } else { "Yes" }
            );
        }
        if !option.contains('=') {
            return false;
        }
        match solver::parse(
            &option.split_whitespace().collect::<Vec<_>>(),
            s.preferences,
        ) {
            Ok(input) => {
                let rebuild = input.append.is_some()
                    || input.edge.is_some_and(|edge| Some(edge) != s.edge)
                    || input.preferences.from_edge != s.preferences.from_edge;
                if rebuild
                    && s.draft
                        .as_ref()
                        .is_some_and(|d| d.face_count() > 0 || !d.points.is_empty())
                {
                    self.push_log(
                        "Undo the current corners before changing Append or FromEdge".into(),
                    );
                    self.command_input.clear();
                    return true;
                }
                if rebuild {
                    let target = match input
                        .append
                        .map(|id| solver::target(&self.document, Some(id)))
                        .transpose()
                    {
                        Ok(target) => target.flatten().or(s.target),
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            self.command_input.clear();
                            return true;
                        }
                    };
                    let edge = if input.preferences.from_edge {
                        input.edge.or(s.edge)
                    } else {
                        None
                    };
                    let draft = if input.preferences.from_edge && edge.is_none() {
                        None
                    } else {
                        match solver::Draft::new(
                            input.preferences,
                            solver::source(&self.document, target),
                            edge,
                        ) {
                            Ok(draft) => Some(draft),
                            Err(e) => {
                                self.push_log(format!("Error: {e}"));
                                self.command_input.clear();
                                return true;
                            }
                        }
                    };
                    s.target = target;
                    s.original = target
                        .and_then(|id| self.document.object(id))
                        .map(|o| o.geometry_snapshot().clone());
                    s.edge = edge;
                    s.append_pick = input.preferences.from_edge && target.is_none();
                    s.draft = draft;
                } else if let Some(draft) = &mut s.draft {
                    draft.preferences = input.preferences;
                }
                s.preferences = input.preferences;
                s.value = None;
                self.commands.accept_face_create_preferences(s.preferences);
                self.face_create_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn cancel_face_corner(&mut self) -> bool {
        let Some(s) = self.face_create_session.as_mut() else {
            return false;
        };
        if s.draft.as_ref().is_some_and(|d| !d.points.is_empty()) {
            if s.preferences.from_edge {
                s.draft = None;
                s.edge = None;
            } else {
                s.draft.as_mut().unwrap().clear_points();
            }
            self.face_create_prompt();
            self.command_input.clear();
            return true;
        }
        let session = self.face_create_session.take().unwrap();
        self.commands
            .accept_face_create_preferences(session.preferences);
        if let Some(target) = session.target {
            let _ = self
                .document
                .select_objects_direct([target], viboceros_document::SelectionMode::Remove);
        }
        self.active_command = None;
        self.command_input.clear();
        self.push_log("3DFace canceled".into());
        true
    }
    pub(super) fn finish_face_create(&mut self) -> bool {
        if !self.valid_face_create() {
            return false;
        }
        let Some(mut s) = self.face_create_session.take() else {
            return false;
        };
        self.commands.accept_face_create_preferences(s.preferences);
        let mesh = match s.draft.as_ref().map(solver::Draft::mesh).transpose() {
            Ok(Some(Some(m))) => m,
            Ok(_) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match solver::apply(&mut self.document, s.target, mesh) {
                Ok(_) => {
                    self.document.commit_group_transaction(&mut s.group)?;
                    Ok(())
                }
                Err(e) => {
                    self.document.rollback_transaction()?;
                    Err(e)
                }
            }
        })();
        match result {
            Ok(()) => {
                self.push_log("Created mesh face(s)".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
}
