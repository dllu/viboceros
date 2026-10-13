//! Ordered edge picking and cached, cancellable surface-blend previews.
use super::*;
use crate::viewport::EdgePick;
use viboceros_command::blend_surface::{Options, Prepared, parse, prepare};

#[derive(Debug)]
pub(super) struct Prompt {
    picks: Vec<EdgePick>,
    sources: Vec<viboceros_document::GeometrySnapshot>,
    options: Options,
    candidates: Vec<EdgePick>,
    prepared: Option<Prepared>,
    scene: Option<Document>,
    tolerance: Tolerance,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.picks.len() < 2
    }
    pub(super) fn hint(&self) -> &'static str {
        if !self.candidates.is_empty() {
            "Type an edge number from the log, or pick again; Esc cancels"
        } else if self.picks.is_empty() {
            "Pick the first complete surface edge; Esc cancels"
        } else if self.picks.len() == 1 {
            "Pick the second complete surface edge; Esc cancels"
        } else {
            "Set Continuity1/2, Bulge1/2, Reverse2, Trim1/2 or Join; Enter accepts, Esc cancels"
        }
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub(super) fn highlights(&self) -> Vec<EdgePick> {
        self.picks.iter().chain(&self.candidates).copied().collect()
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.picks.iter().enumerate().all(|(i, p)| {
                doc.is_object_selectable(p.object)
                    && doc
                        .object(p.object)
                        .is_some_and(|o| self.sources[i].shares_storage_with(o.geometry_snapshot()))
            })
    }
}
impl VibocerosApp {
    pub(super) fn start_blend_surface(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("BlendSrf")
        }) {
            return false;
        }
        let options = match parse(&words[1..]) {
            Ok(options)
                if options.sources.is_some()
                    || options.preview
                    || words[1..].iter().any(|word| {
                        word.split_once('=').is_some_and(|(key, _)| {
                            ["Edge1", "Edge2"]
                                .iter()
                                .any(|name| key.trim_start_matches('_').eq_ignore_ascii_case(name))
                        })
                    }) =>
            {
                return false;
            }
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.blend_surface_prompt = Some(Prompt {
            picks: vec![],
            sources: vec![],
            options,
            candidates: vec![],
            prepared: None,
            scene: None,
            tolerance: self.document.tolerance(),
        });
        self.push_log(format!("> {input}"));
        self.log_blend_surface();
        true
    }
    fn log_blend_surface(&mut self) {
        if let Some(p) = &self.blend_surface_prompt {
            self.push_log(format!("BlendSrf: {}", p.hint()));
        }
    }
    pub(super) fn validate_blend_surface(&mut self) {
        if self
            .blend_surface_prompt
            .as_ref()
            .is_some_and(|p| !p.current(&self.document))
        {
            self.blend_surface_prompt = None;
            self.push_log("BlendSrf sources or tolerance changed; select the edges again".into());
        }
    }
    pub(super) fn accept_blend_edges(&mut self, picks: Vec<EdgePick>) {
        self.validate_blend_surface();
        let Some(p) = &mut self.blend_surface_prompt else {
            return;
        };
        if !p.selecting() {
            return;
        }
        p.candidates = picks;
        if p.candidates.len() == 1 {
            let pick = p.candidates[0];
            self.pick_blend_edge(pick);
        } else {
            let labels = p
                .candidates
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{}. object {} edge {}", i + 1, p.object, p.edge))
                .collect::<Vec<_>>();
            for label in labels {
                self.push_log(label);
            }
            self.log_blend_surface();
        }
    }
    fn pick_blend_edge(&mut self, pick: EdgePick) {
        let Some(object) = self.document.object(pick.object) else {
            return;
        };
        let p = self.blend_surface_prompt.as_mut().unwrap();
        if p.picks.iter().any(|old| old.object == pick.object) {
            self.push_log("Pick an edge on a different parent surface".into());
            return;
        }
        p.picks.push(pick);
        p.sources.push(object.geometry_snapshot().clone());
        p.candidates.clear();
        if p.picks.len() == 2 {
            self.update_blend_preview();
        } else {
            self.log_blend_surface();
        }
    }
    fn update_blend_preview(&mut self) {
        let Some(p) = self.blend_surface_prompt.as_mut() else {
            return;
        };
        p.prepared = None;
        p.scene = None;
        p.options.edge_indices = Some([p.picks[0].edge, p.picks[1].edge]);
        let ids = [p.picks[0].object, p.picks[1].object];
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let prepared = prepare(&self.document, ids, &p.options)?;
            let mut scene = self.document.clone();
            prepared.commit(&mut scene)?;
            scene.clear_history()?;
            Ok((prepared, scene))
        })();
        match result {
            Ok((prepared, scene)) => {
                let message = prepared.message();
                p.prepared = Some(prepared);
                p.scene = Some(scene);
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Preview error: {error}")),
        }
        self.log_blend_surface();
    }
    pub(super) fn continue_blend_surface(&mut self, input: &str) -> bool {
        let Some(p) = self.blend_surface_prompt.as_ref() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.blend_surface_prompt = None;
            self.command_input.clear();
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.blend_surface_prompt = None;
            return false;
        }
        if !p.current(&self.document) {
            self.validate_blend_surface();
            return true;
        }
        if p.selecting() {
            if let Ok(number) = input.parse::<usize>() {
                if let Some(pick) = number
                    .checked_sub(1)
                    .and_then(|i| p.candidates.get(i))
                    .copied()
                {
                    self.pick_blend_edge(pick);
                } else {
                    self.log_blend_surface();
                }
            } else if let [id, edge] = input.split_whitespace().collect::<Vec<_>>().as_slice() {
                if let (Ok(object), Ok(edge)) = (id.parse::<ObjectId>(), edge.parse::<usize>()) {
                    self.pick_blend_edge(EdgePick { object, edge });
                } else {
                    self.log_blend_surface();
                }
            } else {
                self.log_blend_surface();
            }
            self.command_input.clear();
            return true;
        }
        if input.is_empty() {
            if p.prepared.is_none() {
                self.push_log("Fix the preview options before accepting".into());
                return true;
            }
            let p = self.blend_surface_prompt.take().unwrap();
            match p.prepared.unwrap().commit(&mut self.document) {
                Ok(_) => self.push_log("Created surface blend".into()),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        } else {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let edit = if words.len() == 2 && !words[0].contains('=') {
                format!("{}={}", words[0], words[1])
            } else {
                input.to_owned()
            };
            let allowed = [
                "continuity1",
                "continuity2",
                "bulge1",
                "bulge2",
                "reverse2",
                "trim1",
                "trim2",
                "join",
            ];
            let tokens = edit.split_whitespace().collect::<Vec<_>>();
            if tokens.iter().any(|w| {
                !w.split_once('=').is_some_and(|(k, _)| {
                    allowed.contains(&k.trim_start_matches('_').to_ascii_lowercase().as_str())
                })
            }) {
                self.push_log("Set Continuity1/2=G0|G1|G2, Bulge1/2=positive, Reverse2=Auto|Yes|No, Trim1/2=fraction, or Join=Yes|No".into());
            } else {
                match p.options.updated(&tokens) {
                    Ok(options) => {
                        if options != p.options {
                            self.blend_surface_prompt.as_mut().unwrap().options = options;
                            self.update_blend_preview();
                        } else {
                            self.log_blend_surface();
                        }
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        }
        self.command_input.clear();
        true
    }
}
