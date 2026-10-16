//! Point deviation uses the common two-set picker and a live analysis panel.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::{ObjectSelectionFilter, point_deviation::parse_options};
use viboceros_document::{ObjectId, SelectionMode};
impl VibocerosApp {
    pub(super) fn try_start_point_deviation(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("PointDeviation")
        }) {
            return false;
        }
        let args = words.collect::<Vec<_>>();
        if parse_options(self.commands.point_deviation_options(), &args).is_err() {
            return false;
        }
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::DeviationSources.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            deviation: Some(input.into()),
            first: (!first.is_empty()).then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            projection: None,
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }
    pub(super) fn continue_point_deviation(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        let normalized = input.trim_start_matches('_').to_ascii_lowercase();
        if input.is_empty() {
            let ids = self
                .document
                .selected_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if ids.is_empty() {
                self.push_log(prompt.hint().into());
                self.command_input.clear();
                return true;
            }
            if let Some(first) = &prompt.first {
                let command = format!(
                    "{} Sources={} Targets={}",
                    prompt.deviation.as_ref().unwrap(),
                    super::project::project_ids(first),
                    super::project::project_ids(&ids)
                );
                if !self.try_execute_command(&command) {
                    self.intersection_prompt = Some(prompt);
                    self.push_log("Adjust the target objects; Enter retries".into());
                } else {
                    self.document.clear_selection();
                }
            } else {
                prompt.first = Some(ids);
                self.document.clear_selection();
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
        } else if normalized == "selnone" {
            self.document.clear_selection();
        } else if normalized == "selall" {
            let ids = self
                .document
                .selectable_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
        } else if self.commands.recognizes(
            input
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['_', '-']),
        ) {
            self.cancel_intersection_prompt(true);
            return false;
        } else if let Ok(ids) = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>()
        {
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
        } else {
            self.push_log(prompt.hint().into());
        }
        self.command_input.clear();
        true
    }
    pub(super) fn show_point_deviation(&mut self, ui: &mut egui::Ui) {
        let view = match self.commands.point_deviation_view(&self.document) {
            Ok(Some(v)) if v.visible => v,
            Err(error) => {
                let mut end = false;
                egui::Window::new("Point Deviation").show(ui.ctx(), |ui| {
                    ui.label(error.to_string());
                    end = ui.button("End").clicked();
                });
                if end {
                    self.execute_command("PointDeviation End");
                }
                return;
            }
            _ => return,
        };
        let mut o = view.options.clone();
        let mut open = true;
        let mut action: Option<String> = None;
        egui::Window::new("Point Deviation")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() {
                        action = Some("PointDeviation".into())
                    }
                    if ui.button("End").clicked() {
                        action = Some("PointDeviation End".into())
                    }
                });
                egui::Grid::new("point-deviation-options").show(ui, |ui| {
                    for (label, value) in [
                        ("Good point", &mut o.good),
                        ("Bad point", &mut o.bad),
                        ("Ignore", &mut o.ignore),
                        ("Proximity angle", &mut o.proximity_angle),
                        ("Hair scale", &mut o.hair_scale),
                    ] {
                        ui.label(label);
                        ui.add(
                            egui::DragValue::new(value)
                                .speed(0.01)
                                .range(0. ..=f64::MAX),
                        );
                        ui.end_row();
                    }
                    ui.label("Good color");
                    ui.color_edit_button_srgb(&mut o.good_color);
                    ui.end_row();
                    ui.label("Bad color");
                    ui.color_edit_button_srgb(&mut o.bad_color);
                    ui.end_row();
                    ui.label("Curve divisions");
                    ui.add(egui::DragValue::new(&mut o.curve_divisions).range(1..=10000));
                    ui.end_row();
                    ui.label("Surface samples per span");
                    ui.add(egui::DragValue::new(&mut o.surface_samples).range(1..=128));
                    ui.end_row();
                });
                ui.checkbox(&mut o.display_hair, "Display hair");
                ui.checkbox(&mut o.ignore_bad, "Ignore bad points in display");
                ui.checkbox(&mut o.automatic, "Automatically apply geometry changes");
                if o.validate().is_err() {
                    ui.colored_label(
                        egui::Color32::RED,
                        "Require 0 ≤ Good ≤ Bad ≤ Ignore and angle ≤ 180°",
                    );
                }
                if let Some(s) = &view.statistics {
                    ui.separator();
                    for (label, value) in [
                        ("Point count", s.count.to_string()),
                        ("Mean distance", s.mean.to_string()),
                        ("Median distance", s.median.to_string()),
                        ("Standard deviation", s.standard_deviation.to_string()),
                        ("Minimum distance", s.minimum.to_string()),
                        ("Maximum distance", s.maximum.to_string()),
                    ] {
                        ui.label(format!("{label}: {value}"));
                    }
                } else {
                    ui.label("No points within the ignore distance");
                }
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        action = Some("PointDeviation Apply".into())
                    }
                    if ui.button("Make hair permanent").clicked() {
                        action = Some("PointDeviation MakePermanent".into())
                    }
                    if ui.button("Copy statistics").clicked() {
                        ui.ctx().copy_text(view.report());
                    }
                });
            });
        if o != view.options && o.validate().is_ok() {
            self.execute_command(&format!("PointDeviation Good={} Bad={} Ignore={} ProximityAngle={} HairScale={} DisplayHair={} IgnoreBad={} Automatic={} CurveDivisions={} SurfaceSamples={} GoodColor={},{},{} BadColor={},{},{}",o.good,o.bad,o.ignore,o.proximity_angle,o.hair_scale,if o.display_hair{"Yes"}else{"No"},if o.ignore_bad{"Yes"}else{"No"},if o.automatic{"Yes"}else{"No"},o.curve_divisions,o.surface_samples,o.good_color[0],o.good_color[1],o.good_color[2],o.bad_color[0],o.bad_color[1],o.bad_color[2]));
        }
        if !open {
            action = Some("PointDeviation Visible=No".into())
        }
        if let Some(action) = action {
            if action == "PointDeviation" {
                self.try_start_point_deviation(&action);
            } else {
                self.execute_command(&action);
            }
        }
    }
}
