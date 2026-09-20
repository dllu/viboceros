//! Alignment mode/point phases stay outside document transactions.
use super::*;
use viboceros_command::AlignmentOptions;

impl VibocerosApp {
    pub(super) fn start_align(&self, input: &str) -> Option<InteractiveCommand> {
        self.document.selected_object_ids().next()?;
        let prompt = self.commands.object_selection_prompt(input).ok()??;
        let line = prompt.command_line();
        let arguments = line.split_whitespace().skip(1).collect::<Vec<_>>();
        let options = AlignmentOptions::default().parse(&arguments).ok()?;
        self.commands
            .accept_object_selection_options(&prompt)
            .ok()?;
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
            .chain(["AlignTo", "Mode", "Auto"].iter())
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
                if options.automatic || options.target.is_some() {
                    self.finish_align(options.target, options);
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
            self.push_log(
                "Choose Left, Right, Top, Bottom, HorizCenter, VertCenter, or Concentric first"
                    .into(),
            );
            return false;
        }
        let input = format!(
            "{} {}",
            options.command_line(),
            target.map_or_else(|| "Auto".into(), format_model_point)
        );
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
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
