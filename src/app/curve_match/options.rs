//! Match options shared by typed input, on-screen controls, and acceptance.
use super::*;
use match_curve::Join;
use viboceros_geometry::{CurveBlendContinuity as Continuity, CurveMatchPreserveEnd as Preserve};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Menu {
    MatchTo,
    Mode,
    Preserve,
    Join,
}

impl Menu {
    pub(super) fn hint(self) -> &'static str {
        match self {
            Self::MatchTo => "Match to CurveEnd or PickPoint",
            Self::Mode => "Select Position, Tangency, or Curvature",
            Self::Preserve => "Preserve Nothing, Position, Tangency, or Curvature at the other end",
            Self::Join => "Choose None, Join, or Merge",
        }
    }

    fn option(self) -> &'static str {
        match self {
            Self::MatchTo => "MatchTo",
            Self::Mode => "Mode",
            Self::Preserve => "PreserveOtherEnd",
            Self::Join => "Join",
        }
    }
}

impl Session {
    pub(super) fn option_summary(&self) -> String {
        let options = self.options;
        let base = format!(
            "Mode={:?} PreserveOtherEnd={:?}",
            options.continuity, options.preserve
        );
        match self.match_to {
            MatchTo::CurveEnd => format!(
                "{base} Join={:?} AverageCurves={}",
                options.join,
                yes_no(options.average)
            ),
            MatchTo::PickPoint => base,
            MatchTo::SurfaceEdge => format!(
                "{base} PerpendicularToEdge={}",
                yes_no(options.perpendicular)
            ),
        }
    }
}

impl VibocerosApp {
    pub(super) fn continue_curve_match_options(&mut self, input: &str) {
        let session = self.curve_match_session.as_mut().expect("Match options");
        if input.is_empty() && session.menu.take().is_some() {
            self.log_curve_match();
            return;
        }
        if input.is_empty()
            || input.eq_ignore_ascii_case("Accept")
            || input.eq_ignore_ascii_case("Enter")
        {
            self.accept_curve_match();
            return;
        }
        let key = input.trim_start_matches('_');
        if session.menu.is_none() {
            let menu = [Menu::Mode, Menu::Preserve, Menu::Join]
                .into_iter()
                .find(|menu| {
                    menu.option().eq_ignore_ascii_case(key)
                        || (*menu == Menu::Mode && key.eq_ignore_ascii_case("Continuity"))
                });
            if let Some(menu) = menu {
                session.menu = Some(menu);
                self.log_curve_match();
                return;
            }
        }
        let arguments = if let Some(menu) = session.menu {
            format!("{}={key}", menu.option())
        } else if key.eq_ignore_ascii_case("AverageCurves") {
            format!("AverageCurves={}", yes_no(!session.options.average))
        } else if key.eq_ignore_ascii_case("PerpendicularToEdge") {
            format!(
                "PerpendicularToEdge={}",
                yes_no(!session.options.perpendicular)
            )
        } else if !key.contains('=')
            && let Some((name, value)) = key.split_once(char::is_whitespace)
        {
            format!("{name}={}", value.trim())
        } else {
            key.to_owned()
        };
        match match_curve::parse(
            &arguments.split_whitespace().collect::<Vec<_>>(),
            session.options,
        ) {
            Ok(parsed) if parsed.picks.iter().all(Option::is_none) => {
                session.menu = None;
                self.set_curve_match_options(parsed.options);
            }
            _ => self.push_log(format!("Error: {}", match_curve::USAGE)),
        }
    }

    fn set_curve_match_options(&mut self, mut options: match_curve::Options) {
        let session = self.curve_match_session.as_mut().expect("Match options");
        if options.continuity != Continuity::Curvature && options.join == Join::Merge {
            if session.options.join != Join::Merge {
                self.push_log("Merge requires curvature matching".into());
                return;
            }
            options.join = Join::None;
        }
        session.options = options;
        self.curve_match_options = options;
        self.refresh_curve_match();
    }

    fn accept_curve_match(&mut self) {
        self.validate_curve_match();
        let Some(session) = &self.curve_match_session else {
            return;
        };
        let result = session
            .prepared
            .as_ref()
            .ok_or(CommandError::Usage(
                "Set options to produce a valid Match preview",
            ))
            .and_then(|prepared| prepared.commit(&mut self.document));
        match result {
            Ok(_) => {
                self.curve_match_options.preserve = match_curve::effective_preserve(
                    session.match_to == MatchTo::SurfaceEdge,
                    session.options,
                );
                self.cancel_interactive_command(false);
                self.push_log("Matched curve".into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(in crate::app) fn show_curve_match_controls(&mut self, ui: &mut egui::Ui) {
        let Some(session) = &self.curve_match_session else {
            return;
        };
        if self.plane_prompt.is_some() {
            return;
        }
        if !session.ready() {
            if session.picks.len() == 1 && session.guide.is_none() && !session.object_id_pending {
                let mut target = session.match_to;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Match to");
                    for (value, label) in [
                        (MatchTo::CurveEnd, "Curve end"),
                        (MatchTo::PickPoint, "Curve point"),
                        (MatchTo::SurfaceEdge, "Surface edge"),
                    ] {
                        ui.selectable_value(&mut target, value, label);
                    }
                });
                let session = self.curve_match_session.as_mut().unwrap();
                if target != session.match_to {
                    session.match_to = target;
                    session.menu = None;
                    self.log_curve_match();
                }
            }
            return;
        }
        let previous = session.options;
        let mut options = previous;
        let ready = session.prepared.is_some();
        let match_to = session.match_to;
        let mut accept = false;
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            combo(
                ui,
                "Continuity",
                &mut options.continuity,
                &[
                    (Continuity::Position, "Position"),
                    (Continuity::Tangency, "Tangency"),
                    (Continuity::Curvature, "Curvature"),
                ],
            );
            combo(
                ui,
                "Preserve other end",
                &mut options.preserve,
                &[
                    (Preserve::None, "Nothing"),
                    (Preserve::Position, "Position"),
                    (Preserve::Tangency, "Tangency"),
                    (Preserve::Curvature, "Curvature"),
                ],
            );
            if match_to == MatchTo::CurveEnd {
                ui.checkbox(&mut options.average, "Average curves");
                let choices = [
                    (Join::None, "None"),
                    (Join::Join, "Join"),
                    (Join::Merge, "Merge"),
                ];
                let count = if options.continuity == Continuity::Curvature {
                    3
                } else {
                    2
                };
                combo(ui, "Join", &mut options.join, &choices[..count]);
            } else if match_to == MatchTo::SurfaceEdge {
                ui.checkbox(&mut options.perpendicular, "Perpendicular to edge");
            }
            accept = ui.add_enabled(ready, egui::Button::new("Match")).clicked();
            cancel = ui.button("Cancel").clicked();
        });
        if options != previous {
            self.set_curve_match_options(options);
        }
        if accept {
            self.accept_curve_match();
            self.command_focus_requested = true;
        } else if cancel {
            self.cancel_interactive_command(true);
            self.command_focus_requested = true;
        }
    }
}

fn combo<T: Copy + PartialEq>(ui: &mut egui::Ui, label: &str, value: &mut T, values: &[(T, &str)]) {
    let name = values
        .iter()
        .find_map(|(option, name)| (*option == *value).then_some(*name))
        .unwrap_or("None");
    ui.label(label);
    egui::ComboBox::from_id_salt(label)
        .selected_text(name)
        .show_ui(ui, |ui| {
            for &(option, name) in values {
                ui.selectable_value(value, option, name);
            }
        });
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}
