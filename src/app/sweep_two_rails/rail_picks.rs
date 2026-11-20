use super::*;
use crate::viewport::EdgePick;

impl VibocerosApp {
    pub(in crate::app) fn picking_sweep_two_rail_edges(&self) -> bool {
        self.plane_prompt.is_none()
            && self.chain_prompt.is_none()
            && self
                .sweep_two_rails
                .as_ref()
                .is_some_and(|session| session.selecting() && session.rail_count() < 2)
    }

    pub(in crate::app) fn select_sweep_two_rail_edge_click(
        &mut self,
        picks: &[EdgePick],
        modifiers: egui::Modifiers,
    ) -> bool {
        if !self.picking_sweep_two_rail_edges() {
            return false;
        }
        if let Some(pick) = picks.first() {
            self.select_sweep_two_rail_edge(
                Target::Edge(pick.object, pick.edge),
                None,
                modifiers.ctrl && !modifiers.shift,
            );
        }
        true
    }

    pub(super) fn select_sweep_two_rail_edge_argument(&mut self, value: &str) {
        if !self.picking_sweep_two_rail_edges() {
            self.push_log("Sweep2: select surface edges while choosing rails".into());
            return;
        }
        match RailReference::parse_edge(value) {
            Ok(reference) => {
                self.select_sweep_two_rail_edge(reference.target, reference.face, false)
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn select_sweep_two_rail_edge(&mut self, target: Target, face: Option<usize>, remove: bool) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        let existing = session
            .state
            .rails
            .iter()
            .position(|rail| rail.targets().any(|piece| piece == target));
        match (remove, existing) {
            (true, Some(index)) if session.state.rails[index].targets().count() == 1 => {
                session.undo.push(session.state.clone());
                session.state.rails.remove(index);
            }
            (false, None) => {
                match Rail::capture_reference(&self.document, RailReference { target, face }) {
                    Ok(rail) => {
                        session.undo.push(session.state.clone());
                        session.state.rails.push(rail);
                    }
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        return;
                    }
                }
            }
            _ => return,
        }
        self.sync_sweep_two_rail_picks();
        self.log_sweep_two_rails();
    }
}
