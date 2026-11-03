//! Command-first source selection and a cached control-cage offset preview.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::offset_subd::{self as solver, Options, Plan};
pub(super) struct Session {
    options: Options,
    flips: BTreeSet<ObjectId>,
    plan: Plan,
    edges: Vec<viboceros_geometry::LineSegment>,
    postselected: bool,
}
impl Session {
    pub(super) fn edges(&self) -> &[viboceros_geometry::LineSegment] {
        &self.edges
    }
}
impl VibocerosApp {
    pub(super) fn start_offset_subd(&mut self, input: &str, postselected: bool) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("OffsetSubD")
        }) {
            return false;
        }
        let Ok(input) = solver::parse(&words[1..], self.commands.offset_subd_options()) else {
            return false;
        };
        if input.distance_supplied || self.document.selected_object_count() == 0 {
            return false;
        }
        let plan = match Plan::prepare(&self.document, input.options, &BTreeSet::new()) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let edges = match plan.preview_edges() {
            Ok(e) => e,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.commands.remember_offset_subd_options(input.options);
        self.offset_subd_session = Some(Session {
            options: input.options,
            flips: BTreeSet::new(),
            plan,
            edges,
            postselected,
        });
        self.active_command = Some(InteractiveCommand::OffsetSubD);
        self.log_offset_subd();
        true
    }
    fn log_offset_subd(&mut self) {
        if let Some(s) = &self.offset_subd_session {
            self.push_log(format!(
                "{}; click an input or use FlipAll to reverse its side; Enter commits, Esc cancels",
                s.options.command()
            ));
        }
    }
    pub(super) fn continue_offset_subd(&mut self, input: &str) -> bool {
        let Some(mut session) = self.offset_subd_session.take() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.active_command = None;
            self.command_input.clear();
            return true;
        }
        if input.is_empty() {
            let result = session
                .plan
                .commit(&mut self.document, session.postselected);
            match result {
                Ok(message) => {
                    self.active_command = None;
                    self.push_log(message);
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.offset_subd_session = Some(session);
                }
            }
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.active_command = None;
            return false;
        } else {
            let result = solver::parse(
                &input.split_whitespace().collect::<Vec<_>>(),
                session.options,
            )
            .and_then(|parsed| {
                if !session.plan.is_current(&self.document) {
                    return Err(viboceros_command::CommandError::OffsetSubDStale);
                }
                let plan = Plan::prepare(&self.document, parsed.options, &session.flips)?;
                let edges = plan.preview_edges()?;
                self.commands.remember_offset_subd_options(parsed.options);
                session.options = parsed.options;
                session.plan = plan;
                session.edges = edges;
                Ok(())
            });
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
            self.offset_subd_session = Some(session);
            self.log_offset_subd();
        }
        self.command_input.clear();
        true
    }
    pub(super) fn flip_offset_subd_input(&mut self, id: Option<ObjectId>) -> bool {
        let Some(session) = self.offset_subd_session.as_ref() else {
            return false;
        };
        if !session.plan.is_current(&self.document) {
            self.push_log(
                "Error: OffsetSubD inputs changed; cancel and select inputs again".into(),
            );
            return true;
        }
        if let Some(id) = id.filter(|id| self.document.is_selected(*id)) {
            let mut flips = session.flips.clone();
            if !flips.remove(&id) {
                flips.insert(id);
            }
            let result = Plan::prepare(&self.document, session.options, &flips)
                .and_then(|p| p.preview_edges().map(|e| (p, e)));
            match result {
                Ok((plan, edges)) => {
                    let s = self.offset_subd_session.as_mut().unwrap();
                    s.flips = flips;
                    s.plan = plan;
                    s.edges = edges;
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        true
    }
}
