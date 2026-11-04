//! Standalone mixed curve/edge chains retain live selection and pending redo.
use super::*;
use viboceros_command::chain_selection::{Session, Target, parse_options};
pub(super) const HINT: &str = "Select connected curves or surface edges; Undo / Next / All / ChainContinuity / Direction / GapTolerance / AngleTolerance; Enter accepts, Esc restores selection";
#[derive(Debug)]
pub(super) struct Prompt {
    session: Session,
    choices: Vec<Target>,
    preview: Vec<Target>,
    pending: Option<String>,
    defer_apply: bool,
}
impl Prompt {
    pub(super) fn hint(&self) -> &'static str {
        if self.session.choosing_branch() {
            "Select a branch to follow, or Enter to confirm the preview; Esc cancels"
        } else {
            HINT
        }
    }
    pub(super) fn highlights(&self) -> &[Target] {
        if self.choices.is_empty() {
            &self.preview
        } else {
            &self.choices
        }
    }
}
impl VibocerosApp {
    pub(super) fn start_chain(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SelChain")
        }) {
            return false;
        }
        let args = words.collect::<Vec<_>>();
        if args.iter().any(|s| {
            s.split_once('=')
                .is_some_and(|(k, _)| ["Seed", "Edge"].iter().any(|n| k.eq_ignore_ascii_case(n)))
        }) {
            return false;
        }
        let options = match parse_options(
            &args.join(" "),
            self.commands.chain_selection_options(&self.document),
            self.document.tolerance(),
        ) {
            Ok(o) => o,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        match Session::new(&self.document, self.commands.component_selection(), options) {
            Ok(session) => {
                self.commands
                    .remember_chain_selection_options(options)
                    .expect("validated chain options");
                self.chain_prompt = Some(Prompt {
                    session,
                    choices: Vec::new(),
                    preview: Vec::new(),
                    pending: None,
                    defer_apply: false,
                });
                self.push_log(format!("SelChain: {HINT}"));
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn pick_chain(&mut self, picks: Vec<Target>) {
        let Some(p) = &mut self.chain_prompt else {
            return;
        };
        if !p.session.current(&self.document) {
            self.validate_chain();
            return;
        }
        if picks.len() != 1 {
            p.choices = picks;
            let labels = p
                .choices
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let kind = match t {
                        Target::Curve(_) => "curve".to_owned(),
                        Target::Edge(_, edge) => format!("surface edge {}", edge + 1),
                    };
                    let label = self
                        .document
                        .object(t.object())
                        .and_then(|o| o.attributes().name())
                        .map_or(kind.clone(), |name| format!("{name} ({kind})"));
                    format!("{}: {label}", i + 1)
                })
                .collect::<Vec<_>>();
            if !labels.is_empty() {
                self.push_log(format!("SelChain: choose {}", labels.join("; ")));
            }
            return;
        }
        let result = p
            .session
            .pick_input(&self.document, picks[0])
            .and_then(|()| {
                p.session
                    .apply(&mut self.document, self.commands.component_selection())
            });
        if result.is_ok() {
            p.defer_apply = false;
        }
        p.preview = p.session.preview();
        p.choices.clear();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        }
        self.command_input.clear();
    }
    pub(super) fn cancel_chain_input(&mut self) {
        if let Some(p) = &mut self.chain_prompt
            && !p.choices.is_empty()
        {
            p.choices.clear();
            self.command_input.clear();
            return;
        }
        self.finish_chain(false);
    }
    pub(super) fn finish_chain(&mut self, accept: bool) {
        let Some(p) = self.chain_prompt.take() else {
            return;
        };
        let accept = accept && p.session.has_seed();
        let result = if accept {
            p.session
                .apply(&mut self.document, self.commands.component_selection())
        } else {
            p.session
                .cancel(&mut self.document, self.commands.component_selection())
        };
        match result {
            Ok(()) => self.push_log(if accept {
                format!("Selected {} chain segment(s)", p.session.targets().len())
            } else {
                "SelChain canceled".into()
            }),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
    }
    pub(super) fn validate_chain(&mut self) {
        if self
            .chain_prompt
            .as_ref()
            .is_some_and(|p| !p.session.current(&self.document))
        {
            self.finish_chain(false);
            self.push_log("Error: chain sources or tolerance changed; select again".into());
        }
    }
    pub(super) fn continue_chain(&mut self, input: &str) -> bool {
        let Some(p) = &mut self.chain_prompt else {
            return false;
        };
        let input = input.trim().trim_start_matches(['_', '-']);
        if !p.choices.is_empty() {
            if let Ok(n) = input.parse::<usize>() {
                if let Some(&target) = n.checked_sub(1).and_then(|i| p.choices.get(i)) {
                    self.pick_chain(vec![target]);
                } else {
                    let count = p.choices.len();
                    self.push_log(format!("Choose a chain target from 1 to {count}"));
                    self.command_input.clear();
                }
                return true;
            }
            if input.is_empty() {
                p.choices.clear();
                self.command_input.clear();
                return true;
            }
        }
        if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            match p.session.enter(&self.document) {
                Ok(true) => self.finish_chain(true),
                Ok(false) => {
                    p.preview = p.session.preview();
                    let result = if p.defer_apply {
                        Ok(())
                    } else {
                        p.session
                            .apply(&mut self.document, self.commands.component_selection())
                    };
                    self.command_input.clear();
                    if let Err(e) = result {
                        self.push_log(format!("Error: {e}"));
                    }
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.command_input.clear();
                }
            }
            return true;
        }
        if input == "!" || input.eq_ignore_ascii_case("Cancel") {
            self.finish_chain(false);
            return true;
        }
        if p.session.choosing_branch() {
            if let Ok(n) = input.parse::<usize>()
                && let Some(target) = n
                    .checked_sub(1)
                    .and_then(|i| p.session.branch_choices().get(i).copied())
            {
                self.pick_chain(vec![target]);
                return true;
            }
            let result = if input.eq_ignore_ascii_case("All") {
                // Native global SelAll remains visible through proposal confirmation;
                // finishing the getter restores its internal chain selection.
                p.defer_apply = true;
                self.commands
                    .execute(&mut self.document, "SelAll")
                    .map(|_| ())
            } else if input.eq_ignore_ascii_case("Undo") {
                let _ = self.document.undo();
                Ok(())
            } else {
                Err(viboceros_command::CommandError::Usage(
                    "Select a branch to follow, or Enter to confirm the proposal",
                ))
            };
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
            self.command_input.clear();
            return true;
        }
        if p.pending.is_none()
            && input
                .split_whitespace()
                .next()
                .is_some_and(|s| self.commands.recognizes(s))
            && !input.eq_ignore_ascii_case("Undo")
        {
            self.finish_chain(false);
            return false;
        }
        let combined = p.pending.take().map(|name| format!("{name}={input}"));
        let input = combined.as_deref().unwrap_or(input);
        if [
            "ChainContinuity",
            "Direction",
            "GapTolerance",
            "AngleTolerance",
        ]
        .iter()
        .any(|n| input.eq_ignore_ascii_case(n))
        {
            p.pending = Some(input.to_owned());
            self.command_input.clear();
            return true;
        }
        let result = if input.eq_ignore_ascii_case("Undo") {
            p.session.undo(&self.document)
        } else if input.eq_ignore_ascii_case("Next") || input.eq_ignore_ascii_case("All") {
            p.session
                .extend(&self.document, input.eq_ignore_ascii_case("Next"))
        } else {
            parse_options(input, p.session.options(), self.document.tolerance())
                .and_then(|o| p.session.set_options(o))
        };
        let result = result.and_then(|()| {
            p.session
                .apply(&mut self.document, self.commands.component_selection())
        });
        if result.is_ok() {
            p.defer_apply = false;
        }
        p.preview = p.session.preview();
        self.commands
            .remember_chain_selection_options(p.session.options())
            .expect("validated chain options");
        p.choices.clear();
        self.command_input.clear();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        }
        true
    }
}
