//! Standalone curve/source, endpoint and numeric-confirmation phases.
use super::*;
use viboceros_document::ObjectId;

#[derive(Clone, Debug)]
pub(super) struct SubcurvePrompt {
    pub(super) source: Option<ObjectId>,
    pub(super) start: Option<f64>,
    pub(super) length: Option<f64>,
    pub(super) copy: bool,
}
impl SubcurvePrompt {
    pub(super) fn hint(&self) -> &'static str {
        match (self.source, self.start, self.length) {
            (None, _, _) => "SubCrv: select one curve; Esc cancels",
            (Some(_), None, _) => "SubCrv: pick the subcurve start (Esc cancels)",
            (Some(_), Some(_), None) => {
                "SubCrv: pick the directed subcurve end or type a length (Esc cancels)"
            }
            (Some(_), Some(_), Some(_)) => {
                "SubCrv: confirm the length direction; a new number replaces the length"
            }
        }
    }
}
impl VibocerosApp {
    pub(super) fn begin_subcurve_prompt(&mut self, copy: bool) {
        let sources = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let source = if let [id] = sources.as_slice() {
            Some(*id)
        } else {
            None
        };
        self.subcurve_prompt = Some(SubcurvePrompt {
            source,
            start: None,
            length: None,
            copy,
        });
        self.active_command = source.map(|_| InteractiveCommand::SubCrv { start: None, copy });
        if source.is_none() {
            self.document.clear_selection();
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
    }
    pub(super) fn pick_subcurve_source(&mut self, id: Option<ObjectId>) -> bool {
        let Some(prompt) = self.subcurve_prompt.as_mut() else {
            return false;
        };
        if prompt.source.is_some() {
            return false;
        }
        if let Some(id) = id.filter(|id| {
            self.document.is_object_selectable(*id)
                && self
                    .document
                    .object(*id)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
        }) {
            prompt.source = Some(id);
            self.active_command = Some(InteractiveCommand::SubCrv {
                start: None,
                copy: prompt.copy,
            });
            let _ = self.document.select_command_results([id]);
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        true
    }
    pub(super) fn continue_subcurve_prompt(&mut self, input: &str) -> bool {
        let Some(prompt) = self.subcurve_prompt.clone() else {
            return false;
        };
        if input.is_empty() {
            self.cancel_interactive_command(true);
            return true;
        }
        if let Some((name, value)) = input.trim_start_matches('_').split_once('=')
            && name.eq_ignore_ascii_case("Copy")
        {
            let copy = if value.trim_start_matches('_').eq_ignore_ascii_case("Yes") {
                Some(true)
            } else if value.trim_start_matches('_').eq_ignore_ascii_case("No") {
                Some(false)
            } else {
                None
            };
            if let Some(copy) = copy {
                self.subcurve_prompt.as_mut().unwrap().copy = copy;
                if let Some(InteractiveCommand::SubCrv { start, .. }) = self.active_command {
                    self.active_command = Some(InteractiveCommand::SubCrv { start, copy });
                }
                self.command_input.clear();
                self.push_log(format!("SubCrv Copy={}", if copy { "Yes" } else { "No" }));
            } else {
                self.push_log("Copy expects Yes or No".into());
            }
            return true;
        }
        if prompt.source.is_none() {
            if let Ok(id) = input.parse::<ObjectId>() {
                self.pick_subcurve_source(Some(id));
                self.command_input.clear();
                return true;
            }
        } else if prompt.start.is_some()
            && let Some(source) = prompt.source
            && let Some(quantity) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match quantity {
                Ok(0.) => self.cancel_interactive_command(true),
                Ok(length) => {
                    let total = self
                        .document
                        .object(source)
                        .and_then(|o| o.geometry().curve_ref())
                        .map(|c| c.length(self.document.tolerance()));
                    match total {
                        Some(Ok(total)) if length.abs() <= total => {
                            self.subcurve_prompt.as_mut().unwrap().length = Some(length.abs());
                            self.command_input.clear();
                            self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
                        }
                        Some(Err(error)) => self.push_log(format!("Error: {error}")),
                        _ => self.push_log("Curve length exceeds the whole source curve".into()),
                    }
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        } else if self.try_continue_point_constraint(input)
            || self.try_continue_point_filter(input)
            || self.try_continue_point_input(input)
        {
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_interactive_command(false);
            return false;
        }
        self.push_log(self.subcurve_prompt.as_ref().unwrap().hint().into());
        self.command_input.clear();
        true
    }
    pub(super) fn accept_standalone_subcurve_point(&mut self, point: Point3) -> Option<bool> {
        let mut prompt = self.subcurve_prompt.clone()?;
        let source = prompt.source?;
        let result = (|| -> Result<(), viboceros_command::CommandError> {
            let curve = self
                .document
                .object(source)
                .and_then(|o| o.geometry().curve_ref())
                .ok_or(viboceros_command::CommandError::Usage(
                    "Select an existing curve",
                ))?;
            let parameter = curve.closest_parameter(point, self.document.tolerance())?;
            let Some(start) = prompt.start else {
                prompt.start = Some(parameter);
                self.subcurve_prompt = Some(prompt.clone());
                self.active_command = Some(InteractiveCommand::SubCrv {
                    start: Some(point),
                    copy: prompt.copy,
                });
                self.push_log(prompt.hint().into());
                return Ok(());
            };
            let input = if let Some(length) = prompt.length {
                format!(
                    "SubCrv Numeric={start},{length},{parameter} Copy={}",
                    if prompt.copy { "Yes" } else { "No" }
                )
            } else {
                // Use point syntax so standalone orientation follows the public
                // command policy, while Parameter= retains directed math edits.
                let start_point = curve.evaluate(start)?;
                format!(
                    "SubCrv {} {} Copy={}",
                    format_model_point(start_point),
                    format_model_point(point),
                    if prompt.copy { "Yes" } else { "No" }
                )
            };
            self.document.select_command_results([source])?;
            self.commands.execute(&mut self.document, &input)?;
            self.subcurve_prompt = None;
            self.active_command = None;
            self.drafting_plane = None;
            self.push_log(format!("> {input}"));
            Ok(())
        })();
        match result {
            Ok(()) => Some(true),
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                Some(false)
            }
        }
    }
}
