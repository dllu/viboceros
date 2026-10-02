//! Preselection applies immediately; command-first edge selection waits for Enter.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::{ComponentSelectionKind, UnjoinEdgeSelection};
impl VibocerosApp {
    pub(super) fn try_start_unjoin_command(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("UnjoinEdge")
        }) || words.next().is_some()
        {
            return false;
        }
        let picks = self
            .component_selection
            .valid_picks(&self.document)
            .into_iter()
            .filter(|pick| pick.kind == ComponentSelectionKind::BrepEdge)
            .map(|pick| (pick.object, pick.index));
        let prepared = UnjoinEdgeSelection::prepare_preselected(&self.document, picks);
        self.cancel_interactive_command(false);
        self.component_selection.clear();
        self.document.clear_selection();
        self.command_input.clear();
        match prepared {
            Ok(selection) if selection.changes_geometry() => {
                self.commit_unjoin_selection(selection)
            }
            Ok(_) => {
                self.unjoin_prompt = Some(self.document.tolerance());
                self.push_log("UnjoinEdge: select joined edges; Enter applies, Esc cancels".into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(super) fn try_continue_unjoin_command(&mut self, input: &str) -> bool {
        if self.unjoin_prompt.is_none() {
            return false;
        }
        let input = input.trim();
        if input.is_empty() {
            self.finish_unjoin_command(true);
            return true;
        }
        if ["Cancel", "None"]
            .iter()
            .any(|name| input.trim_start_matches('_').eq_ignore_ascii_case(name))
        {
            self.finish_unjoin_command(false);
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.finish_unjoin_command(false);
            return false;
        }
        self.push_log("Select joined edges in a viewport; Enter applies, Esc cancels".into());
        self.command_input.clear();
        true
    }
    pub(super) fn unjoinable_picks(&self, picks: Vec<ComponentPick>) -> Vec<ComponentPick> {
        let mut joined = std::collections::BTreeMap::new();
        picks
            .into_iter()
            .filter(|pick| {
                if pick.kind != ComponentSelectionKind::BrepEdge
                    || !self.document.is_object_selectable(pick.object)
                {
                    return false;
                }
                joined
                    .entry(pick.object)
                    .or_insert_with(|| {
                        match self
                            .document
                            .object(pick.object)
                            .map(|object| object.geometry())
                        {
                            Some(Geometry::Brep(brep)) => brep.edges_shared_by_distinct_faces(),
                            _ => Vec::new(),
                        }
                    })
                    .get(pick.index)
                    .copied()
                    .unwrap_or(false)
            })
            .collect()
    }
    pub(super) fn finish_unjoin_command(&mut self, apply: bool) {
        let Some(tolerance) = self.unjoin_prompt.take() else {
            return;
        };
        let picks = self.component_selection.checked_picks(&self.document);
        self.component_selection.clear();
        self.command_input.clear();
        self.document.clear_selection();
        if !apply {
            self.push_log("Cancelled UnjoinEdge".into());
            return;
        }
        if tolerance != self.document.tolerance() {
            self.push_log("Error: tolerance changed; select the edges again".into());
            return;
        }
        match picks {
            Ok(picks) => match UnjoinEdgeSelection::prepare(
                &self.document,
                picks.into_iter().map(|pick| (pick.object, pick.index)),
            ) {
                Ok(selection) => self.commit_unjoin_selection(selection),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    fn commit_unjoin_selection(&mut self, selection: UnjoinEdgeSelection) {
        match selection.commit(&mut self.document) {
            Ok(result) => self.push_log(format!(
                "Separated {} source(s), added {} component(s)",
                result.updated.len(),
                result.added.len()
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
