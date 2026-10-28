//! Sweep1 curve selection and point ends share the ordinary drafting pipeline.
use super::object_selection::PendingObjectCommand;
use super::*;
use viboceros_command::sweep::Options;
use viboceros_document::GeometrySnapshot;

mod input;
mod selection;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
enum End {
    Start,
    Finish,
}
#[derive(Clone, Debug)]
struct State {
    options: Options,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    rail: Option<(ObjectId, GeometrySnapshot)>,
    finished: bool,
}
#[derive(Clone, Debug)]
pub(super) struct Session {
    state: State,
    undo: Vec<State>,
    picking: Option<End>,
    value: Option<&'static str>,
}
impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.picking.is_none() && self.value.is_none() && !self.state.finished
    }
    pub(super) fn hint(&self) -> &'static str {
        match (self.picking, self.value, self.state.finished) {
            (Some(End::Start), _, _) => "Pick the Sweep1 start point; Esc cancels",
            (Some(End::Finish), _, _) => "Pick the Sweep1 end point; Esc cancels",
            (_, Some(_), _) => "Enter the Sweep1 option value; Undo returns, Esc cancels",
            (_, _, true) => "Enter creates the sweep; StartPoint/EndPoint/Undo; Esc cancels",
            _ => {
                "Select rail and profiles in order; Point/StartPoint/EndPoint/RefitRail/FrameStyle/Undo; Enter sweeps"
            }
        }
    }
    fn profiles(&self) -> usize {
        self.state.sources.len()
            - usize::from(
                self.state
                    .sources
                    .iter()
                    .any(|(id, _)| self.state.rail.as_ref().is_some_and(|(rail, _)| rail == id))
                    || (self.state.options.rail_id.is_none()
                        && self.state.options.rail_name.is_none()
                        && !self.state.sources.is_empty()),
            )
    }
}
impl VibocerosApp {
    pub(super) fn initialize_sweep_prompt(&mut self, input: &str) {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "Sweep1")
        {
            return;
        }
        let Ok(options) = Options::parse(&input.split_whitespace().skip(1).collect::<Vec<_>>())
        else {
            return;
        };
        let sources = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect();
        let rail = self.sweep_reference(&options);
        self.sweep_session = Some(Session {
            state: State {
                options,
                sources,
                rail,
                finished: false,
            },
            undo: vec![],
            picking: None,
            value: None,
        });
    }
    fn sweep_reference(&self, options: &Options) -> Option<(ObjectId, GeometrySnapshot)> {
        let id = options.explicit_rail(&self.document).ok().flatten()?;
        self.document
            .object(id)
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| (id, o.geometry_snapshot().clone()))
    }
    pub(super) fn start_sweep_point_input(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Sweep1")
        }) {
            return false;
        }
        let Some(name) = words.last().copied().filter(|w| {
            ["Point", "StartPoint", "EndPoint"]
                .iter()
                .any(|n| w.trim_start_matches('_').eq_ignore_ascii_case(n))
        }) else {
            return false;
        };
        let base = words[..words.len() - 1].join(" ");
        let Ok(Some(description)) = self.commands.object_selection_prompt(&base) else {
            return false;
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let mut pending = PendingObjectCommand::selecting(description);
        pending.command_override = Some(base.clone());
        pending.selection_before = Some(before);
        self.object_prompt = Some(pending);
        self.initialize_sweep_prompt(&base);
        self.begin_sweep_point(name);
        self.command_input.clear();
        true
    }
    fn log_sweep_input(&mut self) {
        if let Some(p) = &self.sweep_session {
            self.push_log(format!(
                "Sweep1: {}. {}",
                p.hint(),
                p.state.options.arguments()
            ));
        }
    }
    fn sync_sweep_selection(&mut self) {
        let p = self.sweep_session.as_ref().unwrap();
        let ids = p.state.sources.iter().map(|(id, _)| *id).collect();
        let text = format!("Sweep1 {}", p.state.options.arguments());
        self.object_prompt.as_mut().unwrap().command_override = Some(text);
        self.restore_curve_selection(ids);
    }
    fn finish_sweep_input(&mut self) {
        let p = self.sweep_session.as_ref().unwrap();
        if p.profiles() == 0 {
            self.push_log("Select a rail and at least one Sweep1 profile".into());
            return;
        }
        let valid = p
            .state
            .sources
            .iter()
            .chain(p.state.rail.iter())
            .all(|(id, snapshot)| {
                self.document.is_object_selectable(*id)
                    && self
                        .document
                        .object(*id)
                        .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            });
        let reference = self.sweep_reference(&p.state.options);
        let same_reference =
            reference.as_ref().map(|(id, _)| *id) == p.state.rail.as_ref().map(|(id, _)| *id);
        if !valid || !same_reference {
            self.cancel_object_prompt(false);
            self.push_log("Sweep1 sources changed; select them again".into());
            return;
        }
        self.sync_sweep_selection();
        let session = self.sweep_session.take().unwrap();
        let pending = self.object_prompt.take().unwrap();
        if !self.try_execute_command(&format!("Sweep1 {}", session.state.options.arguments())) {
            self.sweep_session = Some(session);
            self.object_prompt = Some(pending);
            self.log_sweep_input();
        }
    }
    pub(super) fn clear_sweep_input(&mut self) {
        self.sweep_session = None;
        if self.active_command == Some(InteractiveCommand::SweepPoint) {
            self.active_command = None;
            self.drafting_plane = None;
        }
    }
}
