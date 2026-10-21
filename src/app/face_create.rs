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
    boundaries: Vec<(usize, [Point3; 2])>,
}
impl Session {
    pub(super) fn boundary_pick(&self) -> bool {
        !self.append_pick && self.draft.as_ref().is_none_or(solver::Draft::waiting_edge)
    }
    pub(super) fn boundary_segments(&self) -> &[(usize, [Point3; 2])] {
        &self.boundaries
    }
    pub(super) fn preview_curves(&self) -> &[NurbsCurve] {
        &self.preview
    }
    fn refresh(&mut self, cursor: Option<Point3>) {
        self.preview.clear();
        self.boundaries.clear();
        if !self.append_pick && self.draft.as_ref().is_none_or(solver::Draft::waiting_edge) {
            let mesh = if let Some(draft) = &self.draft {
                draft.current_mesh().ok().flatten()
            } else {
                self.original
                    .as_ref()
                    .and_then(|geometry| match &**geometry {
                        Geometry::Mesh(mesh) => Some(mesh.clone()),
                        Geometry::SubD(subd) => subd.control_mesh().ok(),
                        _ => None,
                    })
            };
            if let Some(mesh) = mesh {
                self.boundaries = mesh
                    .diagnostic_edges()
                    .into_iter()
                    .filter(|e| e.face_count == 1)
                    .map(|e| (e.index, e.points))
                    .collect();
            }
        }
        let Some(draft) = &self.draft else {
            return;
        };
        for (points, current) in draft
            .completed()
            .iter()
            .map(|f| (&f.points, false))
            .chain(std::iter::once((&draft.points, true)))
        {
            let mut points = if current {
                draft
                    .preview_corners(cursor)
                    .unwrap_or_else(|_| points.clone())
            } else {
                points.clone()
            };
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
        let mut input = match self.commands.face_create_input(&words[1..]) {
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
        input.preferences = solver::target_preferences(
            &self.document,
            target,
            input.preferences,
            input.output_explicit,
        );
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
        let draft = {
            match solver::Draft::with_source(
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
            preferences: draft.as_ref().map_or(input.preferences, |d| d.preferences),
            draft,
            target,
            original,
            edge: input.edge,
            append_pick: input.preferences.from_edge && target.is_none(),
            value: None,
            group,
            tolerance: self.document.tolerance(),
            preview: vec![],
            boundaries: vec![],
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
            "3DFace: select a mesh or SubD to append".into()
        } else if s.draft.as_ref().is_none_or(solver::Draft::waiting_edge) {
            "3DFace: select a boundary edge".into()
        } else {
            "3DFace: pick a corner; Enter ends the chain (Output Interpolated PolygonType FromEdge Chain InPlane WeldAngle Mode Undo)"
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
        self.face_create_session.as_ref().is_some_and(|s| {
            !s.append_pick && s.draft.as_ref().is_none_or(solver::Draft::waiting_edge)
        })
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
                let preferences = solver::target_preferences(
                    &self.document,
                    Some(id),
                    self.face_create_session.as_ref().unwrap().preferences,
                    false,
                );
                match solver::Draft::with_source(
                    preferences,
                    solver::source(&self.document, Some(id)),
                    None,
                ) {
                    Ok(draft) => {
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
                        s.preferences = draft.preferences;
                        s.draft = Some(draft);
                    }
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        return true;
                    }
                }
                self.face_create_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
            _ => (),
        }
        true
    }
    pub(super) fn pick_face_edge(&mut self, index: usize) {
        if !self.valid_face_create() {
            return;
        }
        let Some(s) = self.face_create_session.as_mut() else {
            return;
        };
        let result = if let Some(draft) = s.draft.as_mut() {
            draft.pick_edge(index)
        } else {
            solver::Draft::with_source(
                s.preferences,
                solver::source(&self.document, s.target),
                Some(index),
            )
            .map(|draft| {
                s.draft = Some(draft);
            })
        };
        match result {
            Ok(()) => {
                s.edge = Some(index);
                self.face_create_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    #[cfg(test)]
    pub(super) fn face_boundary_edges(&self) -> Option<&[(usize, [Point3; 2])]> {
        self.picking_face_edge().then(|| {
            self.face_create_session
                .as_ref()
                .unwrap()
                .boundaries
                .as_slice()
        })
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
                s.preferences = draft.preferences;
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
            && let Ok(index) = word.strip_prefix("Edge=").unwrap_or(word).parse()
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
        } else if ["PolygonType", "WeldAngle", "Mode", "Output", "Interpolated"]
            .iter()
            .any(|v| word.eq_ignore_ascii_case(v))
        {
            s.value = Some(if word.eq_ignore_ascii_case("PolygonType") {
                "PolygonType"
            } else if word.eq_ignore_ascii_case("WeldAngle") {
                "WeldAngle"
            } else if word.eq_ignore_ascii_case("Interpolated") {
                "Interpolated"
            } else if word.eq_ignore_ascii_case("Mode") {
                "Mode"
            } else {
                "Output"
            });
            self.face_create_prompt();
            self.command_input.clear();
            return true;
        } else if word.eq_ignore_ascii_case("Chain") {
            option = format!("Chain={}", if s.preferences.chain { "No" } else { "Yes" });
        } else if word.eq_ignore_ascii_case("FromEdge") {
            option = format!(
                "FromEdge={}",
                if s.preferences.from_edge { "No" } else { "Yes" }
            );
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
                if let Some(edge) = input.edge {
                    self.pick_face_edge(edge);
                    self.command_input.clear();
                    return true;
                }
                if let Some(id) = input.append {
                    if s.draft
                        .as_ref()
                        .is_some_and(|d| d.face_count() > 0 || !d.points.is_empty())
                    {
                        self.push_log("Clear the draft before changing its append target".into());
                        self.command_input.clear();
                        return true;
                    }
                    s.preferences = input.preferences;
                    s.append_pick = true;
                    self.pick_face_append(Some(id));
                    self.command_input.clear();
                    return true;
                }
                let result = if let Some(draft) = &mut s.draft {
                    draft.update_preferences(input.preferences)
                } else {
                    solver::Draft::with_source(
                        input.preferences,
                        solver::source(&self.document, s.target),
                        None,
                    )
                    .map(|d| {
                        s.draft = Some(d);
                    })
                };
                if let Err(e) = result {
                    self.push_log(format!("Error: {e}"));
                    self.command_input.clear();
                    return true;
                }
                s.preferences = input.preferences;
                s.append_pick = s.preferences.from_edge
                    && s.target.is_none()
                    && s.draft.as_ref().is_none_or(|d| d.face_count() == 0);
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
            s.draft.as_mut().unwrap().clear_points();
            s.edge = None;
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
        let mesh = match s
            .draft
            .as_ref()
            .map(|draft| draft.output(s.tolerance))
            .transpose()
        {
            Ok(Some(Some(m))) => m,
            Ok(_) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            self.document.begin_group_transaction(&s.group)?;
            match solver::apply_output(&mut self.document, s.target, mesh) {
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
                self.push_log("Created face(s)".into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
}
