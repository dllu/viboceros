//! Typed numeric Rebuild edits share object selection and command preferences.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::surface_rebuild::{self, Options, Prepared, SourceState};
use viboceros_document::{ControlPointId, Group, Layer, Object};

const NAMES: [&str; 7] = [
    "UPointCount",
    "VPointCount",
    "UDegree",
    "VDegree",
    "DeleteInput",
    "OutputLayer",
    "ReTrim",
];

#[derive(Debug)]
struct Background {
    objects: Vec<Object>,
    layers: Vec<Layer>,
    groups: Vec<Group>,
    grips: Vec<(ControlPointId, Point3, bool)>,
}
impl Background {
    fn capture(doc: &Document) -> Self {
        Self {
            objects: doc.objects().cloned().collect(),
            layers: doc.layers().cloned().collect(),
            groups: doc.groups().cloned().collect(),
            grips: doc.control_points().collect(),
        }
    }
    fn is_current(&self, doc: &Document) -> bool {
        doc.objects().len() == self.objects.len()
            && doc.objects().zip(&self.objects).all(|(a, b)| {
                a.geometry_snapshot()
                    .shares_storage_with(b.geometry_snapshot())
                    && a == b
            })
            && doc.layers().eq(self.layers.iter())
            && doc.groups().eq(self.groups.iter())
            && doc.control_points().eq(self.grips.iter().copied())
    }
}
#[derive(Debug)]
pub(super) struct Preview {
    state: SourceState,
    options: Options,
    pub(super) prepared: Option<Prepared>,
    scene: Option<Document>,
    background: Background,
}
impl Preview {
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
}

