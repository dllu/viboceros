//! Edge Analysis controls and camera requests share the console session.
use super::*;

pub(super) struct ZoomPrompt {
    group: viboceros_document::HistoryGroup,
    pub(super) command: &'static str,
}

impl VibocerosApp {
    pub(super) fn start_edge_zoom_options(&mut self, input: &str) {
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else { return };
        if words.next().is_some() {
            return;
        }
        let command = match name.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "zoomnaked" => "ZoomNaked",
            "zoomnonmanifold" => "ZoomNonManifold",
            _ => return,
        };
        let Ok(Some(view)) = self.commands.edge_zoom_view(&self.document) else {
            return;
        };
        if view.current.is_none() {
            self.commands.clear_edge_zoom();
            return;
        }
        match self.document.begin_history_group(command) {
            Ok(group) => {
                self.edge_zoom_prompt = Some(ZoomPrompt { group, command });
                self.log_edge_zoom_options();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn log_edge_zoom_options(&mut self) {
        if let Some(prompt) = &self.edge_zoom_prompt {
            self.push_log(format!(
                "{}: All, Current, Next, Previous or Mark; Enter or Esc ends",
                prompt.command
            ));
        }
    }

    pub(super) fn finish_edge_zoom_options(&mut self, announce: bool) {
        if let Some(prompt) = self.edge_zoom_prompt.take() {
            self.commands.clear_edge_zoom();
            if announce {
                self.push_log(format!("{} ended; accepted marks retained", prompt.command));
            }
        }
    }

    pub(super) fn try_continue_edge_zoom(&mut self, input: &str) -> bool {
        if self.edge_zoom_prompt.is_none() {
            return false;
        }
        if input.is_empty()
            || matches!(
                input.trim_start_matches('_').to_ascii_lowercase().as_str(),
                "enter" | "cancel"
            )
        {
            self.finish_edge_zoom_options(false);
            self.command_input.clear();
            return true;
        }
        let valid = input.split_whitespace().all(|word| {
            matches!(
                word.trim_start_matches('_').to_ascii_lowercase().as_str(),
                "all" | "current" | "next" | "previous" | "mark"
            )
        });
        if !valid {
            if input
                .split_whitespace()
                .next()
                .is_some_and(|name| self.commands.recognizes(name))
            {
                self.finish_edge_zoom_options(false);
                return false;
            }
            self.log_edge_zoom_options();
            self.command_input.clear();
            return true;
        }
        let prompt = self.edge_zoom_prompt.as_mut().unwrap();
        let result =
            self.commands
                .execute_edge_zoom_actions(&mut self.document, input, &mut prompt.group);
        self.push_log(match result {
            Ok(message) => message,
            Err(error) => format!("Error: {error}"),
        });
        self.command_input.clear();
        self.log_edge_zoom_options();
        true
    }

    fn update_edge_zoom_camera(&mut self) {
        let Ok(Some(view)) = self.commands.edge_zoom_view(&self.document) else {
            return;
        };
        if !view.zoom_requested {
            return;
        }
        let fitted = view.bounds().is_none_or(|bounds| {
            self.viewports[self.active_viewport]
                .zoom_bounding_box(bounds)
                .is_ok()
        });
        if fitted {
            self.commands.acknowledge_edge_zoom();
            if self.edge_zoom_prompt.is_none() {
                self.commands.clear_edge_zoom();
            }
        }
    }

    pub(super) fn show_edge_analysis(&mut self, ui: &mut egui::Ui) {
        self.update_edge_zoom_camera();
        let Ok(Some(view)) = self.commands.edge_analysis_view(&self.document) else {
            return;
        };
        if view.zoom_requested {
            if let Some(bounds) = view.bounds() {
                if self.viewports[self.active_viewport]
                    .zoom_bounding_box(bounds)
                    .is_ok()
                {
                    self.commands.acknowledge_edge_analysis_zoom();
                }
            } else {
                self.commands.acknowledge_edge_analysis_zoom();
            }
        }
        let mut open = true;
        let mut action = None;
        let mut color = view.color;
        egui::Window::new("Edge Analysis")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "{} edges on {} objects",
                    view.displayed().count(),
                    view.sources
                ));
                ui.horizontal(|ui| {
                    for mode in [
                        viboceros_command::edge_analysis::Mode::All,
                        viboceros_command::edge_analysis::Mode::Naked,
                        viboceros_command::edge_analysis::Mode::NonManifold,
                    ] {
                        if ui
                            .selectable_label(view.mode == mode, mode.label())
                            .clicked()
                        {
                            action = Some(format!("ShowEdges Show={}", mode.label()));
                        }
                    }
                });
                if ui.color_edit_button_srgb(&mut color).changed() {
                    action = Some(format!(
                        "ShowEdges Color={},{},{}",
                        color[0], color[1], color[2]
                    ));
                }
                ui.horizontal(|ui| {
                    for (label, command) in [
                        ("All", "All"),
                        ("Current", "Current"),
                        ("Previous", "Previous"),
                        ("Next", "Next"),
                    ] {
                        if ui.button(label).clicked() {
                            action = Some(format!("ShowEdges Zoom {command}"));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Mark ends").clicked() {
                        action = Some("ShowEdges Mark".into());
                    }
                    if ui.button("Add selected").clicked() {
                        action = Some("ShowEdges Add".into());
                    }
                    if ui.button("Remove selected").clicked() {
                        action = Some("ShowEdges Remove".into());
                    }
                });
            });
        if !open {
            action = Some("ShowEdgesOff".into());
        }
        if let Some(action) = action {
            self.execute_command(&action);
        }
    }
}
