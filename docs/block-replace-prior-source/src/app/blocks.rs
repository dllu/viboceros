//! Block sources, base point/name getters and existing-definition insertion.
use super::*;
use viboceros_command::blocks::{InsertOptions, base_point, tokenize};

#[derive(Clone, Debug)]
pub(super) enum PendingBlock {
    Unique {
        sources: Vec<ObjectId>,
    },
    Create {
        sources: Vec<ObjectId>,
    },
    Insert {
        name: Option<String>,
        options: InsertOptions,
    },
}

impl VibocerosApp {
    pub(super) fn try_start_block_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        let name = input[..end].trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("CreateUniqueBlock") {
            if !input[end..].trim().is_empty() {
                return false;
            }
            let sources = picked.map(<[ObjectId]>::to_vec).unwrap_or_else(|| {
                self.document
                    .selected_objects()
                    .filter(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
                    .map(|o| o.id())
                    .collect()
            });
            if sources.is_empty() {
                return false;
            }
            self.cancel_interactive_command(false);
            self.block_session = Some(PendingBlock::Unique { sources });
            self.active_command = Some(InteractiveCommand::CreateUniqueBlock);
            self.command_input.clear();
            self.push_log(self.active_command.unwrap().prompt().into());
            return true;
        }
        if !name.eq_ignore_ascii_case("Block") && !name.eq_ignore_ascii_case("Insert") {
            return false;
        }
        let arguments = match tokenize(&input[end..]) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if name.eq_ignore_ascii_case("Block") {
            let base = if arguments.is_empty() {
                None
            } else {
                let Ok((point, count)) = base_point(&arguments) else {
                    return false;
                };
                if count != arguments.len() {
                    return false;
                }
                Some(point)
            };
            let sources = picked
                .map(<[ObjectId]>::to_vec)
                .unwrap_or_else(|| self.document.selected_object_ids().collect());
            if sources.is_empty() {
                return false;
            }
            self.cancel_interactive_command(false);
            self.block_session = Some(PendingBlock::Create { sources });
            self.active_command = Some(InteractiveCommand::Block { base });
        } else {
            let quoted_name = input[end..].trim_start().starts_with('"');
            let (block_name, option_arguments) =
                if !quoted_name && arguments.first().is_none_or(|word| word.contains('=')) {
                    (None, arguments.as_slice())
                } else {
                    if arguments.iter().skip(1).any(|word| !word.contains('=')) {
                        return false;
                    }
                    (Some(arguments[0].to_owned()), &arguments[1..])
                };
            let options = match InsertOptions::parse(option_arguments) {
                Ok(options) => options,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            };
            if let Some(name) = &block_name
                && self.document.block_definition_by_name(name).is_none()
            {
                self.push_log(format!("Error: block definition '{name}' was not found"));
                return true;
            }
            self.cancel_interactive_command(false);
            self.active_command = Some(InteractiveCommand::Insert {
                has_name: block_name.is_some(),
            });
            self.block_session = Some(PendingBlock::Insert {
                name: block_name,
                options,
            });
            if matches!(
                &self.block_session,
                Some(PendingBlock::Insert { name: None, .. })
            ) {
                let names = self
                    .document
                    .block_definitions()
                    .map(|definition| definition.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.push_log(format!("Available blocks: {names}"));
            }
        }
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.command_input.clear();
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }

    pub(super) fn try_continue_block_input(&mut self, input: &str) -> bool {
        let Some(session) = self.block_session.clone() else {
            return false;
        };
        if input
            .split_whitespace()
            .next()
            .is_some_and(|word| self.commands.recognizes(word))
        {
            return false;
        }
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            return false;
        }
        match (session, self.active_command) {
            (PendingBlock::Unique { sources }, Some(InteractiveCommand::CreateUniqueBlock)) => {
                let words = match tokenize(input) {
                    Ok(w) if w.len() == 1 => w,
                    _ => {
                        self.push_log(
                            "Enter one new block name; quote names containing spaces".into(),
                        );
                        self.command_input.clear();
                        return true;
                    }
                };
                match self.document.make_block_instances_unique(words[0], sources) {
                    Ok(id) => {
                        self.push_log(format!(
                            "Created unique definition '{}'",
                            self.document.block_definition(id).unwrap().name()
                        ));
                        self.cancel_interactive_command(false);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                true
            }
            (
                PendingBlock::Create { sources },
                Some(InteractiveCommand::Block { base: Some(base) }),
            ) => {
                let arguments = match tokenize(input) {
                    Ok(arguments) if arguments.len() == 1 => arguments,
                    _ => {
                        self.push_log("Enter one block name; quote names containing spaces".into());
                        self.command_input.clear();
                        return true;
                    }
                };
                if let Err(error) = self
                    .document
                    .select_objects_direct(sources, SelectionMode::Replace)
                {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                let command = format!("Block {} \"{}\"", format_model_point(base), arguments[0]);
                self.run_block_command(&command);
                true
            }
            (
                PendingBlock::Insert {
                    name: None,
                    options,
                },
                Some(InteractiveCommand::Insert { .. }),
            ) => {
                let arguments = match tokenize(input) {
                    Ok(arguments) if arguments.len() == 1 => arguments,
                    _ => {
                        self.push_log(
                            "Enter an existing block name; quote names containing spaces".into(),
                        );
                        self.command_input.clear();
                        return true;
                    }
                };
                if self
                    .document
                    .block_definition_by_name(arguments[0])
                    .is_none()
                {
                    self.push_log(format!(
                        "Error: block definition '{}' was not found",
                        arguments[0]
                    ));
                    self.command_input.clear();
                    return true;
                }
                self.block_session = Some(PendingBlock::Insert {
                    name: Some(arguments[0].into()),
                    options,
                });
                self.active_command = Some(InteractiveCommand::Insert { has_name: true });
                self.command_input.clear();
                self.push_log(self.active_command.unwrap().prompt().into());
                true
            }
            (
                PendingBlock::Insert {
                    name: Some(name),
                    options,
                },
                Some(InteractiveCommand::Insert { .. }),
            ) if input.contains('=') => {
                let words = input.split_whitespace().collect::<Vec<_>>();
                // Preserve current values while staging all newly supplied options.
                let changed = InsertOptions::parse(&words);
                match changed {
                    Ok(parsed) => {
                        let mut next = options;
                        for word in words {
                            let field = word.split_once('=').unwrap().0.trim_start_matches('_');
                            if field.eq_ignore_ascii_case("Scale") {
                                next.scale = parsed.scale;
                            } else if field.eq_ignore_ascii_case("Axis") {
                                next.axis = parsed.axis;
                            } else {
                                next.rotation_degrees = parsed.rotation_degrees;
                            }
                        }
                        self.block_session = Some(PendingBlock::Insert {
                            name: Some(name),
                            options: next,
                        });
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                true
            }
            _ => false,
        }
    }

    pub(super) fn accept_block_point(&mut self, point: Point3) -> Option<bool> {
        match (self.active_command, self.block_session.clone()) {
            (Some(InteractiveCommand::Block { base: None }), Some(PendingBlock::Create { .. })) => {
                self.active_command = Some(InteractiveCommand::Block { base: Some(point) });
                self.last_point = Some(point);
                self.command_input.clear();
                self.push_log(self.active_command.unwrap().prompt().into());
                Some(true)
            }
            (
                Some(InteractiveCommand::Insert { has_name: true }),
                Some(PendingBlock::Insert {
                    name: Some(name),
                    options,
                }),
            ) => {
                let command = format!(
                    "Insert \"{name}\" {} {}",
                    format_model_point(point),
                    options.command_options()
                );
                let accepted = self.run_block_command(&command);
                if accepted {
                    self.last_point = Some(point);
                }
                Some(accepted)
            }
            (
                Some(
                    InteractiveCommand::Block { .. }
                    | InteractiveCommand::Insert { .. }
                    | InteractiveCommand::CreateUniqueBlock,
                ),
                _,
            ) => Some(false),
            _ => None,
        }
    }

    fn run_block_command(&mut self, input: &str) -> bool {
        self.push_log(format!("> {input}"));
        match self.commands.execute(&mut self.document, input) {
            Ok(message) => {
                self.push_log(message);
                self.active_command = None;
                self.block_session = None;
                self.drafting_plane = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.command_input.clear();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                false
            }
        }
    }
}
