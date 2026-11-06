//! Bounded macro input uses the same getters as typed and picked input.
use super::*;
use viboceros_command::command_macro::{Action, Program, Stream, Waiting, path_argument};

pub(super) struct Job {
    stream: Stream,
    batch: viboceros_document::HistoryBatch,
    echo: bool,
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
        let job = self.command_file_job.take();
        self.command_file_prompt = false;
        self.cancel_layer_prompt();
        self.cancel_interactive_command(false);
        for _ in 0..64 {
            if self.command_file_child_idle() {
                break;
            }
            self.cancel_current_prompt_or_selection();
        }
        let idle = self.command_file_child_idle();
        self.command_file_job = job;
        if !idle {
            self.push_log("Error: the previous command prompt could not be canceled".into());
        }
        idle
    }
    fn begin_command_file_job(&mut self) -> bool {
        if self.command_file_job.is_some() {
            return true;
        }
        if !self.cancel_command_file_getters() {
            self.command_file_step_failed = true;
            self.last_command_file_result = Some(false);
            return false;
        }
        match self.document.begin_history_batch("ReadCommandFile") {
            Ok(batch) => {
                self.command_file_job = Some(Job {
                    stream: Stream::default(),
                    batch,
                    echo: true,
                });
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_file_step_failed = true;
                self.last_command_file_result = Some(false);
                false
            }
        }
    }
    pub(super) fn command_file_input(&mut self, text: &str) -> bool {
        let mut words = text.trim().splitn(2, char::is_whitespace);
        let name = words.next().unwrap_or("").trim_start_matches(['_', '-']);
        let is_command = name.eq_ignore_ascii_case("ReadCommandFile");
        if !self.command_file_prompt && !is_command {
            return false;
        }
        let path = if is_command {
            words.next().unwrap_or("").trim()
        } else {
            text.trim()
        };
        if is_command && self.command_file_job.is_none() {
            self.last_command_file_result = None;
        }
        if is_command && path.is_empty() {
            if !self.begin_command_file_job() {
                return true;
            }
            self.command_file_prompt = true;
            self.command_input.clear();
            self.push_log("ReadCommandFile: enter the text filename; Esc cancels".into());
            return true;
        }
        if path.is_empty() || path.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.command_file_prompt = false;
            self.abort_command_file();
            self.command_input.clear();
            return true;
        }
        let result = path_argument(path).and_then(|p| Program::read(&p));
        match result {
            Ok(program) => {
                if !self.begin_command_file_job() {
                    return true;
                }
                if let Err(e) = self.command_file_job.as_mut().unwrap().stream.push(program) {
                    self.push_log(format!("Error: {e}"));
                    self.command_file_step_failed = true;
                    self.abort_command_file();
                }
                self.command_file_prompt = false;
                self.command_input.clear();
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                self.command_file_step_failed = true;
                if self.command_file_job.is_none() {
                    self.last_command_file_result = Some(false);
                }
            }
        }
        true
    }
    fn finish_command_file(&mut self, success: bool) {
        if let Some(job) = self.command_file_job.take() {
            let result = self.document.finish_history_batch(&job.batch);
            self.last_command_file_result = Some(success && result.is_ok());
            if let Err(e) = result {
                self.push_log(format!("Error: {e}"));
            }
        }
        self.command_file_prompt = false;
    }
    pub(super) fn abort_command_file(&mut self) -> bool {
        if self.command_file_job.is_none() && !self.command_file_prompt {
            return false;
        }
        self.command_file_prompt = false;
        self.cancel_command_file_getters();
        self.finish_command_file(false);
        true
    }
    pub(super) fn command_file_point_accepted(&mut self) {
        if self.command_file_job.is_none() {
            return;
        }
        if !self.executing_command_file {
            if let Some(job) = self.command_file_job.as_mut() {
                job.stream.accepted_input();
            }
            self.drain_command_file();
        }
    }
    pub(super) fn command_file_user_step(&mut self) {
        if self.executing_command_file || self.command_file_job.is_none() {
            return;
        }
        if self.command_file_child_idle()
            && self
                .command_file_job
                .as_ref()
                .is_some_and(|j| j.stream.waiting() == Some(Waiting::Input))
        {
            self.abort_command_file();
        } else {
            self.drain_command_file();
        }
    }
    pub(super) fn suppress_command_file_echo(&self) -> bool {
        self.executing_command_file && self.command_file_job.as_ref().is_some_and(|j| !j.echo)
    }
    /// Returns true when another frame can continue without user input.
    pub(super) fn drain_command_file(&mut self) -> bool {
        if self.executing_command_file || self.command_file_job.is_none() {
            return false;
        }
        let started = std::time::Instant::now();
        for _ in 0..4096 {
            let active = !self.command_file_child_idle();
            let Some(job) = self.command_file_job.as_mut() else {
                return false;
            };
            if !self.document.history_batch_is_current(&job.batch) {
                self.abort_command_file();
                return false;
            }
            let action = job.stream.next(active);
            match action {
                Ok(Some(Action::Input(input))) => {
                    self.command_file_step_failed = false;
                    self.executing_command_file = true;
                    self.command_input = input;
                    self.run_command();
                    self.executing_command_file = false;
                    if self.command_file_step_failed {
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
                    self.command_file_job.as_mut().unwrap().echo = on;
                }
                Ok(None) => {
                    if self
                        .command_file_job
                        .as_ref()
                        .unwrap()
                        .stream
                        .finished(active)
                    {
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
                && let Some(job) = &self.command_file_job
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
