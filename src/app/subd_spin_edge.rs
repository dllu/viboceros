//! Immediate edge-spin previews with command-local Undo and one final history step.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::{
    ComponentSelectionKind,
    subd_spin_edge::{Direction, SpinSession, Target},
};
pub(super) struct Prompt {
    session: SpinSession,
    pub scene: Document,
}
impl Prompt {
    fn refresh(&mut self, document: &Document) -> Result<(), viboceros_command::CommandError> {
        self.session.validate(document)?;
        let mut scene = document.clone();
        self.session.apply_preview(&mut scene)?;
        self.scene = scene;
        Ok(())
    }
}
impl VibocerosApp {
    pub(super) fn picking_spin_edge(&self) -> bool {
        matches!(self.crease_prompt, Some(crease::Prompt::Spin(_)))
    }
    pub(super) fn try_start_spin_edge(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDSpinEdge")
        }) {
            return false;
        }
        let mut direction = Direction::default();
        if let Some(option) = words.next() {
            let (key, value) = if let Some(pair) = option.split_once('=') {
                pair
            } else {
                let Some(value) = words.next() else {
                    return false;
                };
                (option, value)
            };
            if !key
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Direction")
            {
                return false;
            }
            let Some(value) = Direction::parse(value) else {
                return false;
            };
            direction = value;
        }
        if words.next().is_some() {
            return false;
        }
        let mut targets = match self.component_selection.checked_picks(&self.document) {
            Ok(picks) => picks
                .into_iter()
                .filter(|p| p.kind == ComponentSelectionKind::SubDEdge)
                .map(|p| Target {
                    object: p.object,
                    edge: p.index,
                })
                .collect::<Vec<_>>(),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        if targets.len() > 1 {
            targets.clear();
        }
        self.cancel_interactive_command(false);
        let mut session = SpinSession::new(&self.document, direction);
        if let Some(&target) = targets.first() {
            match session
                .spin(&self.document, target)
                .and_then(|_| session.finish(&mut self.document))
            {
                Ok(out) => {
                    self.component_selection
                        .release_objects(&out.processed_objects);
                    self.push_log(out.message);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.component_selection.clear();
            self.document.clear_selection();
            let prompt = Prompt {
                session,
                scene: self.document.clone(),
            };
            self.crease_prompt = Some(crease::Prompt::Spin(Box::new(prompt)));
            self.push_log(
                "SubDSpinEdge: pick a SubD edge (Direction SpinEdgeAgain Undo); Enter finishes"
                    .into(),
            );
        }
        self.command_input.clear();
        true
    }
    pub(super) fn pick_spin_edge(&mut self, picks: &[ComponentPick]) {
        let [pick] = picks else {
            if !picks.is_empty() {
                self.push_log("Pick one SubD edge to spin".into());
            }
            return;
        };
        if pick.kind != ComponentSelectionKind::SubDEdge {
            return;
        }
        let Some(crease::Prompt::Spin(mut prompt)) = self.crease_prompt.take() else {
            return;
        };
        let result = prompt
            .session
            .spin(
                &self.document,
                Target {
                    object: pick.object,
                    edge: pick.index,
                },
            )
            .and_then(|changed| {
                prompt.refresh(&self.document)?;
                Ok(changed)
            });
        match result {
            Ok(true) => self
                .push_log("Spun SubD edge (SpinEdgeAgain Undo Direction); Enter finishes".into()),
            Ok(false) => self.push_log("This SubD edge cannot spin".into()),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.crease_prompt = Some(crease::Prompt::Spin(prompt));
    }
    pub(super) fn continue_spin_edge(&mut self, input: &str) -> bool {
        let token = input.trim().trim_start_matches(['_', '-']);
        if token.is_empty()
            || ["Enter", "Cancel"]
                .iter()
                .any(|s| token.eq_ignore_ascii_case(s))
        {
            self.finish_spin_edge();
            self.command_input.clear();
            return true;
        }
        let name = token.split_whitespace().next().unwrap_or("");
        if !["Undo", "SpinEdgeAgain", "Direction"]
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
            && self.commands.recognizes(name)
        {
            self.finish_spin_edge();
            return false;
        }
        let Some(crease::Prompt::Spin(mut prompt)) = self.crease_prompt.take() else {
            return false;
        };
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            prompt.session.validate(&self.document)?;
            if token.eq_ignore_ascii_case("SpinEdgeAgain") {
                prompt.session.spin_again(&self.document)?;
            } else if token.eq_ignore_ascii_case("Undo") {
                prompt.session.undo_spin();
            } else if token.eq_ignore_ascii_case("Direction") {
                prompt.session.direction = match prompt.session.direction {
                    Direction::Clockwise => Direction::CounterClockwise,
                    Direction::CounterClockwise => Direction::Clockwise,
                };
            } else {
                let value = token
                    .split_once('=')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("Direction"))
                    .map(|(_, value)| value)
                    .or_else(|| {
                        token
                            .split_once(' ')
                            .filter(|(key, _)| key.eq_ignore_ascii_case("Direction"))
                            .map(|(_, value)| value)
                    });
                if let Some(direction) = value.and_then(Direction::parse) {
                    prompt.session.direction = direction;
                } else {
                    return Err(viboceros_command::CommandError::Usage(
                        "SubDSpinEdge: Direction SpinEdgeAgain Undo Enter",
                    ));
                }
            }
            prompt.refresh(&self.document)
        })();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        }
        self.crease_prompt = Some(crease::Prompt::Spin(prompt));
        self.command_input.clear();
        true
    }
    pub(super) fn finish_spin_edge(&mut self) {
        if let Some(crease::Prompt::Spin(prompt)) = self.crease_prompt.take() {
            match prompt.session.finish(&mut self.document) {
                Ok(out) => {
                    self.component_selection
                        .release_objects(&out.processed_objects);
                    self.push_log(out.message);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.component_selection.clear();
        }
    }
}
