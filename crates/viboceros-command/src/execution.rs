//! Command invocation stages, dispatch and transaction boundaries.
use super::*;

#[derive(Clone, Copy)]
enum InvocationSelection {
    Preselected,
    Postselected,
    StartedGetter,
}

impl CommandRegistry {
    pub fn execute(&self, document: &mut Document, input: &str) -> Result<String, CommandError> {
        self.execute_in_context(document, input, CommandContext::default())
    }

    pub fn execute_in_context(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            InvocationSelection::Preselected,
            None,
            None,
        )
    }

    pub fn execute_postselected(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            InvocationSelection::Postselected,
            None,
            None,
        )
    }

    /// Complete a getter started with `begin_object_selection`, without repeating
    /// command-start history effects. The picked owners are postselected inputs.
    pub fn execute_after_object_selection(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        projection: Option<picking::Projection>,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            InvocationSelection::StartedGetter,
            None,
            projection,
        )
    }

    /// Accept one atomic step of an already-started interactive command.
    /// Failed steps roll back without undoing prior steps. The caller starts
    /// Copy preferences once; subsequent steps do not reset disabled defaults.
    pub fn execute_in_history_group(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            InvocationSelection::Preselected,
            Some(group),
            None,
        )
    }

    /// Execute a point getter with the active camera; other commands retain
    /// their usual construction-plane context and transaction behavior.
    pub fn execute_with_pick_projection(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        postselected: bool,
        projection: picking::Projection,
    ) -> Result<String, CommandError> {
        self.execute_invocation(
            document,
            input,
            context,
            if postselected {
                InvocationSelection::Postselected
            } else {
                InvocationSelection::Preselected
            },
            None,
            Some(projection),
        )
    }

    fn execute_invocation(
        &self,
        document: &mut Document,
        input: &str,
        context: CommandContext,
        selection_stage: InvocationSelection,
        group: Option<&mut viboceros_document::HistoryGroup>,
        projection: Option<picking::Projection>,
    ) -> Result<String, CommandError> {
        let postselected = !matches!(selection_stage, InvocationSelection::Preselected);
        let mut tokens = input.trim_start().splitn(2, char::is_whitespace);
        let name = tokens
            .next()
            .filter(|name| !name.is_empty())
            .ok_or(CommandError::EmptyInput)?;
        let name = normalize_command_name(name);

        if name == "help" || name == "?" {
            return Ok(format!("Commands: {}", self.command_names().join(", ")));
        }

        let index = self
            .lookup
            .get(&name)
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        let command = &self.commands[index];
        if document.is_block_editing()
            && matches!(
                command.name(),
                "Open3dm" | "Export3dm" | "SaveAs" | "ExportStl" | "ExportStep"
            )
        {
            return Err(CommandError::Usage(
                "finish BlockEdit before opening or saving a file",
            ));
        }
        if group.is_some() && !command.records_history() {
            return Err(CommandError::Usage(
                "history groups require an editing command",
            ));
        }
        let arguments = command.parse_arguments(tokens.next().unwrap_or(""))?;
        let (arguments, copy) = if let Some(default) = command.copy_option_default() {
            copy_options::arguments(
                &arguments,
                if group.is_some() {
                    self.copy_preferences.peek(command.name(), default)
                } else {
                    self.copy_preferences.begin(command.name(), default)
                },
            )
        } else {
            (arguments, None)
        };
        let run = |document: &mut Document| {
            if matches!(selection_stage, InvocationSelection::StartedGetter) {
                command.run_after_object_selection(document, &arguments, context, projection)
            } else if let Some(projection) = projection {
                command.run_with_pick_projection(
                    document,
                    &arguments,
                    context,
                    postselected,
                    projection,
                )
            } else if postselected {
                command.run_postselected(document, &arguments, context)
            } else {
                command.run_in_context(document, &arguments, context)
            }
        };
        let mut single_group = if group.is_none()
            && command.records_history()
            && command.history_policy() != CommandHistoryPolicy::Ordinary
        {
            Some(document.begin_history_group(command.name())?)
        } else {
            None
        };
        let result = if let Some(group) = group.or(single_group.as_mut()) {
            command.history_policy().configure(group);
            document.begin_group_transaction(group)?;
            match run(document) {
                Ok(message) => {
                    document.commit_group_transaction(group)?;
                    Ok(message)
                }
                Err(error) => {
                    if command.commits_on_error(&error) {
                        document.commit_group_transaction(group)?;
                    } else {
                        document.rollback_transaction()?;
                    }
                    Err(error)
                }
            }
        } else if command.records_history() {
            run_command_transaction_with_policy(document, command.name(), run, |error| {
                command.commits_on_error(error)
            })
        } else {
            run(document)
        };
        if let Err(error) = &result {
            command.cleanup_failed_selection(document, error, postselected);
        } else if let Some(copy) = copy {
            self.copy_preferences.complete(command.name(), copy);
        }
        result
    }
}

pub(super) fn run_command_transaction<T>(
    document: &mut Document,
    name: &'static str,
    run: impl FnOnce(&mut Document) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    run_command_transaction_with_policy(document, name, run, |_| false)
}

fn run_command_transaction_with_policy<T>(
    document: &mut Document,
    name: &'static str,
    run: impl FnOnce(&mut Document) -> Result<T, CommandError>,
    commits_on_error: impl FnOnce(&CommandError) -> bool,
) -> Result<T, CommandError> {
    document.begin_transaction(name)?;
    match run(document) {
        Ok(message) => {
            document.commit_transaction()?;
            Ok(message)
        }
        Err(error) => {
            if commits_on_error(&error) {
                document.commit_transaction()?;
            } else {
                document.rollback_transaction()?;
            }
            Err(error)
        }
    }
}
