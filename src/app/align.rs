//! Alignment mode/point phases stay outside document transactions.
use super::*;
use viboceros_command::AlignmentOptions;

impl VibocerosApp {
    pub(super) fn update_align_selection(
        &self,
        prompt: &viboceros_command::ObjectSelectionPrompt,
        input: &str,
    ) -> Result<viboceros_command::ObjectSelectionPrompt, viboceros_command::CommandError> {
        let usage = viboceros_command::CommandError::Usage(
            "alignment options only; finish object selection before entering points",
        );
        let line = prompt.command_line();
        let options = AlignmentOptions::default()
            .parse(&line.split_whitespace().skip(1).collect::<Vec<_>>())?
            .parse(&input.split_whitespace().collect::<Vec<_>>())?;
        if options.target.is_some()
            || options.automatic
            || options.references.iter().any(Option::is_some)
        {
            return Err(usage);
        }
        self.commands
            .object_selection_prompt(&options.command_line())?
            .ok_or(usage)
    }

    pub(super) fn start_align(&self, input: &str) -> Option<InteractiveCommand> {
        self.document.selected_object_ids().next()?;
        let prompt = self.commands.object_selection_prompt("Align").ok()??;
        let line = prompt.command_line();
        let arguments = line.split_whitespace().skip(1).collect::<Vec<_>>();
        let options = AlignmentOptions::default()
            .parse(&arguments)
            .ok()?
            .parse(&input.split_whitespace().skip(1).collect::<Vec<_>>())
            .ok()?;
        if options.ready() {
            return None;
        }
        self.commands.accept_object_selection_input(input).ok()?;
        Some(InteractiveCommand::Align {
            options,
            postselected: false,
        })
    }

    pub(super) fn try_continue_align(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Align {
            options,
            postselected,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        if input.is_empty() {
            self.finish_align(None, options);
            return true;
        }
        let arguments = input.split_whitespace().collect::<Vec<_>>();
        let first = arguments[0]
            .split('=')
            .next()
            .unwrap()
            .trim_start_matches('_');
        if !viboceros_command::AlignmentMode::LABELS
            .iter()
            .chain(["AlignTo", "Mode", "Auto", "3Point"].iter())
            .any(|name| first.eq_ignore_ascii_case(name))
        {
            return false;
        }
        match options.parse(&arguments) {
            Ok(options) => {
                if let Err(error) = self
                    .commands
                    .accept_object_selection_input(&options.command_line())
                {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                if options.ready() {
                    self.execute_align(options);
                } else {
                    let command = InteractiveCommand::Align {
                        options,
                        postselected,
                    };
                    self.active_command = Some(command);
                    self.push_log(options.command_line());
                    self.push_log(command.prompt().into());
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn finish_align(
        &mut self,
        target: Option<Point3>,
        options: AlignmentOptions,
    ) -> bool {
        if options.mode.is_none() {
            self.push_log("Choose an alignment mode first".into());
            return false;
        }
        let updated = if let Some(point) = target {
            options.with_point(point)
        } else {
            options.parse(&["Auto"])
        };
        let options = match updated {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        if !options.ready() {
            if let Some(InteractiveCommand::Align {
                options: active, ..
            }) = &mut self.active_command
            {
                *active = options;
            }
            self.push_log(self.active_command.unwrap().prompt().into());
            return true;
        }
        self.execute_align(options)
    }

    fn execute_align(&mut self, options: AlignmentOptions) -> bool {
        let input = options.command_line();
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let postselected = matches!(
            self.active_command,
            Some(InteractiveCommand::Align {
                postselected: true,
                ..
            })
        );
        let result = if postselected {
            self.commands
                .execute_postselected(&mut self.document, &input, context)
        } else {
            self.commands
                .execute_in_context(&mut self.document, &input, context)
        };
        match result {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(message);
                true
            }
            Err(error) => {
                if matches!(
                    error,
                    viboceros_command::CommandError::InsufficientPlaneAlignmentObjects { .. }
                ) {
                    self.cancel_interactive_command(false);
                }
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
