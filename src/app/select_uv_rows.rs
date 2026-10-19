//! SelUV keeps each accepted row, including when Escape ends the getter.
use super::*;
use viboceros_command::select_uv_rows::{self as rows, Options, Session};

#[derive(Debug)]
pub(super) struct Prompt {
    session: Option<Session>,
    options: Options,
    preselected: bool,
    hint: String,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.session.is_none()
    }
    pub(super) fn hint(&self) -> &str {
        &self.hint
    }
    pub(super) fn surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        Some((self.session.as_ref()?.object(), None, true))
    }
    pub(super) fn picker(&self) -> Option<(&Session, Options)> {
        Some((self.session.as_ref()?, self.options))
    }
    fn refresh(&mut self) {
        self.hint = if self.selecting() {
            "Select one surface; Esc cancels".into()
        } else {
            format!(
                "Select row of points ({}{}); Enter or Esc finishes",
                self.options.arguments(),
                if self.options.direction == viboceros_geometry::SurfaceKnotDirection::Both {
                    ""
                } else {
                    " Toggle"
                }
            )
        };
    }
}
impl VibocerosApp {
    pub(super) fn start_select_uv(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SelUV")
        }) {
            return false;
        }
        let mut options = self.commands.select_uv_options();
        for word in words {
            if !matches!(options.accept(word), Ok(true)) {
                return false;
            }
        }
        let ids = self
            .document
            .selected_objects()
            .filter(|o| self.document.is_object_selectable(o.id()) && rows::accepts(o.geometry()))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let mut prompt = Prompt {
            session: None,
            options,
            preselected: ids.len() == 1,
            hint: String::new(),
        };
        prompt.refresh();
        self.select_uv_prompt = Some(prompt);
        self.commands.remember_select_uv_options(options);
        self.command_input.clear();
        if ids.len() == 1 {
            self.pick_select_uv_object(Some(ids[0]));
        } else {
            self.document.clear_selection();
            self.log_select_uv();
        }
        true
    }
    fn log_select_uv(&mut self) {
        if let Some(p) = &self.select_uv_prompt {
            self.push_log(format!("SelUV: {}", p.hint()));
        }
    }
    pub(super) fn finish_select_uv(&mut self) {
        if let Some(p) = self.select_uv_prompt.take()
            && let Some(session) = p.session
            && let Err(e) = session.finish(&mut self.document)
        {
            self.push_log(format!("Error: {e}"));
        }
        self.point_filter = None;
        self.point_constraint = None;
        self.snaps.model_override = None;
        self.command_input.clear();
    }
    pub(super) fn validate_select_uv(&mut self) {
        if self
            .select_uv_prompt
            .as_ref()
            .and_then(|p| p.session.as_ref())
            .is_some_and(|s| !s.current(&self.document))
        {
            self.finish_select_uv();
            self.push_log("SelUV surface or control display changed; start again".into());
        }
    }
    pub(super) fn pick_select_uv_object(&mut self, id: Option<ObjectId>) -> bool {
        let Some(p) = &self.select_uv_prompt else {
            return false;
        };
        if !p.selecting() {
            return true;
        }
        let Some(id) = id else {
            return true;
        };
        match Session::start(&mut self.document, id, p.preselected) {
            Ok(session) => {
                let p = self.select_uv_prompt.as_mut().unwrap();
                p.session = Some(session);
                p.refresh();
                self.log_select_uv();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn select_uv_point(&mut self, point: Point3) -> bool {
        self.validate_select_uv();
        let Some(p) = &self.select_uv_prompt else {
            return false;
        };
        let Some(session) = &p.session else {
            return false;
        };
        match session.pick(&mut self.document, point, p.options) {
            Ok(count) => self.push_log(format!("SelUV: {count} grip(s) selected")),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn continue_select_uv(&mut self, input: &str) -> bool {
        if self.select_uv_prompt.is_none() {
            return false;
        }
        let text = input.trim();
        let key = text.trim_start_matches(['_', '-']);
        if text.is_empty() || text == "!" || key.eq_ignore_ascii_case("Cancel") {
            self.finish_select_uv();
            return true;
        }
        if self
            .commands
            .recognizes(text.split_whitespace().next().unwrap())
        {
            self.finish_select_uv();
            return false;
        }
        self.validate_select_uv();
        let Some(p) = &mut self.select_uv_prompt else {
            return true;
        };
        self.command_input.clear();
        if !p.selecting() {
            match p.options.accept(text) {
                Ok(true) => {
                    self.commands.remember_select_uv_options(p.options);
                    p.refresh();
                    self.log_select_uv();
                    return true;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                Ok(false) => {}
            }
            if self.try_continue_point_constraint(text)
                || self.try_continue_point_filter(text)
                || self.try_continue_point_input(text)
            {
                return true;
            }
        }
        self.log_select_uv();
        true
    }
    pub(super) fn select_uv_viewport_hit(&mut self, output: &ViewportOutput) -> bool {
        let Some(p) = &self.select_uv_prompt else {
            return false;
        };
        if output.enter_pressed {
            self.finish_select_uv();
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
                            .is_some_and(|o| rows::accepts(o.geometry()))
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                if ids.len() == 1 {
                    self.pick_select_uv_object(ids.into_iter().next());
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
        } else if let Some(selection) = &output.control_point_selection {
            let owner = p.session.as_ref().unwrap().object();
            let points = selection
                .picks
                .iter()
                .filter(|g| g.object == owner)
                .filter_map(|g| {
                    self.document
                        .control_point_locations(owner)
                        .and_then(|p| p.get(g.index))
                        .copied()
                })
                .collect::<Vec<_>>();
            for point in points {
                self.select_uv_point(point);
            }
            return true;
        } else if let Some(point) = output.picked_point {
            self.accept_filtered_drafting_snap(point, output.picked_snap);
            return true;
        }
        false
    }
}
