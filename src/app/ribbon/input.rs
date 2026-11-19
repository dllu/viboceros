//! Keyboard options and numeric distance getters.
use super::*;
use crate::app::object_selection::ObjectPromptPhase;

impl VibocerosApp {
    pub(in crate::app) fn continue_ribbon_source_trim(&mut self, input: &str) -> bool {
        if !is_trim_option(input) {
            return false;
        }
        let Some(pending) = self.object_prompt.as_ref().filter(|pending| {
            pending.description.command == "Ribbon" && pending.phase == ObjectPromptPhase::Selecting
        }) else {
            return false;
        };
        let current = pending.command_override.as_deref().unwrap_or("Ribbon");
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let units = self.document.units();
        let options = self.commands.ribbon_options();
        let result = solver::parse(
            &current.split_whitespace().skip(1).collect::<Vec<_>>(),
            options,
            units,
            context,
        )
        .and_then(|mut ribbon| {
            ribbon.options = trim_options(input, ribbon.options, units, context)?;
            Ok(ribbon)
        });
        match result {
            Ok(ribbon) => {
                self.object_prompt.as_mut().unwrap().command_override =
                    Some(ribbon_command_line(ribbon));
                self.commands.remember_ribbon_trim(ribbon.options.trim);
                self.push_log(format!(
                    "Ribbon Trim={}",
                    if ribbon.options.trim { "Yes" } else { "No" }
                ));
                self.log_object_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn continue_ribbon(&mut self, input: &str) -> bool {
        if self.ribbon_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if viboceros_command::interface::parse(input).is_some() {
            return false;
        }
        let option = input.trim().trim_start_matches(['_', '-']);
        if is_subcurve_option(option) {
            self.push_log("SubCrv is available while selecting the Ribbon source".into());
            self.command_input.clear();
            return true;
        }
        let local = [
            "Distance",
            "Corner",
            "OutputLayer",
            "Tolerance",
            "Loose",
            "InCPlane",
            "BothSides",
            "ThroughPoint",
            "Trim",
        ];
        let option_name = option
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('=')
            .next()
            .unwrap();
        if self.ribbon_session.as_ref().unwrap().menu.is_none()
            && !local.iter().any(|n| option_name.eq_ignore_ascii_case(n))
            && self
                .commands
                .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            return false;
        }
        if !self.validate_ribbon() {
            return true;
        }
        if is_trim_option(option) && self.ribbon_session.as_ref().unwrap().menu.is_none() {
            let session = self.ribbon_session.as_mut().unwrap();
            let context = viboceros_command::CommandContext {
                construction_plane: session.plane,
            };
            match trim_options(input, session.options, self.document.units(), context) {
                Ok(options) => {
                    session.options = options;
                    self.commands.remember_ribbon_trim(options.trim);
                    self.ribbon_prompt();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
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
                    s.through_point = false;
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
                if p.through_point {
                    s.through_point = true;
                } else if args.iter().any(|arg| {
                    let name = arg.split_once('=').map_or(*arg, |(name, _)| name);
                    name.trim_start_matches('_')
                        .eq_ignore_ascii_case("Distance")
                        || solver::length(arg, self.document.units()).is_ok()
                }) {
                    s.through_point = false;
                }
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

fn is_trim_option(input: &str) -> bool {
    input.split_whitespace().next().is_some_and(|word| {
        word.split('=')
            .next()
            .unwrap()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Trim")
    })
}

fn trim_options(
    input: &str,
    options: Options,
    units: &viboceros_geometry::LengthUnitSystem,
    context: viboceros_command::CommandContext,
) -> Result<Options, viboceros_command::CommandError> {
    if options.loose {
        return Err(viboceros_command::CommandError::Usage(
            "Set Loose=No to edit Trim",
        ));
    }
    let words = input.split_whitespace().collect::<Vec<_>>();
    if let [word] = words.as_slice()
        && !word.contains('=')
    {
        return Ok(Options {
            trim: !options.trim,
            ..options
        });
    }
    let parsed = solver::parse(&words, options, units, context)?;
    if parsed.side.is_some() || parsed.through_point {
        return Err(viboceros_command::CommandError::Usage(
            "Trim=Yes or Trim=No",
        ));
    }
    Ok(parsed.options)
}
