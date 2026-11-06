//! Scriptable layer options; creation leaves the current layer unchanged.
use super::*;

pub(super) struct Prompt {
    batch: viboceros_document::HistoryBatch,
    operation: Option<String>,
}
impl VibocerosApp {
    pub(super) fn start_layer_prompt(&mut self, input: &str) -> bool {
        if !input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Layer")
        {
            return false;
        }
        self.cancel_interactive_command(false);
        match self.document.begin_history_batch("Layer") {
            Ok(batch) => {
                self.layer_prompt = Some(Prompt {
                    batch,
                    operation: None,
                })
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.layer_prompt_message();
        true
    }
    pub(super) fn cancel_layer_prompt(&mut self) {
        if let Some(p) = self.layer_prompt.take()
            && let Err(e) = self.document.finish_history_batch(&p.batch)
        {
            self.push_log(format!("Error: {e}"));
        }
    }
    fn layer_prompt_message(&mut self) {
        if let Some(p) = &self.layer_prompt {
            self.push_log(if let Some(op) = &p.operation {
                format!("Layer {op}: layer name; Esc cancels")
            } else {
                "Layer: New Current On Off Lock Unlock Delete List; Enter finishes".into()
            });
        }
        self.command_input.clear();
    }
    pub(super) fn continue_layer_prompt(&mut self, text: &str) -> bool {
        let Some(p) = self.layer_prompt.as_ref() else {
            return false;
        };
        if !self.document.history_batch_is_current(&p.batch) {
            self.cancel_layer_prompt();
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            self.cancel_layer_prompt();
            self.command_input.clear();
            return true;
        }
        if let Some(operation) = p.operation.clone() {
            let name = text
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(text.trim());
            let result = (|| -> Result<(), String> {
                if operation == "new" {
                    self.document
                        .add_layer(name, viboceros_document::ColorRgb::new(0, 0, 0))
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                let id = self
                    .document
                    .layer_by_name(name)
                    .ok_or_else(|| format!("layer '{name}' was not found"))?
                    .id();
                match operation.as_str() {
                    "current" => self.document.set_current_layer(id),
                    "on" => self.document.set_layer_visibility(id, true).map(|_| ()),
                    "off" => self.document.set_layer_visibility(id, false).map(|_| ()),
                    "lock" => self.document.set_layer_locked(id, true).map(|_| ()),
                    "unlock" => self.document.set_layer_locked(id, false).map(|_| ()),
                    "delete" => self.document.delete_layer(id).map(|_| ()),
                    _ => unreachable!(),
                }
                .map_err(|e| e.to_string())?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    self.layer_prompt.as_mut().unwrap().operation = None;
                    let p = self.layer_prompt.as_ref().unwrap();
                    if let Err(e) = self.document.checkpoint_history_batch(&p.batch) {
                        self.push_log(format!("Error: {e}"));
                    }
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.command_file_step_failed = self.executing_command_file;
                }
            }
        } else {
            let word = word.to_ascii_lowercase();
            if ["new", "current", "on", "off", "lock", "unlock", "delete"].contains(&word.as_str())
            {
                self.layer_prompt.as_mut().unwrap().operation = Some(word);
            } else if word == "list" {
                self.push_log(
                    self.document
                        .layers()
                        .map(|l| l.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            } else {
                self.push_log("Error: choose a supported layer option".into());
                self.command_file_step_failed = self.executing_command_file;
            }
        }
        self.layer_prompt_message();
        true
    }
}
