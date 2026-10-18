//! Surface-side picks and cached, cancellable FilletSrf previews.
use super::*;
use viboceros_command::fillet_surface::{Options, Prepared, parse, prepare, source_surface};
use viboceros_geometry::Real;

#[derive(Debug)]
pub(super) struct Prompt {
    picks: Vec<ObjectId>,
    sources: Vec<viboceros_document::GeometrySnapshot>,
    options: Options,
    prepared: Option<Prepared>,
    scene: Option<Document>,
    tolerance: Tolerance,
}
impl Prompt {
    pub(super) fn selecting(&self) -> bool {
        self.picks.len() < 2
    }
    pub(super) fn excluded(&self) -> Option<ObjectId> {
        self.selecting()
            .then(|| self.picks.first().copied())
            .flatten()
    }
    pub(super) fn hint(&self) -> &'static str {
        match self.picks.len() {
            0 => {
                "Pick the first surface on the side to keep, or type object-id u,v; Radius / Extend / Trim / Join; Esc cancels"
            }
            1 => {
                "Pick a different surface on the side to keep, or type object-id u,v; Radius / Extend / Trim / Join; Esc cancels"
            }
            _ => {
                "Set Radius, Extend=Yes|No, Trim=Yes|No|Split or Join=Yes|No; Enter accepts, Esc cancels"
            }
        }
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub(super) fn highlights(&self) -> impl Iterator<Item = crate::viewport::ComponentPick> + '_ {
        self.picks
            .iter()
            .map(|&object| crate::viewport::ComponentPick {
                object,
                kind: viboceros_command::ComponentSelectionKind::BrepFace,
                index: 0,
            })
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.prepared.as_ref().is_none_or(|p| p.is_current(doc))
            && self.picks.iter().enumerate().all(|(i, id)| {
                doc.is_object_selectable(*id)
                    && doc
                        .object(*id)
                        .is_some_and(|o| self.sources[i].shares_storage_with(o.geometry_snapshot()))
            })
    }
}
impl VibocerosApp {
    pub(super) fn picking_fillet_surface(&self) -> bool {
        self.fillet_surface_prompt
            .as_ref()
            .is_some_and(Prompt::selecting)
    }
    pub(super) fn start_fillet_surface(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("FilletSrf")
        }) {
            return false;
        }
        let options = match parse(&words[1..]) {
            // Explicit scripting inputs are handled by the command registry.
            Ok(options)
                if options.sources.is_some()
                    || options.picks.iter().any(Option::is_some)
                    || options.preview =>
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
        self.fillet_surface_prompt = Some(Prompt {
            picks: vec![],
            sources: vec![],
            options,
            prepared: None,
            scene: None,
            tolerance: self.document.tolerance(),
        });
        self.push_log(format!("> {input}"));
        self.log_fillet_surface();
        true
    }
    fn log_fillet_surface(&mut self) {
        if let Some(p) = &self.fillet_surface_prompt {
            self.push_log(format!("FilletSrf: {}", p.hint()));
        }
    }
    pub(super) fn validate_fillet_surface(&mut self) {
        if self
            .fillet_surface_prompt
            .as_ref()
            .is_some_and(|p| !p.current(&self.document))
        {
            self.fillet_surface_prompt = None;
            self.push_log(
                "FilletSrf sources or document settings changed; pick the surfaces again".into(),
            );
        }
    }
    pub(super) fn accept_fillet_surface_face(
        &mut self,
        object: ObjectId,
        face: usize,
        point: Option<Point3>,
    ) -> bool {
        if self.fillet_surface_prompt.is_none() {
            return false;
        }
        self.validate_fillet_surface();
        if !self.picking_fillet_surface() {
            return true;
        }
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            if face != 0 {
                return Err(viboceros_command::CommandError::SurfaceFillet(
                    "select a single-face surface",
                ));
            }
            let surface = source_surface(&self.document, object)?;
            let point = point.ok_or(viboceros_command::CommandError::SurfaceFillet(
                "click inside the face, or type object-id u,v",
            ))?;
            let (u, v) = surface.closest_parameters(point, self.document.tolerance())?;
            Ok([u, v])
        })();
        match result {
            Ok(uv) => self.pick_fillet_surface(object, uv),
            Err(error) => self.push_log(format!("Error: {error}")),
        };
        true
    }
    fn pick_fillet_surface(&mut self, object: ObjectId, uv: [Real; 2]) {
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let surface = source_surface(&self.document, object)?;
            surface.evaluate(uv[0], uv[1])?;
            Ok(self
                .document
                .object(object)
                .unwrap()
                .geometry_snapshot()
                .clone())
        })();
        let source = match result {
            Ok(source) => source,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        let p = self.fillet_surface_prompt.as_mut().unwrap();
        if p.picks.contains(&object) {
            self.push_log("Pick a different parent surface".into());
            return;
        }
        p.options.picks[p.picks.len()] = Some(uv);
        p.picks.push(object);
        p.sources.push(source);
        if p.selecting() {
            self.log_fillet_surface();
        } else {
            self.update_fillet_preview();
        }
    }
    fn update_fillet_preview(&mut self) {
        let p = self.fillet_surface_prompt.as_mut().unwrap();
        p.prepared = None;
        p.scene = None;
        if p.selecting() {
            self.log_fillet_surface();
            return;
        }
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let prepared = prepare(&self.document, [p.picks[0], p.picks[1]], &p.options)?;
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
        self.log_fillet_surface();
    }
    pub(super) fn continue_fillet_surface(&mut self, input: &str) -> bool {
        let Some(p) = self.fillet_surface_prompt.as_ref() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.fillet_surface_prompt = None;
            self.command_input.clear();
            return true;
        }
        let first = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('_');
        let option = ["Radius", "Extend", "Trim", "Join", "BlendType"]
            .iter()
            .any(|name| first.eq_ignore_ascii_case(name));
        if !option && self.commands.recognizes(first) {
            self.fillet_surface_prompt = None;
            return false;
        }
        if !p.current(&self.document) {
            self.validate_fillet_surface();
            return true;
        }
        if input.is_empty() && !p.selecting() {
            if p.prepared.is_none() {
                self.push_log("Fix the preview options before accepting".into());
                return true;
            }
            let p = self.fillet_surface_prompt.take().unwrap();
            match p.prepared.unwrap().commit(&mut self.document) {
                Ok(_) => self.push_log("Created surface fillet".into()),
                Err(error) => self.push_log(format!("Error: {error}")),
            };
        } else {
            let words = input.split_whitespace().collect::<Vec<_>>();
            let typed = if p.selecting() {
                match words.as_slice() {
                    [id, uv] if uv.contains(',') => id.parse::<ObjectId>().ok().and_then(|id| {
                        parse(&[&format!("Pick1={uv}")])
                            .ok()
                            .map(|o| (id, o.picks[0].unwrap()))
                    }),
                    [id, u, v] => id.parse::<ObjectId>().ok().and_then(|id| {
                        parse(&[&format!("Pick1={u},{v}")])
                            .ok()
                            .map(|o| (id, o.picks[0].unwrap()))
                    }),
                    _ => None,
                }
            } else {
                None
            };
            if let Some((id, uv)) = typed {
                self.pick_fillet_surface(id, uv);
            } else {
                let edit = if words.len() == 2 && !words[0].contains('=') {
                    format!("{}={}", words[0], words[1])
                } else if words.len() == 1 && words[0].parse::<Real>().is_ok() {
                    format!("Radius={}", words[0])
                } else {
                    input.to_owned()
                };
                let tokens = edit.split_whitespace().collect::<Vec<_>>();
                let allowed = ["radius", "extend", "trim", "join", "blendtype"];
                if tokens.is_empty()
                    || tokens.iter().any(|w| {
                        !w.split_once('=').is_some_and(|(k, _)| {
                            allowed
                                .contains(&k.trim_start_matches('_').to_ascii_lowercase().as_str())
                        })
                    })
                {
                    self.log_fillet_surface();
                } else {
                    match p.options.updated(&tokens) {
                        Ok(options) => {
                            self.fillet_surface_prompt.as_mut().unwrap().options = options;
                            self.update_fillet_preview();
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                }
            }
        }
        self.command_input.clear();
        true
    }
}
