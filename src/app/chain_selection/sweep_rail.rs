//! A nested chain picker that returns a sweep rail without altering selection.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn begin_sweep_rail_chain(
        &mut self,
        owner: SweepRailOwner,
        input: &str,
        excluded: &[Target],
    ) {
        let mut auto_chain = *self.sweep_auto_chain(owner);
        let mut other = Vec::new();
        for word in input.split_whitespace() {
            if let Some((key, value)) = word.split_once('=')
                && key
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("AutoChain")
            {
                let Some(auto) = auto_value(value) else {
                    self.push_log("Use AutoChain=Yes or AutoChain=No".into());
                    return;
                };
                auto_chain = auto;
            } else {
                other.push(word);
            }
        }
        let result = parse_options(
            &other.join(" "),
            self.commands.chain_selection_options(&self.document),
            self.document.tolerance(),
        )
        .and_then(|options| Session::new_for_rail(&self.document, options, excluded));
        match result {
            Ok(session) => {
                *self.sweep_auto_chain(owner) = auto_chain;
                self.commands
                    .remember_chain_selection_options(session.options())
                    .expect("validated chain options");
                let mut prompt = Prompt::new(session, Purpose::SweepRail(owner));
                prompt.auto_chain = auto_chain;
                self.push_log(prompt.hint().into());
                self.chain_prompt = Some(prompt);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn continue_sweep_rail_chain(&mut self, input: &str) -> bool {
        let Some(prompt) = self.chain_prompt.as_mut() else {
            return false;
        };
        let Purpose::SweepRail(owner) = prompt.purpose else {
            return false;
        };
        let name = input.trim().trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("AutoChain") {
            prompt.pending = Some("AutoChain".into());
            self.push_log("Choose AutoChain Yes or No".into());
        } else if let Some(value) = name
            .split_once('=')
            .filter(|(key, _)| key.eq_ignore_ascii_case("AutoChain"))
            .map(|(_, value)| value)
            .or_else(|| {
                (prompt.pending.as_deref() == Some("AutoChain")
                    && !["Cancel", "!", "Undo"]
                        .iter()
                        .any(|word| name.eq_ignore_ascii_case(word)))
                .then_some(name)
            })
        {
            if let Some(auto) = auto_value(value) {
                prompt.auto_chain = auto;
                prompt.pending = None;
                prompt.session.discard_proposal();
                prompt.refresh_preview();
                *self.sweep_auto_chain(owner) = auto;
            } else {
                self.push_log("Use AutoChain=Yes or AutoChain=No".into());
            }
        } else if let Some(target) = rail_target(name) {
            self.pick_chain(vec![target]);
        } else if name.eq_ignore_ascii_case("Undo")
            && prompt.pending.as_deref() == Some("AutoChain")
        {
            prompt.pending = None;
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    fn sweep_auto_chain(&mut self, owner: SweepRailOwner) -> &mut bool {
        match owner {
            SweepRailOwner::Sweep1 => &mut self.sweep_session.as_mut().unwrap().chain_auto,
            SweepRailOwner::Sweep2 => &mut self.sweep_two_rails.as_mut().unwrap().chain_auto,
        }
    }
}

fn auto_value(value: &str) -> Option<bool> {
    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

fn rail_target(input: &str) -> Option<Target> {
    if let Some((key, value)) = input.split_once('=') {
        if key.eq_ignore_ascii_case("Edge") {
            return viboceros_command::curve_reference::parse_edge(value)
                .map(|(id, edge)| Target::Edge(id, edge));
        }
        if key.eq_ignore_ascii_case("Seed") {
            return value.parse().ok().map(Target::Curve);
        }
    }
    input.parse().ok().map(Target::Curve)
}
