//! Weight's picker and numeric dialog keep previews outside model history.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::weight::Prepared;

#[derive(Debug)]
pub(super) struct Session {
    prepared: Option<Prepared>,
    value: f64,
    inline: bool,
    dialog: bool,
    range: [f64; 2],
    scene: Option<Document>,
    background: Option<surface_rebuild::Background>,
    error: Option<String>,
}
impl Session {
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
}
impl VibocerosApp {
    pub(super) fn initialize_weight_prompt(&mut self, input: &str) {
        let Some(prompt) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "Weight")
        else {
            return;
        };
        let words = input.split_whitespace().collect::<Vec<_>>();
        let value = viboceros_command::weight::parse(&words[1..]).ok().flatten();
        let prepared = (prompt.phase == ObjectPromptPhase::Options)
            .then(|| Prepared::capture(&self.document))
            .transpose();
        let prepared = match prepared {
            Ok(p) => p,
            Err(e) => {
                self.cancel_object_prompt(false);
                self.push_log(format!("Error: {e}"));
                return;
            }
        };
        self.weight_prompt = Some(Session {
            value: value.unwrap_or_else(|| prepared.as_ref().map_or(1., Prepared::default_weight)),
            inline: value.is_some(),
            prepared,
            dialog: !words[0].trim_start_matches('_').starts_with('-'),
            range: [0.1, 10.],
            scene: None,
            background: None,
            error: None,
        });
    }

    pub(super) fn continue_weight(&mut self, input: &str) -> bool {
        let Some(prompt) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "Weight")
        else {
            return false;
        };
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            self.command_input.clear();
            return true;
        }
        let first = input.split_whitespace().next().unwrap_or("");
        if self.commands.recognizes(first) {
            return false;
        }
        if prompt.phase == ObjectPromptPhase::Selecting {
            if !input.is_empty() {
                self.push_log("Select curve or surface control points; Enter continues".into());
                self.command_input.clear();
                return true;
            }
            let prepared = match Prepared::capture(&self.document) {
                Ok(p) => p,
                Err(e) => {
                    self.cancel_object_prompt(false);
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            };
            let session = self.weight_prompt.as_mut().unwrap();
            if !session.inline {
                session.value = prepared.default_weight();
            }
            session.prepared = Some(prepared);
            self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
            self.command_input.clear();
            if session.inline {
                self.finish_weight();
            } else {
                self.log_weight_value();
            }
            return true;
        }
        if input.is_empty() {
            self.finish_weight();
            self.command_input.clear();
            return true;
        }
        match input.parse::<f64>() {
            Ok(value) if value.is_finite() && value > 0. => {
                self.weight_prompt.as_mut().unwrap().value = value;
                self.finish_weight();
            }
            Ok(_) => {
                self.cancel_object_prompt(false);
                self.push_log("Error: Weight must be positive and finite".into());
            }
            Err(_) => self.push_log("Enter a positive control point weight".into()),
        }
        self.command_input.clear();
        true
    }

    fn log_weight_value(&mut self) {
        if let Some(p) = self
            .weight_prompt
            .as_ref()
            .and_then(|s| s.prepared.as_ref())
        {
            self.push_log(if p.weights_vary() {
                "Control point weights vary; Enter uses 1".into()
            } else {
                format!("New control point weight <{}>", p.default_weight())
            });
        }
    }

    fn finish_weight(&mut self) {
        let Some(session) = self.weight_prompt.as_ref() else {
            return;
        };
        let Some(prepared) = session.prepared.as_ref() else {
            return;
        };
        if !prepared.is_current(&self.document) {
            self.cancel_object_prompt(false);
            self.push_log("Weight sources changed; start the command again".into());
            return;
        }
        let weight = session.value;
        let post = self.object_prompt.as_ref().is_some_and(|p| p.postselected);
        let old = self
            .document
            .selected_control_points()
            .map(|(p, _)| p.object)
            .collect::<std::collections::BTreeSet<_>>();
        let sources = self
            .document
            .objects()
            .filter(|o| old.contains(&o.id()))
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        let command = format!("Weight {weight}");
        match self.commands.execute(&mut self.document, &command) {
            Ok(message) => {
                if post {
                    let unchanged = sources
                        .iter()
                        .filter(|(id, g)| {
                            self.document
                                .object(*id)
                                .is_some_and(|o| o.geometry_snapshot().shares_storage_with(g))
                        })
                        .map(|(id, _)| *id)
                        .collect::<std::collections::BTreeSet<_>>();
                    let kept = self
                        .document
                        .selected_control_points()
                        .filter(|(p, _)| !unchanged.contains(&p.object))
                        .map(|(p, _)| p)
                        .collect::<Vec<_>>();
                    self.document.clear_control_point_selection();
                    if let Err(e) = self
                        .document
                        .select_control_points(kept, SelectionMode::Add)
                    {
                        self.push_log(format!("Error: {e}"));
                    }
                }
                self.object_prompt = None;
                self.weight_prompt = None;
                self.push_log(format!("> {command}"));
                self.push_log(message);
            }
            Err(e) => {
                self.cancel_object_prompt(false);
                self.push_log(format!("Error: {e}"));
            }
        }
    }

    pub(super) fn set_weight_preview(&mut self, value: f64) {
        let Some(session) = self.weight_prompt.as_mut() else {
            return;
        };
        session.value = value;
        session.scene = None;
        session.error = None;
        let Some(prepared) = &session.prepared else {
            return;
        };
        match prepared.preview(&self.document, value) {
            Ok(scene) => {
                session.scene = Some(scene);
                session.background = Some(surface_rebuild::Background::capture(&self.document));
            }
            Err(e) => session.error = Some(e.to_string()),
        }
    }

    pub(super) fn validate_weight_preview(&mut self) {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "Weight")
        {
            self.weight_prompt = None;
            return;
        }
        let Some(session) = &self.weight_prompt else {
            return;
        };
        if session
            .prepared
            .as_ref()
            .is_some_and(|p| !p.is_current(&self.document))
        {
            self.cancel_object_prompt(false);
            self.push_log("Weight sources changed; start the command again".into());
        } else if session
            .background
            .as_ref()
            .is_some_and(|b| !b.is_current(&self.document))
        {
            let value = session.value;
            self.set_weight_preview(value);
        }
    }

    pub(super) fn show_weight_dialog(&mut self, ctx: &egui::Context) {
        let Some(session) = self
            .weight_prompt
            .as_ref()
            .filter(|s| s.dialog && s.prepared.is_some())
        else {
            return;
        };
        let (mut value, mut range) = (session.value, session.range);
        let (mut open, mut changed, mut accept, mut cancel) = (true, false, false, false);
        egui::Window::new("Control point weight")
            .id(egui::Id::new("weight_dialog"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                if session.prepared.as_ref().unwrap().weights_vary() {
                    ui.label("Selected weights vary");
                }
                ui.horizontal(|ui| {
                    ui.label("Weight");
                    changed |= ui
                        .add(egui::DragValue::new(&mut value).speed(0.05))
                        .changed();
                });
                if range[0].is_finite()
                    && range[1].is_finite()
                    && range[0] > 0.
                    && range[0] < range[1]
                {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut value, range[0]..=range[1])
                                .clamping(egui::SliderClamping::Edits)
                                .show_value(false),
                        )
                        .changed();
                }
                ui.horizontal(|ui| {
                    ui.label("Range");
                    ui.add(egui::DragValue::new(&mut range[0]).speed(0.1));
                    ui.add(egui::DragValue::new(&mut range[1]).speed(0.1));
                });
                if let Some(error) = &session.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    accept = ui
                        .add_enabled(value.is_finite() && value > 0., egui::Button::new("OK"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if let Some(s) = self.weight_prompt.as_mut() {
            s.range = range;
        }
        if cancel || !open {
            self.cancel_object_prompt(true);
        } else if accept || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.weight_prompt.as_mut().unwrap().value = value;
            self.finish_weight();
        } else if changed {
            self.set_weight_preview(value);
        }
    }
}
