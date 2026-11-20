use super::*;
use crate::viewport::EdgePick;

pub(super) fn is_chain_option(name: &str) -> bool {
    let name = name.trim_start_matches(['_', '-']);
    name.eq_ignore_ascii_case("ChainEdges") || name.eq_ignore_ascii_case("Chain")
}

impl VibocerosApp {
    pub(in crate::app) fn picking_sweep_edges(&self) -> bool {
        self.plane_prompt.is_none()
            && self.chain_prompt.is_none()
            && self
                .sweep_session
                .as_ref()
                .is_some_and(Session::awaiting_rail)
    }

    pub(in crate::app) fn select_sweep_edge_click(&mut self, picks: &[EdgePick]) -> bool {
        if !self.picking_sweep_edges() {
            return false;
        }
        if let Some(pick) = picks.first() {
            self.update_sweep_options(&format!("RailEdge={},{}", pick.object, pick.edge));
        }
        true
    }

    pub(super) fn start_sweep_one_rail_chain(&mut self, options: &str) {
        if !self.validate_sweep_sources() {
            return;
        }
        let session = self.sweep_session.as_ref().unwrap();
        if !session.awaiting_rail() {
            self.push_log("Sweep1 ChainEdges is available while selecting the rail".into());
            return;
        }
        let excluded = session
            .state
            .sources
            .iter()
            .map(|(id, _)| Target::Curve(*id))
            .collect::<Vec<_>>();
        self.begin_sweep_rail_chain(chain_selection::SweepRailOwner::Sweep1, options, &excluded);
    }

    pub(in crate::app) fn accept_sweep_one_rail_chain(
        &mut self,
        path: &[(Target, bool)],
        gap: f64,
    ) -> bool {
        if self.sweep_session.is_none() || !self.validate_sweep_sources() {
            return true;
        }
        let rail = match Rail::chain(&self.document, path, gap) {
            Ok(rail) => rail,
            Err(error) => {
                self.push_log(format!("Sweep1: {error}"));
                return false;
            }
        };
        let session = self.sweep_session.as_mut().unwrap();
        if session.state.options.surface_aligned && !rail.supports_alignment() {
            self.push_log(
                "Sweep1 AlignWithSurface requires an edge with one supporting face".into(),
            );
            return false;
        }
        session.undo.push(session.state.clone());
        session.state.rail = Some(rail);
        self.sync_sweep_selection();
        self.log_sweep_input();
        true
    }
}
