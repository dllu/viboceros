//! Ordered source picks and two sequential distance getters without model edits.
use super::*;
use viboceros_command::chamfer::{Preferences, parse, parse_distance, prepare};
use viboceros_geometry::Real;
mod distance;
mod input;

#[derive(Clone, Copy)]
enum Pending {
    Distance(usize),
}
pub(super) struct Session {
    picks: Vec<(viboceros_document::Object, Point3)>,
    options: Preferences,
    pending: Option<Pending>,
    start: Option<Point3>,
    tolerance: viboceros_geometry::Tolerance,
    layer: viboceros_document::LayerId,
    pub pointer: Option<egui::Pos2>,
}
impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.pending.is_none() && self.picks.len() < 2
    }
    pub(super) fn wants_point(&self) -> bool {
        matches!(self.pending, Some(Pending::Distance(_)))
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.pending {
            Some(Pending::Distance(0)) => "First chamfer distance or two distance points",
            Some(Pending::Distance(_)) => "Second chamfer distance; Enter uses the first distance",
            None => {
                if self.picks.is_empty() {
                    "Select first curve near the end"
                } else {
                    "Select second curve near the end"
                }
            }
        }
    }
    fn prompt(&self) -> String {
        match self.pending {
            Some(Pending::Distance(i)) => format!(
                "{} chamfer distance <{}>{}",
                if i == 0 { "First" } else { "Second" },
                self.options.distances[0],
                if self.start.is_some() {
                    " from the starting point"
                } else {
                    " or first distance point"
                }
            ),
            None => format!(
                "Select {} curve near the end ({})",
                if self.picks.is_empty() {
                    "first"
                } else {
                    "second"
                },
                self.options.arguments()
            ),
        }
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.layer == doc.current_layer_id()
            && self.picks.iter().all(|(old, _)| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
}
impl VibocerosApp {
    pub(super) fn start_curve_chamfer(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Chamfer")
        }) {
            return false;
        }
        let mut pending = None;
        let options = if words.len() == 2 && !words[1].contains('=') {
            match parse_distance(words[1], self.document.units()) {
                Ok(value) => {
                    let mut o = self.commands.chamfer_preferences();
                    o.distances[0] = value;
                    pending = Some(Pending::Distance(1));
                    o
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        } else {
            match parse(
                &words[1..],
                self.commands.chamfer_preferences(),
                self.document.units(),
            ) {
                Ok(a) if a.sources.is_some() || a.picks.iter().any(Option::is_some) => {
                    return false;
                }
                Ok(a) => a.preferences,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        };
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.curve_chamfer_session = Some(Session {
            picks: Vec::new(),
            options,
            pending,
            start: None,
            tolerance: self.document.tolerance(),
            layer: self.document.current_layer_id(),
            pointer: None,
        });
        self.active_command = Some(InteractiveCommand::Chamfer { anchor: None });
        self.push_log(format!("> {text}"));
        self.curve_chamfer_prompt();
        true
    }
    fn curve_chamfer_prompt(&mut self) {
        if let Some(s) = &self.curve_chamfer_session {
            self.commands
                .remember_chamfer_preferences(s.options)
                .expect("validated Chamfer options");
            self.push_log(format!("Chamfer: {}", s.prompt()));
        }
    }
    pub(super) fn validate_curve_chamfer(&mut self) {
        if self
            .curve_chamfer_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log("Chamfer sources or document settings changed; pick again".into());
        }
    }
    pub(super) fn picking_curve_chamfer(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curve_chamfer_session
                .as_ref()
                .is_some_and(Session::selecting)
    }
    pub(super) fn pick_curve_chamfer(&mut self, id: Option<ObjectId>) -> bool {
        if !self.picking_curve_chamfer() {
            return false;
        }
        let point = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                None,
                self.curve_chamfer_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(point)) = (id, point) {
            self.pick_curve_chamfer_at(id, point);
        }
        true
    }
    pub(super) fn pick_curve_chamfer_at(&mut self, id: ObjectId, point: Point3) {
        self.validate_curve_chamfer();
        if !self.picking_curve_chamfer() {
            return;
        }
        let object = self
            .document
            .object(id)
            .filter(|o| {
                self.document.is_object_selectable(id) && o.geometry().curve_ref().is_some()
            })
            .cloned();
        let Some(object) = object else {
            self.push_log("Select an editable curve".into());
            return;
        };
        let s = self.curve_chamfer_session.as_mut().unwrap();
        s.picks.push((object, point));
        self.document
            .set_transient_selection(s.picks.iter().map(|(o, _)| o.id()))
            .expect("captured Chamfer sources");
        self.document
            .mark_object_selection_temporary(s.picks.iter().map(|(o, _)| o.id()));
        if s.picks.len() < 2 {
            self.curve_chamfer_prompt();
            return;
        }
        let result = prepare(
            &self.document,
            [
                (s.picks[0].0.id(), s.picks[0].1),
                (s.picks[1].0.id(), s.picks[1].1),
            ],
            s.options,
        )
        .and_then(|p| p.commit(&mut self.document));
        self.cancel_interactive_command(false);
        self.push_log(match result {
            Ok(_) => "Chamfered curves".into(),
            Err(e) => format!("Unable to chamfer these curves: {e}"),
        });
    }
}
