//! Bounded macro input uses the same getters as typed and picked input.
use super::*;
use viboceros_command::command_macro::{Action, Program, Stream, Waiting, path_argument};

#[derive(Default)]
pub(super) struct State {
    job: Option<Job>,
    dispatch: Dispatch,
    last_result: Option<bool>,
}

struct Job {
    stream: Stream,
    batch: viboceros_document::HistoryBatch,
    input: Input,
    echo: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Replay,
    Filename,
}

// A dispatch can still be on the stack after the replay closes its job.
#[derive(Default, PartialEq, Eq)]
enum Dispatch {
    #[default]
    Idle,
    Running,
    Failed,
}

impl State {
    pub(super) fn is_active(&self) -> bool {
        self.job.is_some()
    }

    pub(super) fn waiting_for_filename(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| job.input == Input::Filename)
    }

    pub(super) fn is_dispatching(&self) -> bool {
        self.dispatch != Dispatch::Idle
    }

    pub(super) fn fail_dispatch(&mut self) {
        if self.is_dispatching() {
            self.dispatch = Dispatch::Failed;
        }
    }

    #[cfg(test)]
    pub(super) fn last_result(&self) -> Option<bool> {
        self.last_result
    }
}

impl VibocerosApp {
    pub(super) fn command_file_child_idle(&self) -> bool {
        self.command_line_idle()
            && self.zoom_target.is_none()
            && !self.zoom_window_pending
            && self.copy_cplane_source.is_none()
            && self.selection_window_override.is_none()
            && self.circular_selection.is_none()
            && self.boundary_selection.is_none()
            && self.fence_selection.is_none()
            && self.lasso_selection.is_none()
    }
    fn cancel_command_file_getters(&mut self) -> bool {
        // Use the application's layered Escape handling without recursively
        // canceling the owning file. This also closes transparent view prompts.
        let mut job = self.command_file.job.take();
        if let Some(job) = &mut job {
            job.input = Input::Replay;
        }
        self.cancel_layer_prompt();
        self.cancel_interactive_command(false);
        for _ in 0..64 {
            if self.command_file_child_idle() {
                break;
            }
            self.cancel_current_prompt_or_selection();
        }
        let idle = self.command_file_child_idle();
        self.command_file.job = job;
        if !idle {
            self.push_log("Error: the previous command prompt could not be canceled".into());
        }
        idle
    }
    fn begin_command_file_job(&mut self) -> Option<&mut Job> {
        if !self.command_file.is_active() {
            if !self.cancel_command_file_getters() {
                self.command_file.fail_dispatch();
                self.command_file.last_result = Some(false);
                return None;
            }
            match self.document.begin_history_batch("ReadCommandFile") {
                Ok(batch) => {
                    self.command_file.job = Some(Job {
                        stream: Stream::default(),
                        batch,
                        input: Input::Replay,
                        echo: true,
                    })
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.command_file.fail_dispatch();
                    self.command_file.last_result = Some(false);
                    return None;
                }
            }
        }
        self.command_file.job.as_mut()
    }
    pub(super) fn command_file_input(&mut self, text: &str) -> bool {
        let mut words = text.trim().splitn(2, char::is_whitespace);
        let name = words.next().unwrap_or("").trim_start_matches(['_', '-']);
        let is_command = name.eq_ignore_ascii_case("ReadCommandFile");
        if !self.command_file.waiting_for_filename() && !is_command {
            return false;
        }
        let path = if is_command {
            words.next().unwrap_or("").trim()
        } else {
            text.trim()
        };
        if is_command && !self.command_file.is_active() {
            self.command_file.last_result = None;
        }
        if is_command && path.is_empty() {
            let Some(job) = self.begin_command_file_job() else {
                return true;
            };
            job.input = Input::Filename;
            self.command_input.clear();
            self.push_log("ReadCommandFile: enter the text filename; Esc cancels".into());
            return true;
        }
        if path.is_empty() || path.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.abort_command_file();
            self.command_input.clear();
            return true;
        }
        let result = path_argument(path).and_then(|p| Program::read(&p));
        match result {
            Ok(program) => {
                let Some(job) = self.begin_command_file_job() else {
                    return true;
                };
                if let Err(e) = job.stream.push(program) {
                    self.push_log(format!("Error: {e}"));
                    self.command_file.fail_dispatch();
                    self.abort_command_file();
                } else {
                    job.input = Input::Replay;
                }
                self.command_input.clear();
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_file.fail_dispatch();
                if !self.command_file.is_active() {
                    self.command_file.last_result = Some(false);
                }
            }
        }
        true
    }
    fn finish_command_file(&mut self, success: bool) {
        if let Some(job) = self.command_file.job.take() {
            let result = self.document.finish_history_batch(&job.batch);
            self.command_file.last_result = Some(success && result.is_ok());
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
        }
    }
    pub(super) fn abort_command_file(&mut self) -> bool {
        if !self.command_file.is_active() {
            return false;
        }
        self.cancel_command_file_getters();
        self.finish_command_file(false);
        true
    }
    pub(super) fn command_file_point_accepted(&mut self) {
        if self.command_file.is_dispatching() {
            return;
        }
        if let Some(job) = &mut self.command_file.job {
            job.stream.accepted_input();
            self.drain_command_file();
        }
    }
    pub(super) fn command_file_user_step(&mut self) {
        if self.command_file.is_dispatching() || !self.command_file.is_active() {
            return;
        }
        if self.command_file_child_idle()
            && self
                .command_file
                .job
                .as_ref()
                .is_some_and(|j| j.stream.waiting() == Some(Waiting::Input))
        {
            self.abort_command_file();
        } else {
            self.drain_command_file();
        }
    }
    pub(super) fn suppress_command_file_echo(&self) -> bool {
        self.command_file.is_dispatching()
            && self.command_file.job.as_ref().is_some_and(|job| !job.echo)
    }
    /// Returns true when another frame can continue without user input.
    pub(super) fn drain_command_file(&mut self) -> bool {
        if self.command_file.is_dispatching() {
            return false;
        }
        let started = std::time::Instant::now();
        for _ in 0..4096 {
            let active = !self.command_file_child_idle();
            let Some(job) = self.command_file.job.as_mut() else {
                return false;
            };
            if !self.document.history_batch_is_current(&job.batch) {
                self.abort_command_file();
                return false;
            }
            let action = job.stream.next(active);
            match action {
                Ok(Some(Action::Input(input))) => {
                    self.command_file.dispatch = Dispatch::Running;
                    self.command_input = input;
                    self.run_command();
                    if std::mem::take(&mut self.command_file.dispatch) == Dispatch::Failed {
                        self.abort_command_file();
                        return false;
                    }
                }
                Ok(Some(Action::Cancel { abort })) => {
                    let canceled = self.cancel_command_file_getters();
                    if abort || !canceled {
                        self.finish_command_file(false);
                        return false;
                    }
                }
                Ok(Some(Action::Echo(on))) => {
                    job.echo = on;
                }
                Ok(None) => {
                    if job.stream.finished(active) {
                        self.finish_command_file(true);
                    }
                    return false;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    self.abort_command_file();
                    return false;
                }
            }
            if self.command_file_child_idle()
                && let Some(job) = &self.command_file.job
                && let Err(e) = self.document.checkpoint_history_batch(&job.batch)
            {
                self.push_log(format!("Error: {e}"));
                self.abort_command_file();
                return false;
            }
            if started.elapsed() >= std::time::Duration::from_millis(4) {
                return true;
            }
        }
        true
    }
}
