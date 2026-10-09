//! Typed numeric Rebuild edits share object selection and command preferences.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::curve_rebuild;
use viboceros_command::surface_rebuild::{self, SourceState};
use viboceros_document::{ControlPointId, Group, Layer, Object};

mod controls;

const SURFACE_NAMES: [&str; 7] = [
    "UPointCount",
    "VPointCount",
    "UDegree",
    "VDegree",
    "DeleteInput",
    "OutputLayer",
    "ReTrim",
];

const CURVE_NAMES: [&str; 7] = [
    "PointCount",
    "Degree",
    "PreserveTangents",
    "DeleteInput",
    "OutputLayer",
    "Points",
    "PreserveEndTangents",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Options {
    Surface(surface_rebuild::Options),
    Curve(curve_rebuild::Options),
}
impl Options {
    fn command_line(self) -> String {
        match self {
            Self::Surface(o) => o.command_line(),
            Self::Curve(o) => o.command_line(),
        }
    }
    fn parse(self, args: &[&str]) -> Result<Self, viboceros_command::CommandError> {
        match self {
            Self::Surface(o) => surface_rebuild::parse(args, o).map(Self::Surface),
            Self::Curve(o) => curve_rebuild::parse(args, o).map(Self::Curve),
        }
    }
    fn names(self) -> &'static [&'static str] {
        match self {
            Self::Surface(_) => &SURFACE_NAMES,
            Self::Curve(_) => &CURVE_NAMES,
        }
    }
}
#[derive(Clone, Debug)]
pub(super) enum Prepared {
    Surface(surface_rebuild::Prepared),
    Curve(curve_rebuild::Prepared),
}
impl Prepared {
    fn update_output_options(
        &mut self,
        doc: &Document,
        o: Options,
    ) -> Result<(), viboceros_command::CommandError> {
        match (self, o) {
            (Self::Surface(p), Options::Surface(o)) => p.update_output_options(doc, o),
            (Self::Curve(p), Options::Curve(o)) => p.update_output_options(doc, o),
            _ => Err(viboceros_command::CommandError::StaleCurveRebuild),
        }
    }
    fn apply(&self, doc: &mut Document) -> Result<String, viboceros_command::CommandError> {
        match self {
            Self::Surface(p) => p.apply(doc),
            Self::Curve(p) => p.apply(doc),
        }
    }
    #[cfg(test)]
    pub(super) fn outputs(
        &self,
    ) -> Box<dyn ExactSizeIterator<Item = &viboceros_document::GeometrySnapshot> + '_> {
        match self {
            Self::Surface(p) => Box::new(p.outputs()),
            Self::Curve(p) => Box::new(p.outputs()),
        }
    }
}

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

/// Resolve staged scenes in the same order used by production drawing.
pub(super) fn viewport_document<'a>(
    document: &'a Document,
    boolean: Option<&'a Document>,
    tween: Option<&'a Document>,
    rebuild: Option<&'a Document>,
) -> &'a Document {
    boolean.or(tween).or(rebuild).unwrap_or(document)
}

impl VibocerosApp {
    pub(super) fn initialize_rebuild_options(&mut self) {
        let Some(p) = self.object_prompt.as_ref().filter(|p| {
            p.description.command == "Rebuild" && p.phase == ObjectPromptPhase::Options
        }) else {
            return;
        };
        let defaults = if self
            .document
            .selected_objects()
            .any(|o| viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o))
        {
            Options::Surface(self.commands.surface_rebuild_defaults())
        } else {
            Options::Curve(curve_rebuild::Options::default())
        };
        let input = p.command_override.as_deref().unwrap_or("Rebuild");
        match defaults.parse(&input.split_whitespace().skip(1).collect::<Vec<_>>()) {
            Ok(options) => self.store_rebuild_options(options),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn store_rebuild_options(&mut self, options: Options) {
        if let Options::Surface(o) = options {
            self.commands.remember_surface_rebuild_options(o);
        }
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
        let phase = p.phase;
        let current =
            self.rebuild_preview.as_ref().map_or_else(
                || {
                    if self.document.selected_objects().any(|o| {
                        viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o)
                    }) {
                        Options::Surface(self.commands.surface_rebuild_defaults())
                    } else {
                        Options::Curve(curve_rebuild::Options::default())
                    }
                },
                |p| p.options,
            );
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
        let local = current.names().iter().any(|n| n.eq_ignore_ascii_case(name));
        if !local && phase == ObjectPromptPhase::Options && self.commands.recognizes(name) {
            return false;
        }
        if phase == ObjectPromptPhase::Options
            && let Some(&name) = current
                .names()
                .iter()
                .find(|n| n.eq_ignore_ascii_case(normalized))
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
        match current.parse(&update.split_whitespace().collect::<Vec<_>>()) {
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
            self.push_log("Rebuild sources or settings changed; select the objects again".into());
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
            let result = match options {
                Options::Surface(o) => {
                    surface_rebuild::prepare(&self.document, o).map(Prepared::Surface)
                }
                Options::Curve(o) => curve_rebuild::prepare(&self.document, o).map(Prepared::Curve),
            };
            prepared = match result {
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
