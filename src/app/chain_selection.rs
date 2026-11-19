//! Standalone mixed curve/edge chains retain live selection and pending redo.
use super::edge_fillet::edit as edge_edit;
use super::*;
use viboceros_command::chain_selection::{Session, Target, parse_options};
mod edge_blend;
pub(super) const HINT: &str = "Select connected curves or surface edges; Undo / Next / All / ChainContinuity / Direction / GapTolerance / AngleTolerance; Enter accepts, Esc restores selection";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Select,
    Measure,
    EdgeBlend,
}

#[derive(Debug)]
pub(super) struct Prompt {
    pub(super) session: Session,
    choices: Vec<Target>,
    preview: Vec<Target>,
    pending: Option<String>,
    defer_apply: bool,
    purpose: Purpose,
}
impl Prompt {
    fn new(session: Session, purpose: Purpose) -> Self {
        Self {
            session,
            choices: Vec::new(),
            preview: Vec::new(),
            pending: None,
            defer_apply: false,
            purpose,
        }
    }
    pub(super) fn edge_blend(&self) -> bool {
        self.purpose == Purpose::EdgeBlend
    }
    fn measuring(&self) -> bool {
        self.purpose == Purpose::Measure
    }
    fn document<'a>(
        &self,
        edit: &'a Option<edge_edit::Edit>,
        document: &'a Document,
    ) -> &'a Document {
        if self.edge_blend() {
            edge_edit::source_document(edit, document)
        } else {
            document
        }
    }

    fn document_mut<'a>(
        &self,
        edit: &'a mut Option<edge_edit::Edit>,
        document: &'a mut Document,
    ) -> &'a mut Document {
        if self.edge_blend() {
            edge_edit::source_document_mut(edit, document)
        } else {
            document
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.session.choosing_branch() {
            "Select a branch to follow, or Enter to confirm the preview; Esc cancels"
        } else if self.edge_blend() {
            "Pick connected edges. Options include ChainContinuity, Direction, GapTolerance, AngleTolerance, Undo, Next and All. Enter keeps the chain."
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
    pub(super) fn length_chain_available(&self) -> bool {
        self.object_prompt.as_ref().is_some_and(|p| {
            p.description.command == "Length"
                && p.phase == object_selection::ObjectPromptPhase::Selecting
                && !p.subcurve_measurement
        })
    }
    pub(super) fn start_length_chain(&mut self, input: &str) -> bool {
        self.length_chain_available() && self.start_chain(input)
    }
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
        let private = self.nested_length_wants_source();
        if !private && !self.length_chain_available() {
            self.cancel_interactive_command(false);
        }
        self.length_choices = None;
        let session = if private {
            Session::new_for_measurement(&self.document, options)
        } else {
            Session::new(&self.document, self.commands.component_selection(), options)
        };
        match session {
            Ok(session) => {
                self.commands
                    .remember_chain_selection_options(options)
                    .expect("validated chain options");
                let purpose = if private {
                    Purpose::Measure
                } else {
                    Purpose::Select
                };
                self.chain_prompt = Some(Prompt::new(session, purpose));
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
        if !p
            .session
            .current(p.document(&self.edge_blend_edit, &self.document))
        {
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
                self.push_log(format!("Choose {}", labels.join("; ")));
            }
            return;
        }
        let document = p.document_mut(&mut self.edge_blend_edit, &mut self.document);
        let result = p.session.pick_input(document, picks[0]).and_then(|()| {
            if p.measuring() {
                Ok(())
            } else {
                p.session
                    .apply(document, self.commands.component_selection())
            }
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
        let nested = self.length_chain_available();
        let edge_blend = self.picking_edge_blend_chain();
        self.finish_chain(false);
        if edge_blend {
            self.finish_edge_fillet(false);
        } else if nested {
            self.cancel_object_prompt(true);
        }
    }
    pub(super) fn finish_chain(&mut self, accept: bool) {
        let Some(p) = self.chain_prompt.take() else {
            return;
        };
        let accept = accept && p.session.has_seed();
        if p.measuring() {
            if accept && p.session.current(&self.document) {
                self.add_nested_length_targets(p.session.targets(), SelectionMode::Add, true);
            } else {
                self.push_log("SelChain canceled".into());
            }
            self.command_input.clear();
            self.log_nested_length();
            return;
        }
        let document = p.document_mut(&mut self.edge_blend_edit, &mut self.document);
        let result = if accept {
            p.session
                .apply(document, self.commands.component_selection())
        } else {
            p.session
                .cancel(document, self.commands.component_selection())
        };
        if result.is_ok() && accept && p.edge_blend() {
            self.track_edge_blend_picks(
                p.session.targets().into_iter().filter_map(|target| {
                    let Target::Edge(object, index) = target else {
                        return None;
                    };
                    Some(crate::viewport::ComponentPick {
                        object,
                        index,
                        kind: viboceros_command::ComponentSelectionKind::BrepEdge,
                    })
                }),
                viboceros_command::component_selection::ComponentAction::Add,
            );
        }
        match result {
            Ok(()) => self.push_log(if accept {
                format!("Selected {} chain segment(s)", p.session.targets().len())
            } else if p.edge_blend() {
                "ChainEdges canceled".into()
            } else {
                "SelChain canceled".into()
            }),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        if p.edge_blend() {
            self.log_edge_blend_prompt();
        } else if self.length_chain_available() {
            self.log_object_prompt();
        }
    }
    pub(super) fn validate_chain(&mut self) {
        if self.chain_prompt.as_ref().is_some_and(|p| {
            !p.session
                .current(p.document(&self.edge_blend_edit, &self.document))
        }) {
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
            let document = p.document_mut(&mut self.edge_blend_edit, &mut self.document);
            match p.session.enter(document) {
                Ok(true) => self.finish_chain(true),
                Ok(false) => {
                    p.preview = p.session.preview();
                    let result = if p.measuring() || p.defer_apply {
                        Ok(())
                    } else {
                        p.session
                            .apply(document, self.commands.component_selection())
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
            if p.edge_blend() {
                self.cancel_chain_input();
            } else {
                self.finish_chain(false);
            }
            return true;
        }
        if p.edge_blend() && input.eq_ignore_ascii_case("Undo") {
            p.session.discard_proposal();
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
            let result = if p.measuring() && input.eq_ignore_ascii_case("All") {
                p.session.extend(&self.document, false)
            } else if p.measuring() && input.eq_ignore_ascii_case("Undo") {
                p.session.undo(&self.document)
            } else if p.edge_blend() {
                Err(viboceros_command::CommandError::Usage(
                    "Select a branch to follow, or Enter to confirm the proposal",
                ))
            } else if input.eq_ignore_ascii_case("All") {
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
            if let Some(p) = &mut self.chain_prompt
                && p.measuring()
            {
                p.preview = p.session.preview();
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
        let document = p.document_mut(&mut self.edge_blend_edit, &mut self.document);
        let result = if input.eq_ignore_ascii_case("Undo") {
            p.session.undo(document)
        } else if input.eq_ignore_ascii_case("Next") || input.eq_ignore_ascii_case("All") {
            p.session
                .extend(document, input.eq_ignore_ascii_case("Next"))
        } else {
            parse_options(input, p.session.options(), document.tolerance())
                .and_then(|o| p.session.set_options(o))
        };
        let result = result.and_then(|()| {
            if p.measuring() {
                Ok(())
            } else {
                p.session
                    .apply(document, self.commands.component_selection())
            }
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
