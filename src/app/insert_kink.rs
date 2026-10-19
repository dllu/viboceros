//! Repeated point getter: geometry remains immutable until Enter.
use super::*;
use viboceros_command::insert_kink::{self, Draft, Options};
use viboceros_geometry::NurbsCurve;
#[derive(Debug)]
pub(super) struct Prompt {
    draft: Option<Draft>,
    options: Options,
    postselected: bool,
    subcurve: bool,
    start: Option<f64>,
    shortening: bool,
    cursor: Option<NurbsCurve>,
    hint: String,
    guides: Vec<NurbsCurve>,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.draft.is_none()
    }
    pub(super) fn wants_point(&self) -> bool {
        self.draft.is_some()
    }
    pub(super) fn cursor_curve(&self) -> Option<&NurbsCurve> {
        self.cursor.as_ref()
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn guides(&self) -> &[NurbsCurve] {
        &self.guides
    }
    fn update_guides(&mut self) {
        self.guides.clear();
        let Some(draft) = &self.draft else {
            return;
        };
        let Some((surface, _, _)) =
            viboceros_command::surface_reference::surface(draft.geometry(), None)
        else {
            return;
        };
        for (knots, domain, constant_u) in [
            (surface.knots_u(), surface.domain_u(), true),
            (surface.knots_v(), surface.domain_v(), false),
        ] {
            if constant_u && self.options.direction == viboceros_geometry::SurfaceKnotDirection::U
                || !constant_u
                    && self.options.direction == viboceros_geometry::SurfaceKnotDirection::V
            {
                continue;
            }
            let mut unique = knots
                .iter()
                .copied()
                .filter(|t| domain.contains(t))
                .collect::<Vec<_>>();
            unique.dedup();
            let values = if self.options.midpoints {
                unique
                    .windows(2)
                    .map(|p| p[0] / 2. + p[1] / 2.)
                    .collect::<Vec<_>>()
            } else {
                unique
                    .into_iter()
                    .filter(|t| *t > *domain.start() && *t < *domain.end())
                    .collect()
            };
            for t in values.into_iter().take(1024) {
                if let Ok(c) = if constant_u {
                    surface.isocurve_v(t)
                } else {
                    surface.isocurve_u(t)
                } {
                    self.guides.push(c);
                }
            }
        }
    }
    pub(super) fn surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        let d = self.draft.as_ref()?;
        d.geometry()
            .curve_ref()
            .is_none()
            .then_some((d.object(), None, true))
    }
    pub(super) fn object(&self) -> Option<ObjectId> {
        self.draft.as_ref().map(|d| d.object())
    }
    fn update_hint(&mut self) {
        self.hint = if self.selecting() {
            "Select one curve or surface (SubCrv temporarily shortens a curve); Esc cancels".into()
        } else if self.shortening {
            if self.start.is_none() {
                "Pick temporary subcurve start; Esc cancels".into()
            } else {
                "Pick temporary subcurve end; Esc cancels".into()
            }
        } else if self.cursor.is_some() {
            format!(
                "Point on curve to add a kink; {} accepted; Enter finishes; Esc discards all picks",
                self.draft.as_ref().unwrap().picks()
            )
        } else {
            format!(
                "Point on surface to add a kink ({} Toggle); Enter finishes; Esc discards all picks",
                self.options.arguments()
            )
        };
    }
}
impl VibocerosApp {
    pub(super) fn cancel_insert_kink(&mut self) {
        self.insert_kink_prompt = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.snaps.model_override = None;
        self.command_input.clear();
    }
    pub(super) fn validate_insert_kink(&mut self) {
        if self
            .insert_kink_prompt
            .as_ref()
            .and_then(|p| p.draft.as_ref())
            .is_some_and(|d| !d.current(&self.document))
        {
            self.cancel_insert_kink();
            self.push_log("InsertKink source changed; start again".into());
        }
    }
    pub(super) fn start_insert_kink(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("InsertKink")
        }) {
            return false;
        }
        let mut options = self.commands.insert_kink_options();
        let mut subcurve = false;
        for w in words {
            if w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv") {
                subcurve = true;
            } else {
                match options.accept(w) {
                    Ok(true) => {}
                    _ => return false,
                }
            }
        }
        let ids = self
            .document
            .selected_objects()
            .filter(|o| {
                self.document.is_object_selectable(o.id()) && insert_kink::accepts(o.geometry())
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let mut prompt = Prompt {
            draft: None,
            options,
            postselected: ids.len() != 1 || subcurve,
            subcurve,
            start: None,
            shortening: subcurve,
            cursor: None,
            hint: String::new(),
            guides: Vec::new(),
        };
        prompt.update_hint();
        self.insert_kink_prompt = Some(prompt);
        self.commands.remember_insert_kink_options(options);
        self.push_log(format!("> {input}"));
        if ids.len() == 1 && !subcurve {
            self.pick_insert_kink_object(Some(ids[0]));
        } else {
            self.log_insert_kink();
        }
        true
    }
    fn log_insert_kink(&mut self) {
        if let Some(p) = &mut self.insert_kink_prompt {
            p.update_hint();
            p.update_guides();
            let h = p.hint.clone();
            self.push_log(h);
        }
    }
    pub(super) fn pick_insert_kink_object(&mut self, object: Option<ObjectId>) -> bool {
        let Some(p) = &self.insert_kink_prompt else {
            return false;
        };
        if !p.selecting() {
            return true;
        }
        let Some(id) = object else {
            return true;
        };
        if p.subcurve
            && self
                .document
                .object(id)
                .is_none_or(|o| o.geometry().curve_ref().is_none())
        {
            return true;
        }
        match Draft::new(&self.document, id, None) {
            Ok(draft) => {
                let cursor = draft
                    .geometry()
                    .curve_ref()
                    .map(|c| c.to_nurbs())
                    .transpose();
                match cursor {
                    Ok(cursor) => {
                        let p = self.insert_kink_prompt.as_mut().unwrap();
                        p.cursor = cursor;
                        p.draft = Some(draft);
                        self.log_insert_kink();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn insert_kink_point(&mut self, point: Point3) {
        self.validate_insert_kink();
        let Some(p) = &mut self.insert_kink_prompt else {
            return;
        };
        let Some(d) = &mut p.draft else {
            return;
        };
        if p.shortening {
            let curve = d.geometry().curve_ref().unwrap();
            match curve.closest_parameter(point, self.document.tolerance()) {
                Ok(t) => {
                    if let Some(a) = p.start {
                        match Draft::new(&self.document, d.object(), Some([a, t])) {
                            Ok(draft) => match draft.geometry().curve_ref().unwrap().to_nurbs() {
                                Ok(cursor) => {
                                    p.cursor = Some(cursor);
                                    p.draft = Some(draft);
                                    p.shortening = false;
                                }
                                Err(e) => self.push_log(format!("Error: {e}")),
                            },
                            Err(e) => self.push_log(format!("Error: {e}")),
                        }
                    } else {
                        p.start = Some(t);
                    }
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else if let Err(e) = d.point(point, p.options) {
            self.push_log(format!("Error: {e}"));
        }
        self.log_insert_kink();
    }
    pub(super) fn accept_insert_kink_drafting_point(&mut self, point: Point3) -> Option<bool> {
        if !self
            .insert_kink_prompt
            .as_ref()
            .is_some_and(Prompt::wants_point)
        {
            return None;
        }
        self.insert_kink_point(point);
        Some(true)
    }
    pub(super) fn continue_insert_kink(&mut self, input: &str) -> bool {
        if self.insert_kink_prompt.is_none() {
            return false;
        }
        let text = input.trim();
        let key = text.trim_start_matches(['_', '-']);
        if key.eq_ignore_ascii_case("Cancel") || text == "!" {
            self.cancel_insert_kink();
            return true;
        }
        if self
            .commands
            .recognizes(text.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_insert_kink();
            return false;
        }
        self.validate_insert_kink();
        self.command_input.clear();
        let Some(p) = &mut self.insert_kink_prompt else {
            return true;
        };
        if text.is_empty() {
            if p.shortening || p.selecting() {
                self.cancel_insert_kink();
                return true;
            }
            let p = self.insert_kink_prompt.take().unwrap();
            let d = p.draft.as_ref().unwrap();
            if let Err(e) = self.document.begin_transaction("InsertKink") {
                self.push_log(format!("Error: {e}"));
                self.insert_kink_prompt = Some(p);
                return true;
            }
            match d.commit(&mut self.document, p.postselected) {
                Ok(message) => match self.document.commit_transaction() {
                    Ok(_) => self.push_log(message),
                    Err(e) => self.push_log(format!("Error: {e}")),
                },
                Err(e) => {
                    let _ = self.document.rollback_transaction();
                    self.push_log(format!("Error: {e}"));
                    self.insert_kink_prompt = Some(p);
                }
            }
            return true;
        }
        if p.selecting() && key.eq_ignore_ascii_case("SubCrv") {
            p.subcurve = true;
            p.shortening = true;
            self.log_insert_kink();
            return true;
        }
        match p.options.accept(text) {
            Ok(true) => {
                self.commands.remember_insert_kink_options(p.options);
                self.log_insert_kink();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            _ => {}
        }
        if self.try_continue_point_constraint(text)
            || self.try_continue_point_filter(text)
            || self.try_continue_point_input(text)
        {
            return true;
        }
        self.push_log("Expected a point or surface option; Enter finishes, Esc cancels".into());
        true
    }
    pub(super) fn insert_kink_viewport_hit(&mut self, output: &ViewportOutput) -> bool {
        let Some(p) = &self.insert_kink_prompt else {
            return false;
        };
        if output.enter_pressed {
            self.continue_insert_kink("");
            return true;
        }
        if p.selecting() {
            if let Some(picks) = &output.parametric_click {
                let ids = picks
                    .iter()
                    .filter(|p| p.member.is_none())
                    .map(|p| p.object)
                    .filter(|id| {
                        self.document
                            .object(*id)
                            .is_some_and(|o| insert_kink::accepts(o.geometry()))
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if ids.len() == 1 {
                    self.pick_insert_kink_object(ids.into_iter().next());
                } else if ids.len() > 1 {
                    self.selection_menu = Some(SelectionMenu {
                        choice: SelectionChoice {
                            mode: SelectionMode::Replace,
                            viewport: self.active_viewport,
                            object_ids: ids.into_iter().collect(),
                            pointer: output.selection_pointer.unwrap_or(egui::Pos2::ZERO),
                        },
                        highlighted: 0,
                    });
                }
                return true;
            }
        } else if let Some(t) = output.edge_parameter {
            if let Some(point) = p.cursor.as_ref().and_then(|c| c.evaluate(t).ok()) {
                self.accept_filtered_drafting_snap(point, None);
            }
            return true;
        } else if let Some(point) = output.picked_point {
            self.accept_filtered_drafting_snap(point, output.picked_snap);
            return true;
        }
        false
    }
}
