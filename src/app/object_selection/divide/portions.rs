//! Collect Divide's temporary sources without editing the model.
use super::*;
use crate::app::curve_portion::{Action, Editor, InputOrder, OrderedInput};
use viboceros_command::curve_reference::{CapturedCurve, CurvePortion, PortionBinding};
use viboceros_geometry::NurbsCurve;

#[derive(Clone, Debug, PartialEq)]
struct Range {
    reference: CapturedCurve,
    portion: Arc<CurvePortion>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Inputs {
    ranges: Vec<Range>,
    order: InputOrder,
    editor: Option<Box<Editor>>,
    id_pending: bool,
}

impl Inputs {
    pub(in crate::app) fn editor(&self) -> Option<&Editor> {
        self.editor.as_deref()
    }

    pub(in crate::app) fn hint(&self) -> Option<&'static str> {
        if self.id_pending {
            return Some("Enter the curve object ID");
        }
        self.editor().map(Editor::hint)
    }

    pub(in crate::app) fn sync_order(&mut self, doc: &Document) {
        if !self.ranges.is_empty() {
            self.order.sync(doc.selected_object_ids());
        }
    }

    pub(super) fn snapshots(&self) -> impl Iterator<Item = (ObjectId, &GeometrySnapshot)> {
        self.ranges
            .iter()
            .map(|range| (range.reference.object_id(), range.reference.source().0))
    }

    pub(in crate::app) fn curves<'a>(
        &'a self,
        doc: &'a Document,
    ) -> impl Iterator<Item = &'a NurbsCurve> + 'a {
        self.ranges.iter().filter_map(|range| {
            range
                .reference
                .current(doc)
                .then(|| range.portion.display())
        })
    }

    pub(super) fn sources(&self, doc: &Document) -> Result<Vec<Source>, CommandError> {
        if self.ranges.is_empty() {
            return Ok(doc.selected_object_ids().map(Source::whole).collect());
        }
        let mut order = self.order.clone();
        order.sync(doc.selected_object_ids());
        order
            .ordered()
            .map(|entry| match entry {
                OrderedInput::Object(id) => Ok(Source::whole(id)),
                OrderedInput::Range(index) => {
                    let range = &self.ranges[index];
                    if !range.reference.current(doc) {
                        return Err(CommandError::SubcurveInputStale);
                    }
                    let mut source = Source::whole(range.reference.object_id());
                    source.portion = Some(Arc::clone(&range.portion));
                    Ok(source)
                }
            })
            .collect()
    }

    fn begin(&mut self) {
        self.editor = Some(Box::default());
        self.id_pending = false;
    }

    fn accept(&mut self, doc: &Document, binding: PortionBinding) -> Result<(), CommandError> {
        let reference = self
            .editor()
            .and_then(|e| e.reference.as_ref())
            .filter(|reference| reference.current(doc))
            .ok_or(CommandError::SubcurveInputStale)?;
        let portion =
            CurvePortion::prepare(reference.curve_ref(), binding.parameters, binding.wrap)?;
        let reference = reference.clone();
        self.order
            .push_range(self.ranges.len(), doc.selected_object_ids());
        self.ranges.push(Range {
            reference,
            portion: Arc::new(portion),
        });
        self.editor = None;
        self.id_pending = false;
        Ok(())
    }
}

