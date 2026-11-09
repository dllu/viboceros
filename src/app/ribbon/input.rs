//! Keyboard options and numeric distance getters.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn continue_ribbon(&mut self, input: &str) -> bool {
        if self.ribbon_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if viboceros_command::interface::parse(input).is_some() {
            return false;
        }
        let option = input.trim().trim_start_matches(['_', '-']);
        let local = [
            "Distance",
            "Corner",
            "OutputLayer",
            "Tolerance",
            "Loose",
            "InCPlane",
            "BothSides",
        ];
        if self.ribbon_session.as_ref().unwrap().menu.is_none()
            && !local.iter().any(|n| {
                option
                    .split_once('=')
                    .map_or(option, |p| p.0)
                    .eq_ignore_ascii_case(n)
            })
            && self
                .commands
                .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        if !self.validate_ribbon() {
            return true;
        }
        let menu = self.ribbon_session.as_ref().unwrap().menu;
        if menu.is_none()
            && solver::length(input, self.document.units()).is_err()
            && viboceros_drafting::PointInput::parse_with_units(input, self.document.units())
                .is_some()
        {
            return false;
        }
        if input.is_empty() {
            if menu == Some("Distance") {
                let s = self.ribbon_session.as_mut().unwrap();
                s.menu = None;
                s.distance_start = None;
                self.active_command = Some(InteractiveCommand::Ribbon { anchor: None });
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            self.ribbon_prompt();
            return true;
        }
        if menu == Some("Distance") {
            if let Ok(d) = solver::length(input, self.document.units()) {
                if d > 0. && d.is_finite() {
                    let s = self.ribbon_session.as_mut().unwrap();
                    s.options.distance = d;
                    s.menu = None;
                    s.distance_start = None;
                    self.active_command = Some(InteractiveCommand::Ribbon { anchor: None });
                    self.command_input.clear();
                    self.ribbon_prompt();
                    return true;
                }
                self.push_log("Ribbon distance must be positive and finite".into());
                return true;
            }
            // Shared point input resolves world, relative, units and snaps.
            if viboceros_drafting::PointInput::parse_with_units(input, self.document.units())
                .is_some()
                || self
                    .commands
                    .recognizes(input.split_whitespace().next().unwrap_or(""))
            {
                return false;
            }
            self.push_log("Enter a positive distance or a distance point".into());
            return true;
        }
        for name in ["Distance", "Corner", "OutputLayer", "Tolerance"] {
            if option.eq_ignore_ascii_case(name) {
                let s = self.ribbon_session.as_mut().unwrap();
                if s.options.loose && matches!(name, "Corner" | "Tolerance") {
                    self.push_log("Corner and Tolerance are unavailable with Loose=Yes".into());
                } else {
                    s.menu = Some(name);
                }
                self.command_input.clear();
                self.ribbon_prompt();
                return true;
            }
        }
        let s = self.ribbon_session.as_ref().unwrap();
        let text = if let Some(menu) = menu {
            format!("{menu}={input}")
        } else if option.eq_ignore_ascii_case("Loose") {
            format!("Loose={}", if s.options.loose { "No" } else { "Yes" })
        } else if option.eq_ignore_ascii_case("InCPlane") {
            format!(
                "InCPlane={}",
                if s.options.in_cplane { "No" } else { "Yes" }
            )
        } else {
            input.to_owned()
        };
        let args = text.split_whitespace().collect::<Vec<_>>();
        let context = viboceros_command::CommandContext {
            construction_plane: s.plane,
        };
        match solver::parse(&args, s.options, self.document.units(), context) {
            Ok(p) => {
                let s = self.ribbon_session.as_mut().unwrap();
                s.options = p.options;
                s.menu = None;
                self.command_input.clear();
                if let Some(point) = p.side {
                    self.accept_ribbon_point(point);
                } else {
                    self.ribbon_prompt();
                }
                true
            }
            Err(e) => {
                if menu.is_none()
                    && viboceros_drafting::PointInput::parse_with_units(
                        input,
                        self.document.units(),
                    )
                    .is_some()
                {
                    return false;
                }
                self.push_log(format!("Error: {e}"));
                true
            }
        }
    }
}
