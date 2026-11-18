//! Repeated sections keep their sources until the command finishes.
use super::*;
use viboceros_command::{CommandError, ObjectSelectionFilter, section};
use viboceros_document::HistoryGroup;

pub(super) struct Session {
    pub(super) sources: Vec<ObjectId>,
    group: HistoryGroup,
    outputs: Vec<ObjectId>,
    accepted_plane: bool,
}

impl VibocerosApp {
    pub(super) fn try_start_section_input(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Section")
        }) {
            return false;
        }
        let Ok(options) = self.commands.section_options().with_arguments(&words[1..]) else {
            return false;
        };
        let sources = self
            .document
            .selected_objects()
            .filter(|object| ObjectSelectionFilter::Section.accepts_object(object))
            .map(|object| object.id())
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Section") {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                return true;
            }
        };
        self.section_session = Some(Session {
            sources,
            group,
            outputs: Vec::new(),
            accepted_plane: false,
        });
        self.active_command = Some(InteractiveCommand::Section {
            start: None,
            options,
        });
        self.section_prompt();
        true
    }

    pub(super) fn accept_section_point(&mut self, point: Point3) -> Option<bool> {
        let Some(InteractiveCommand::Section { start, options }) = self.active_command else {
            return None;
        };
        let session = self.section_session.as_mut()?;
        if !self.document.history_group_is_current(&session.group) {
            self.cancel_interactive_command(false);
            self.push_log("Section sources or history changed. Restart the command.".into());
            self.command_file.fail_dispatch();
            return Some(false);
        }
        if let Some(start) = start {
            let tolerance = self.document.tolerance();
            let plane = self.viewports[self.active_viewport].construction_plane();
            let result = section::section_frame(start, point, plane, tolerance)
                .map_err(CommandError::from)
                .and_then(|(frame, length)| {
                    self.document.begin_group_transaction(&session.group)?;
                    let result = (|| {
                        let ids = section::create_sections(
                            &mut self.document,
                            &session.sources,
                            frame,
                            length,
                            options,
                        )?;
                        if !ids.is_empty() {
                            self.document.release_command_selection_on_history_replay(
                                session.sources.iter().copied(),
                            )?;
                            self.document.select_command_results(
                                session.outputs.iter().chain(&ids).copied(),
                            )?;
                        }
                        Ok(ids)
                    })();
                    match result {
                        Ok(ids) => {
                            self.document.commit_group_transaction(&mut session.group)?;
                            Ok(ids)
                        }
                        Err(error) => {
                            self.document.rollback_transaction()?;
                            Err(error)
                        }
                    }
                });
            match result {
                Ok(ids) => {
                    let count = ids.len();
                    session.outputs.extend(ids);
                    session.accepted_plane = true;
                    let _ = self
                        .document
                        .set_transient_selection(session.sources.iter().copied());
                    self.push_log(format!("Created {count} section object(s)"));
                    self.active_command = Some(InteractiveCommand::Section {
                        start: None,
                        options,
                    });
                }
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_file.fail_dispatch();
                    return Some(false);
                }
            }
        } else {
            self.active_command = Some(InteractiveCommand::Section {
                start: Some(point),
                options,
            });
        }
        self.last_point = Some(point);
        self.section_prompt();
        Some(true)
    }

    pub(super) fn continue_section_input(&mut self, text: &str) -> bool {
        let Some(InteractiveCommand::Section { start, mut options }) = self.active_command else {
            return false;
        };
        if text.is_empty()
            || text
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Enter")
        {
            self.cancel_interactive_command(false);
            return true;
        }
        match options.update(text) {
            Ok(true) => {
                self.active_command = Some(InteractiveCommand::Section { start, options });
                self.section_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                true
            }
            Ok(false) => false,
        }
    }

    pub(super) fn finish_section_run(&mut self) {
        if let Some(session) = self.section_session.take()
            && self.document.history_group_is_current(&session.group)
        {
            let _ = self.document.select_command_results(session.outputs);
        }
    }

    fn section_prompt(&mut self) {
        let Some(InteractiveCommand::Section { start, options }) = self.active_command else {
            return;
        };
        self.commands.remember_section_options(options);
        let point = if start.is_some() {
            "End of section"
        } else if self
            .section_session
            .as_ref()
            .is_some_and(|session| session.accepted_plane)
        {
            "Start of section. Press Enter when done"
        } else {
            "Start of section"
        };
        self.push_log(format!("{point}. {}", options.arguments()));
        self.command_input.clear();
    }
}