impl VibocerosApp {
    pub(in crate::app) fn try_start_divide_subcurve(&mut self, input: &str) -> bool {
        let mut words = input
            .split_whitespace()
            .map(|word| word.trim_start_matches(['_', '-']));
        if !words.next().is_some_and(|word| {
            word.eq_ignore_ascii_case("Divide") || word.eq_ignore_ascii_case("Div")
        }) || !words
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("SubCrv"))
            || words.next().is_some()
        {
            return false;
        }
        let Ok(Some(description)) = self.commands.object_selection_prompt("Divide") else {
            return false;
        };
        self.cancel_interactive_command(false);
        let mut pending = PendingObjectCommand::selecting(description);
        pending.selection_before = Some(self.document.selected_object_ids().collect());
        pending.divide_inputs.begin();
        self.object_prompt = Some(pending);
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_object_prompt();
        true
    }

    fn divide_inputs_mut(&mut self) -> Option<&mut Inputs> {
        let pending = self.object_prompt.as_mut()?;
        (pending.description.command == "Divide" && pending.phase == ObjectPromptPhase::Selecting)
            .then_some(&mut pending.divide_inputs)
    }

    pub(in crate::app) fn divide_subcurve_editor(&self) -> Option<&Editor> {
        self.object_prompt.as_ref()?.divide_inputs.editor()
    }

    pub(in crate::app) fn picking_divide_subcurve_source(&self) -> bool {
        self.divide_subcurve_editor()
            .is_some_and(|editor| editor.reference.is_none())
    }

    pub(in crate::app) fn discard_divide_subcurve(&mut self) -> bool {
        let Some(inputs) = self.divide_inputs_mut() else {
            return false;
        };
        if inputs.editor.take().is_none() {
            return false;
        }
        inputs.id_pending = false;
        self.snaps.model_override = None;
        self.command_input.clear();
        self.log_object_prompt();
        true
    }

    pub(in crate::app) fn continue_divide_subcurves(&mut self, input: &str) -> bool {
        let Some(pending) = self.object_prompt.as_ref().filter(|pending| {
            pending.description.command == "Divide"
                && pending.phase == ObjectPromptPhase::Selecting
                && pending.description.workflow == ObjectSelectionWorkflow::ConfirmAfterSelection
        }) else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once(|c: char| c == '=' || c.is_whitespace())
            .map_or((word, ""), |(name, value)| (name, value.trim()));
        if pending.divide_inputs.editor().is_none() {
            if word.eq_ignore_ascii_case("SubCrv") {
                self.divide_inputs_mut().unwrap().begin();
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
            if name.eq_ignore_ascii_case("SelID")
                || pending.divide_inputs.id_pending
                    && !word.is_empty()
                    && !word.eq_ignore_ascii_case("Cancel")
                    && !word.eq_ignore_ascii_case("Enter")
            {
                let id = if pending.divide_inputs.id_pending {
                    word
                } else {
                    value
                };
                self.divide_inputs_mut().unwrap().id_pending = id.is_empty();
                if !id.is_empty() {
                    match id.parse::<ObjectId>() {
                        Ok(id) => self.select_prompt_objects([id], SelectionMode::Add),
                        Err(_) => self.push_log("Enter a valid curve object ID".into()),
                    }
                }
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
            if pending.divide_inputs.ranges.is_empty() {
                return false;
            }
            if word.eq_ignore_ascii_case("SelNone") {
                *self.divide_inputs_mut().unwrap() = Inputs::default();
                return false;
            }
            if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
                if !self.initialize_divide_prompt() {
                    self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Selecting;
                }
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
            return false;
        }

        let inputs = &mut self.object_prompt.as_mut().unwrap().divide_inputs;
        let editor = inputs.editor.as_deref_mut().unwrap();
        if let Some(reference) = &editor.reference
            && !reference.current(&self.document)
        {
            self.push_log(CommandError::SubcurveInputStale.to_string());
            self.discard_divide_subcurve();
            return true;
        }
        if editor.reference.is_none()
            && (inputs.id_pending || name.eq_ignore_ascii_case("SelID"))
            && !word.eq_ignore_ascii_case("Cancel")
            && !word.eq_ignore_ascii_case("Enter")
            && !word.is_empty()
        {
            let id = if inputs.id_pending { word } else { value };
            inputs.id_pending = id.is_empty();
            if inputs.id_pending {
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
            let result = editor.input(
                id,
                &self.document,
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            );
            self.apply_divide_subcurve_action(result);
            return true;
        }
        if !word.eq_ignore_ascii_case("Cancel")
            && !word.eq_ignore_ascii_case("Enter")
            && viboceros_command::subcurve_input::direction_option(input).is_none()
            && self.commands.recognizes(name)
        {
            return false;
        }
        let result = editor.input(
            input,
            &self.document,
            self.viewports[self.active_viewport].construction_plane(),
            self.last_point,
        );
        self.apply_divide_subcurve_action(result);
        true
    }

    pub(in crate::app) fn pick_divide_subcurve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.plane_prompt.is_some() || self.nested_measurement.is_some() {
            return false;
        }
        let Some(inputs) = self
            .object_prompt
            .as_mut()
            .map(|p| &mut p.divide_inputs)
            .filter(|inputs| inputs.editor().is_some())
        else {
            return false;
        };
        let editor = inputs.editor.as_deref_mut().unwrap();
        if editor.reference.is_none()
            && let Some(id) = id
        {
            match editor.capture(&self.document, id, edge, None) {
                Ok(()) => {
                    inputs.id_pending = false;
                    self.command_input.clear();
                    self.log_object_prompt();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        true
    }

    pub(in crate::app) fn hover_divide_subcurve(&mut self, parameter: f64) {
        if let Some(editor) = self
            .divide_inputs_mut()
            .and_then(|inputs| inputs.editor.as_mut())
        {
            editor.hover_parameter(parameter);
        }
    }

    pub(in crate::app) fn accept_divide_subcurve_parameter(&mut self, parameter: f64) -> bool {
        let Some(editor) = self
            .object_prompt
            .as_mut()
            .and_then(|pending| pending.divide_inputs.editor.as_deref_mut())
            .filter(|editor| editor.reference.is_some())
        else {
            return false;
        };
        if !editor.reference.as_ref().unwrap().current(&self.document) {
            self.push_log(CommandError::SubcurveInputStale.to_string());
            self.discard_divide_subcurve();
            return true;
        }
        let result = editor.accept_parameter(parameter, self.document.tolerance());
        if result.is_ok() {
            self.snaps.model_override = None;
        }
        self.apply_divide_subcurve_action(result);
        true
    }

    pub(in crate::app) fn divide_subcurve_preview(&mut self) -> Option<subcurve_preview::Preview> {
        self.object_prompt
            .as_mut()?
            .divide_inputs
            .editor
            .as_mut()?
            .preview(&self.document)
    }

    fn apply_divide_subcurve_action(&mut self, result: Result<Action, String>) {
        match result {
            Ok(Action::Discard) => {
                self.discard_divide_subcurve();
            }
            Ok(Action::Capture(id, edge)) => {
                self.pick_divide_subcurve(Some(id), edge);
            }
            Ok(Action::Pending) => {
                self.command_input.clear();
                self.log_object_prompt();
            }
            Ok(Action::Selected(binding)) => {
                let inputs = &mut self.object_prompt.as_mut().unwrap().divide_inputs;
                match inputs.accept(&self.document, binding) {
                    Ok(()) => {
                        self.snaps.model_override = None;
                        self.command_input.clear();
                        self.log_object_prompt();
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
