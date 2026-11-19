//! Chain picks for FilletEdge, ChamferEdge, and BlendEdge.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::ComponentSelectionKind;
use viboceros_geometry::{ChainContinuity, ChainDirection, CurveChainOptions};

impl VibocerosApp {
    pub(in crate::app) fn picking_edge_blend_chain(&self) -> bool {
        self.chain_prompt.as_ref().is_some_and(Prompt::edge_blend)
    }

    pub(in crate::app) fn begin_edge_blend_chain(&mut self, input: &str) {
        self.command_input.clear();
        if self.picking_edge_blend_faces() {
            self.push_log("Press Enter to finish picking faces before starting a chain".into());
            return;
        }
        let tolerance = self.document.tolerance();
        let parsed = parse_options(
            input,
            self.commands.chain_selection_options(&self.document),
            tolerance,
        );
        match parsed {
            Ok(options) => {
                if self.open_edge_blend_chain(options) {
                    self.commands
                        .remember_chain_selection_options(options)
                        .expect("validated chain options");
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn open_edge_blend_chain(&mut self, options: CurveChainOptions) -> bool {
        self.component_selection.clear_choices();
        match Session::new_for_edge_blend(
            &self.document,
            self.commands.component_selection(),
            options,
        ) {
            Ok(session) => {
                let prompt = Prompt::new(session, Purpose::EdgeBlend);
                self.push_log(prompt.hint().into());
                self.chain_prompt = Some(prompt);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(in crate::app) fn pick_edge_blend_chain(&mut self, picks: Vec<ComponentPick>) {
        let standalone_pick = !self.picking_edge_blend_chain();
        if standalone_pick {
            let options = CurveChainOptions {
                continuity: ChainContinuity::Tangency,
                direction: ChainDirection::Both,
                ..self.commands.chain_selection_options(&self.document)
            };
            if !self.open_edge_blend_chain(options) {
                return;
            }
        }
        let Some(prompt) = &self.chain_prompt else {
            return;
        };
        let targets = picks
            .into_iter()
            .filter(|pick| pick.kind == ComponentSelectionKind::BrepEdge)
            .map(|pick| Target::Edge(pick.object, pick.index))
            .filter(|&target| prompt.session.contains(target))
            .collect::<Vec<_>>();
        self.pick_chain(targets);
        if standalone_pick
            && self
                .chain_prompt
                .as_ref()
                .is_some_and(|p| p.choices.is_empty() && !p.session.choosing_branch())
        {
            self.finish_chain(true);
        }
    }
}