impl VibocerosApp {
    pub(super) fn initialize_rebuild_options(&mut self) {
        let Some(p) = self.object_prompt.as_ref().filter(|p| {
            p.description.command == "Rebuild" && p.phase == ObjectPromptPhase::Options
        }) else {
            return;
        };
        let defaults = self.commands.surface_rebuild_defaults();
        let input = p.command_override.as_deref().unwrap_or("Rebuild");
        match surface_rebuild::parse(
            &input.split_whitespace().skip(1).collect::<Vec<_>>(),
            defaults,
        ) {
            Ok(options) => self.store_rebuild_options(options),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn store_rebuild_options(&mut self, options: Options) {
        self.commands.remember_surface_rebuild_options(options);
        let p = self.object_prompt.as_mut().unwrap();
        p.command_override = Some(options.command_line());
        p.phase = ObjectPromptPhase::Options;
        self.update_rebuild_preview(options);
    }

    pub(super) fn continue_rebuild_options(&mut self, input: &str) -> bool {
        let was_active = self.rebuild_preview.is_some();
        self.validate_rebuild_preview();
        if was_active && self.rebuild_preview.is_none() {
            return true;
        }

        let Some(p) = self.object_prompt.as_ref().filter(|p| {
            p.description.command == "Rebuild" && p.phase != ObjectPromptPhase::Selecting
        }) else {
            return false;
        };
        // Curve Rebuild retains its existing immediate behavior; this prompt
        // belongs to the measured surface branch.
        if !self
            .document
            .selected_objects()
            .any(|o| viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o))
        {
            return false;
        }
        let phase = p.phase;
        let current = self
            .rebuild_preview
            .as_ref()
            .map_or_else(|| self.commands.surface_rebuild_defaults(), |p| p.options);
        if input.is_empty() {
            if matches!(phase, ObjectPromptPhase::RebuildValue(_)) {
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
                self.command_input.clear();
                return true;
            }
            return self.accept_rebuild_preview();
        }
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("Enter") {
            return self.try_continue_object_prompt("");
        }
        if normalized.eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            self.command_input.clear();
            return true;
        }
        if phase == ObjectPromptPhase::Options && normalized.eq_ignore_ascii_case("Preview") {
            self.update_rebuild_preview(current);
            self.command_input.clear();
            return true;
        }
        let name = normalized.split(['=', ' ']).next().unwrap_or("");
        let local = NAMES.iter().any(|n| n.eq_ignore_ascii_case(name));
        if !local && phase == ObjectPromptPhase::Options && self.commands.recognizes(name) {
            return false;
        }
        if phase == ObjectPromptPhase::Options
            && let Some(&name) = NAMES.iter().find(|n| n.eq_ignore_ascii_case(normalized))
        {
            self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::RebuildValue(name);
            self.push_log(format!("{name}: {}", current.command_line()));
            self.command_input.clear();
            return true;
        }
        let update = match phase {
            ObjectPromptPhase::RebuildValue(name) => format!("{name}={normalized}"),
            _ => input.to_owned(),
        };
        match surface_rebuild::parse(&update.split_whitespace().collect::<Vec<_>>(), current) {
            Ok(options) => {
                self.store_rebuild_options(options);
                self.push_log(options.command_line());
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}

impl VibocerosApp {
    pub(super) fn validate_rebuild_preview(&mut self) {
        if self.object_prompt.as_ref().is_none_or(|p| {
            p.description.command != "Rebuild" || p.phase == ObjectPromptPhase::Selecting
        }) {
            self.rebuild_preview = None;
            return;
        }
        let Some(preview) = self.rebuild_preview.as_ref() else {
            return;
        };
        if !preview.state.is_current(&self.document) {
            self.cancel_object_prompt(false);
            self.push_log("Rebuild sources or settings changed; select the surfaces again".into());
        } else if !preview.background.is_current(&self.document) {
            self.refresh_rebuild_scene();
        }
    }
    fn update_rebuild_preview(&mut self, options: Options) {
        if self
            .rebuild_preview
            .as_ref()
            .is_some_and(|p| p.options == options && p.state.is_current(&self.document))
        {
            return;
        }
        let mut prepared = self.rebuild_preview.take().and_then(|p| p.prepared);
        let reused = prepared
            .as_mut()
            .is_some_and(|p| p.update_output_options(&self.document, options).is_ok());
        if !reused {
            prepared = match surface_rebuild::prepare(&self.document, options) {
                Ok(result) => Some(result),
                Err(error) => {
                    self.push_log(format!("Preview error: {error}"));
                    None
                }
            };
        }
        self.rebuild_preview = Some(Preview {
            state: SourceState::capture(&self.document),
            options,
            prepared,
            scene: None,
            background: Background::capture(&self.document),
        });
        self.refresh_rebuild_scene();
    }
    fn refresh_rebuild_scene(&mut self) {
        let Some(preview) = self.rebuild_preview.as_mut() else {
            return;
        };
        preview.scene = None;
        preview.background = Background::capture(&self.document);
        let Some(prepared) = preview.prepared.as_ref() else {
            return;
        };
        let result = (|| -> Result<Document, viboceros_command::CommandError> {
            let mut scene = self.document.clone();
            scene.clear_history()?;
            scene.begin_transaction("Rebuild preview")?;
            prepared.apply(&mut scene)?;
            scene.commit_transaction()?;
            scene.clear_history()?;
            Ok(scene)
        })();
        match result {
            Ok(scene) => preview.scene = Some(scene),
            Err(error) => {
                preview.prepared = None;
                self.push_log(format!("Preview error: {error}"));
            }
        }
    }
    fn accept_rebuild_preview(&mut self) -> bool {
        let Some(preview) = self.rebuild_preview.as_ref() else {
            return false;
        };
        if preview.prepared.is_none() || preview.scene.is_none() {
            self.push_log("Fix the Rebuild options before accepting".into());
            self.command_input.clear();
            return true;
        }
        let preview = self.rebuild_preview.take().unwrap();
        let prepared = preview.prepared.unwrap();
        let mut began = false;
        let result = (|| -> Result<String, viboceros_command::CommandError> {
            self.document.begin_transaction("Rebuild")?;
            began = true;
            let message = prepared.apply(&mut self.document)?;
            self.document.commit_transaction()?;
            Ok(message)
        })();
        match result {
            Ok(message) => {
                self.object_prompt = None;
                self.push_log(message);
            }
            Err(error) => {
                if began {
                    let _ = self.document.rollback_transaction();
                }
                self.push_log(format!("Error: {error}"));
                self.rebuild_preview = Some(Preview {
                    state: SourceState::capture(&self.document),
                    options: preview.options,
                    prepared: None,
                    scene: None,
                    background: Background::capture(&self.document),
                });
            }
        }
        self.command_input.clear();
        true
    }
}
