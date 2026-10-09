//! Repeated section-plane point input uses the shared drafting pipeline.
use super::*;
impl VibocerosApp {
    pub(super) fn try_start_section_input(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty()
            || !words[0]
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Section")
        {
            return false;
        }
        let options = match viboceros_command::section::parse_options(&words[1..]) {
            Ok(options) => options,
            Err(_) => return false,
        };
        if !self
            .document
            .selected_objects()
            .any(|object| viboceros_command::ObjectSelectionFilter::Section.accepts_object(object))
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.active_command = Some(InteractiveCommand::Section {
            start: None,
            options,
        });
        self.command_input.clear();
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
    pub(super) fn accept_section_point(&mut self, point: Point3) -> Option<bool> {
        let Some(InteractiveCommand::Section { start, options }) = self.active_command else {
            return None;
        };
        if let Some(start) = start {
            let sources = self.document.selected_object_ids().collect::<Vec<_>>();
            let command = format!(
                "Section {} {} ExtendSection={} AssignProperties={} Output=CurvesOnly GroupObjectsBySectionPlane={}",
                format_model_point(start),
                format_model_point(point),
                if options.extend { "Yes" } else { "No" },
                if options.input_properties {
                    "ByInputObject"
                } else {
                    "ByCurrentLayer"
                },
                if options.group { "Yes" } else { "No" }
            );
            match self.commands.execute_in_context(
                &mut self.document,
                &command,
                viboceros_command::CommandContext {
                    construction_plane: self.viewports[self.active_viewport].construction_plane(),
                },
            ) {
                Ok(message) => {
                    self.push_log(message);
                    let _ = self
                        .document
                        .select_objects_direct(sources, SelectionMode::Replace);
                    self.active_command = Some(InteractiveCommand::Section {
                        start: None,
                        options,
                    });
                    self.last_point = Some(point);
                    self.command_input.clear();
                    self.push_log("Section: pick another plane or press Enter to finish".into());
                    Some(true)
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    Some(false)
                }
            }
        } else {
            self.active_command = Some(InteractiveCommand::Section {
                start: Some(point),
                options,
            });
            self.last_point = Some(point);
            self.command_input.clear();
            self.push_log(self.active_command.unwrap().prompt().into());
            Some(true)
        }
    }
    pub(super) fn continue_section_input(&mut self, text: &str) -> bool {
        if !matches!(
            self.active_command,
            Some(InteractiveCommand::Section { .. })
        ) {
            return false;
        }
        if text.trim().is_empty() {
            self.cancel_interactive_command(false);
            return true;
        }
        false
    }
}
