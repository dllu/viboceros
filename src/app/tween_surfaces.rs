//! Ordered source picks and cached readonly surface-tween previews.
use super::*;
use viboceros_command::tween_surfaces::{Options, Prepared, prepare};
use viboceros_document::{GeometrySnapshot, GroupId, ObjectAttributes, ObjectId};
use viboceros_geometry::Tolerance;

#[derive(Debug)]
pub(super) struct Prompt {
    pub(super) sources: Vec<ObjectId>,
    options: Options,
    snapshots: Vec<GeometrySnapshot>,
    attributes: Vec<ObjectAttributes>,
    groups: Vec<Vec<GroupId>>,
    tolerance: Tolerance,
    current_layer: viboceros_document::LayerId,
    prepared: Option<Prepared>,
    scene: Option<Document>,
    value_option: Option<String>,
}
impl Prompt {
    pub(super) fn hint(&self) -> &str {
        if self.value_option.is_some() {
            "Enter an option value; Esc cancels"
        } else if self.sources.is_empty() {
            "Select the start surface; Esc cancels"
        } else if self.sources.len() == 1 {
            "Select the end surface; Esc cancels"
        } else {
            "Edit options or directions; Enter accepts the preview, Esc cancels"
        }
    }
    pub(super) fn selecting(&self) -> bool {
        self.sources.len() < 2
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.current_layer == doc.current_layer_id()
            && self.sources.iter().enumerate().all(|(i, id)| {
                doc.is_object_selectable(*id)
                    && doc.object(*id).is_some_and(|o| {
                        self.snapshots[i].shares_storage_with(o.geometry_snapshot())
                            && self.attributes[i] == *o.attributes()
                            && self.groups[i] == o.group_ids()
                    })
            })
    }
}
impl VibocerosApp {
    pub(super) fn start_tween_surfaces(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("TweenSurfaces")
        }) {
            return false;
        }
        let options = match self.commands.tween_surface_options(&words[1..]) {
            Ok(v) => v,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        if options.sources.is_some() {
            return false;
        }
        let selected = self
            .document
            .selected_objects()
            .filter(|o| viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o))
            .map(|o| o.id())
            .take(3)
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.tween_surfaces_prompt = Some(Prompt {
            sources: vec![],
            options,
            snapshots: vec![],
            attributes: vec![],
            groups: vec![],
            tolerance: self.document.tolerance(),
            current_layer: self.document.current_layer_id(),
            prepared: None,
            scene: None,
            value_option: None,
        });
        self.push_log(format!("> {input}"));
        if selected.len() <= 2 {
            for id in selected {
                self.pick_tween_surface(Some(id));
            }
        }
        self.log_tween_surfaces();
        true
    }
    fn log_tween_surfaces(&mut self) {
        if let Some(p) = &self.tween_surfaces_prompt {
            self.push_log(format!(
                "TweenSurfaces: {}. {}",
                p.hint(),
                p.options.arguments()
            ));
        }
    }
    pub(super) fn cancel_tween_surfaces(&mut self) {
        self.tween_surfaces_prompt = None;
    }
    pub(super) fn validate_tween_surfaces(&mut self) {
        if self
            .tween_surfaces_prompt
            .as_ref()
            .is_some_and(|p| !p.current(&self.document))
        {
            self.cancel_tween_surfaces();
            self.push_log(
                "TweenSurfaces sources or document settings changed; select them again".into(),
            );
        }
    }
    pub(super) fn pick_tween_surface(&mut self, id: Option<ObjectId>) -> bool {
        let Some(p) = self.tween_surfaces_prompt.as_ref() else {
            return false;
        };
        if !p.selecting() {
            return true;
        }
        let Some(id) = id.filter(|id| {
            self.document.is_object_selectable(*id)
                && self.document.object(*id).is_some_and(|o| {
                    viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o)
                })
        }) else {
            return true;
        };
        let object = self.document.object(id).unwrap();
        let p = self.tween_surfaces_prompt.as_mut().unwrap();
        p.sources.push(id);
        p.snapshots.push(object.geometry_snapshot().clone());
        p.attributes.push(object.attributes().clone());
        p.groups.push(object.group_ids().to_vec());
        if p.sources.len() == 2 {
            self.update_tween_preview();
        } else {
            self.log_tween_surfaces();
        }
        true
    }
    fn update_tween_preview(&mut self) {
        let Some(p) = self.tween_surfaces_prompt.as_mut() else {
            return;
        };
        self.commands.remember_tween_surface_layer(p.options.layer);
        p.scene = None;
        p.prepared = None;
        let Ok(ids) = <[ObjectId; 2]>::try_from(p.sources.clone()) else {
            return;
        };
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let prepared = prepare(&self.document, ids, &p.options)?;
            let mut scene = self.document.clone();
            scene.clear_history()?;
            scene.begin_transaction("TweenSurfaces preview")?;
            prepared.apply(&mut scene)?;
            scene.commit_transaction()?;
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
        self.log_tween_surfaces();
    }
    pub(super) fn continue_tween_surfaces(&mut self, input: &str) -> bool {
        let Some(p) = self.tween_surfaces_prompt.as_ref() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_tween_surfaces();
            return true;
        }
        if !p.current(&self.document) {
            self.validate_tween_surfaces();
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_tween_surfaces();
            return false;
        }
        if p.selecting() {
            if let Ok(id) = input.parse::<ObjectId>() {
                self.pick_tween_surface(Some(id));
            } else {
                self.log_tween_surfaces();
            }
            self.command_input.clear();
            return true;
        }
        if input.is_empty() {
            if p.value_option.is_some() {
                self.tween_surfaces_prompt.as_mut().unwrap().value_option = None;
                self.log_tween_surfaces();
                self.command_input.clear();
                return true;
            }
            if p.prepared.is_none() {
                self.push_log("Fix the preview options before accepting".into());
                return true;
            }
            let p = self.tween_surfaces_prompt.take().unwrap();
            let mut began = false;
            let result = (|| -> Result<(), viboceros_command::CommandError> {
                self.document.begin_transaction("TweenSurfaces")?;
                began = true;
                p.prepared.unwrap().apply(&mut self.document)?;
                self.document.commit_transaction()?;
                Ok(())
            })();
            if let Err(error) = result {
                if began {
                    let _ = self.document.rollback_transaction();
                }
                self.push_log(format!("Error: {error}"));
            } else {
                self.commands.accept_tween_surface_preferences(&p.options);
                self.push_log(format!("Created {} tween surface(s)", p.options.number));
            }
            self.command_input.clear();
            return true;
        }
        let p = self.tween_surfaces_prompt.as_mut().unwrap();
        let edit = if let Some(name) = p.value_option.as_ref() {
            format!("{name}={input}")
        } else {
            input.to_owned()
        };
        let tokens = edit.split_whitespace().collect::<Vec<_>>();
        if tokens.len() == 1
            && !tokens[0].contains('=')
            && [
                "NumberOfSurfaces",
                "Number",
                "MatchMethod",
                "SampleNumber",
                "OutputLayer",
                "FlipStartU",
                "FlipStartV",
                "SwapStartUV",
                "FlipEndU",
                "FlipEndV",
                "SwapEndUV",
            ]
            .iter()
            .any(|s| tokens[0].eq_ignore_ascii_case(s))
        {
            p.value_option = Some(tokens[0].to_owned());
            self.log_tween_surfaces();
            self.command_input.clear();
            return true;
        }
        let edit = if tokens.len() == 2 && !tokens[0].contains('=') {
            format!("{}={}", tokens[0], tokens[1])
        } else {
            edit
        };
        match p
            .options
            .updated(&edit.split_whitespace().collect::<Vec<_>>())
        {
            Ok(options) => {
                p.value_option = None;
                if options != p.options {
                    p.options = options;
                    self.update_tween_preview();
                } else {
                    self.log_tween_surfaces();
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
