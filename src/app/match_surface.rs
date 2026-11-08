//! Owned MatchSrf boundary contacts and a disposable preview document.
use super::*;
use crate::viewport::EdgePick;
use edge_commands::EdgePrompt;
use viboceros_command::match_surface::{Edge, Options, Prepared, parse, prepare};

#[derive(Clone, Debug)]
pub(super) struct Prompt {
    edges: Vec<Edge>,
    candidates: Vec<(EdgePick, Edge)>,
    options: Options,
    menu: Option<&'static str>,
    prepared: Option<Prepared>,
    scene: Option<Document>,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.edges.len() < 2
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub(super) fn hint(&self) -> &'static str {
        if !self.candidates.is_empty() {
            "Choose an edge number; Esc cancels"
        } else if self.edges.is_empty() {
            "Pick the untrimmed surface edge to change; Esc cancels"
        } else if self.edges.len() == 1 {
            "Pick the target edge near the corresponding end; Esc cancels"
        } else if self.menu.is_some() {
            "Choose an option value; Esc cancels"
        } else {
            "Set Mode, PreserveOtherEnd, MatchByClosestPoints, RefineMatch or IsocurveDirection; Enter accepts, Esc cancels"
        }
    }
    pub(super) fn highlights(&self) -> Vec<EdgePick> {
        self.edges
            .iter()
            .map(|e| EdgePick {
                object: e.object(),
                edge: e.index(),
            })
            .chain(self.candidates.iter().map(|(p, _)| *p))
            .collect()
    }
    pub(super) fn choices(&self) -> Vec<String> {
        if !self.candidates.is_empty() {
            return (1..=self.candidates.len()).map(|n| n.to_string()).collect();
        }
        if self.selecting() {
            return vec![];
        }
        match self.menu {
            Some("Mode") => vec!["Position", "Tangency", "Curvature"],
            Some("PreserveOtherEnd") => vec!["Nothing", "Position", "Tangency", "Curvature"],
            Some("IsocurveDirection") => {
                vec!["Automatic", "Keep", "MatchTarget", "Perpendicular"]
            }
            Some(_) => vec!["Yes", "No"],
            None => vec![
                "Mode",
                "PreserveOtherEnd",
                "MatchByClosestPoints",
                "RefineMatch",
                "IsocurveDirection",
                "Accept",
            ],
        }
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
    fn current(&self, doc: &Document) -> bool {
        self.edges
            .iter()
            .chain(self.candidates.iter().map(|(_, e)| e))
            .all(|e| e.current(doc))
    }
}
impl VibocerosApp {
    pub(super) fn start_match_surface(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("MatchSrf")
        }) {
            return false;
        }
        // Explicit object/edge/pick triples are handled by the public command.
        if words.get(1).is_some_and(|w| w.parse::<ObjectId>().is_ok()) {
            return false;
        }
        let options = match parse(&words[1..], self.commands.match_surface_options()) {
            Ok(o) => o,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.commands.remember_match_surface_options(options);
        self.edge_prompt = Some(EdgePrompt::Match(Box::new(Prompt {
            edges: vec![],
            candidates: vec![],
            options,
            menu: None,
            prepared: None,
            scene: None,
        })));
        self.command_input.clear();
        self.log_match_surface();
        true
    }
    fn log_match_surface(&mut self) {
        if let Some(EdgePrompt::Match(p)) = &self.edge_prompt {
            self.push_log(format!(
                "MatchSrf: {} ({})",
                p.hint(),
                p.options.arguments()
            ));
        }
    }
    pub(super) fn validate_match_surface(&mut self) {
        if let Some(EdgePrompt::Match(p)) = &self.edge_prompt
            && !p.current(&self.document)
        {
            self.finish_edge_command(false);
            self.push_log(
                "MatchSrf sources or model settings changed; pick the edges again".into(),
            );
        }
    }
    pub(super) fn accept_match_contacts(&mut self, contacts: Vec<(EdgePick, Point3)>) {
        self.validate_match_surface();
        let Some(EdgePrompt::Match(p)) = &mut self.edge_prompt else {
            return;
        };
        if !p.selecting() {
            return;
        }
        let mut candidates = vec![];
        let mut errors = vec![];
        for (pick, point) in contacts {
            if p.edges.iter().any(|e| e.object() == pick.object) {
                continue;
            }
            match Edge::capture(&self.document, pick.object, pick.edge, point) {
                Ok(edge) => candidates.push((pick, edge)),
                Err(error) => errors.push(format!("Error: {error}")),
            }
        }
        p.candidates = candidates;
        let single = p.candidates.len() == 1;
        for message in errors {
            self.push_log(message);
        }
        if single {
            self.choose_match_edge(0);
        } else {
            self.log_match_surface();
        }
    }
    fn choose_match_edge(&mut self, index: usize) {
        let Some(EdgePrompt::Match(p)) = &mut self.edge_prompt else {
            return;
        };
        let Some((_, edge)) = p.candidates.get(index).cloned() else {
            return;
        };
        p.edges.push(edge);
        p.candidates.clear();
        if p.selecting() {
            self.log_match_surface();
        } else {
            self.update_match_preview();
        }
    }
    fn update_match_preview(&mut self) {
        let Some(EdgePrompt::Match(p)) = &mut self.edge_prompt else {
            return;
        };
        p.prepared = None;
        p.scene = None;
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let prepared = prepare(
                &self.document,
                p.edges.clone().try_into().unwrap(),
                p.options,
            )?;
            let mut scene = self.document.clone();
            prepared.commit(&mut scene)?;
            scene.clear_history()?;
            Ok((prepared, scene))
        })();
        match result {
            Ok((prepared, scene)) => {
                p.prepared = Some(prepared);
                p.scene = Some(scene);
            }
            Err(error) => self.push_log(format!("Preview error: {error}")),
        }
        self.log_match_surface();
    }
    pub(super) fn continue_match_surface(&mut self, input: &str) -> bool {
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.finish_edge_command(true);
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.finish_edge_command(true);
            return false;
        }
        self.validate_match_surface();
        let Some(EdgePrompt::Match(p)) = &mut self.edge_prompt else {
            return true;
        };
        self.command_input.clear();
        if p.selecting() {
            if input.is_empty() {
                self.finish_edge_command(true);
            } else if let Some(index) = input.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) {
                self.choose_match_edge(index);
            } else {
                self.log_match_surface();
            }
            return true;
        }
        if p.menu.is_some() && input.is_empty() {
            p.menu = None;
            self.log_match_surface();
            return true;
        }
        if p.menu.is_none() && (input.is_empty() || input.eq_ignore_ascii_case("Accept")) {
            let result = p
                .prepared
                .as_ref()
                .ok_or(viboceros_command::CommandError::Usage(
                    "No valid MatchSrf preview",
                ))
                .and_then(|prepared| prepared.commit(&mut self.document));
            match result {
                Ok(message) => {
                    self.edge_prompt = None;
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if p.menu.is_none()
            && let Some(key) = [
                "Mode",
                "PreserveOtherEnd",
                "MatchByClosestPoints",
                "RefineMatch",
                "IsocurveDirection",
            ]
            .into_iter()
            .find(|key| key.eq_ignore_ascii_case(input.trim_start_matches('_')))
        {
            p.menu = Some(key);
            self.log_match_surface();
            return true;
        }
        let arguments = p
            .menu
            .map_or_else(|| input.to_owned(), |key| format!("{key}={input}"));
        match parse(&arguments.split_whitespace().collect::<Vec<_>>(), p.options) {
            Ok(options) => {
                p.options = options;
                p.menu = None;
                self.commands.remember_match_surface_options(options);
                self.update_match_preview();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}
