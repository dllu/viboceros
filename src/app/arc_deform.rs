//! Arc output controls wrap the existing construction prompts and history owner.
use super::*;
use viboceros_command::arc_deform_input::Settings;
impl VibocerosApp {
    pub(super) fn try_start_arc_deform(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        let [command, first, rest @ ..] = words.as_slice() else {
            return false;
        };
        if !["Arc", "A"].iter().any(|w| {
            command
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case(w)
        }) || !first
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Deformable")
        {
            return false;
        }
        let mut settings = Settings::new(self.commands.arc_deformation());
        settings.option("Deformable").unwrap();
        let mut handoff = None;
        for (index, word) in rest.iter().enumerate() {
            if index == rest.len() - 1
                && settings.pending().is_none()
                && ["Center", "StartPoint", "Tangent", "Extension"]
                    .iter()
                    .any(|k| word.trim_start_matches('_').eq_ignore_ascii_case(k))
            {
                handoff = Some(*word);
                break;
            }
            let accepted = if settings.pending().is_some() {
                if word.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
                    settings.default_value()
                } else {
                    word.parse::<f64>()
                        .map_err(|_| ())
                        .and_then(|n| settings.number(n).map_err(|_| ()))
                        .map_err(|_| {
                            viboceros_command::CommandError::Usage(
                                "Arc setting requires an integer",
                            )
                        })
                }
                .map(|_| true)
            } else {
                settings.option(word)
            };
            if !matches!(accepted, Ok(true)) {
                return false;
            }
        }
        self.cancel_interactive_command(false);
        self.arc_deform_session = Some(settings);
        self.arc_deform_prompt();
        if let Some(word) = handoff {
            self.try_continue_arc_deform(word);
        }
        true
    }
    fn arc_deform_prompt(&mut self) {
        let s = self.arc_deform_session.unwrap();
        self.active_command = Some(InteractiveCommand::ArcDeform {
            setting: s.pending(),
        });
        self.push_log(format!("Arc Deformable: {}; Esc cancels", s.prompt()));
    }
    pub(super) fn prepare_arc_deform_point(&mut self) -> bool {
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ArcDeform { .. })
        ) {
            if self
                .arc_deform_session
                .is_some_and(|s| s.pending().is_some())
            {
                self.push_log("Error: enter an integer setting".into());
                return false;
            }
            self.active_command = Some(InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                clockwise: true,
            });
        }
        true
    }
    pub(super) fn try_continue_arc_deform(&mut self, text: &str) -> bool {
        let word = text.trim().trim_start_matches(['_', '-']);
        let root = matches!(
            self.active_command,
            Some(
                InteractiveCommand::ArcDeform { .. }
                    | InteractiveCommand::ArcCenter {
                        center: None,
                        start: None,
                        ..
                    }
            )
        );
        if self.arc_deform_session.is_none() {
            if !root || !word.eq_ignore_ascii_case("Deformable") {
                return false;
            }
            let mut s = Settings::new(self.commands.arc_deformation());
            s.option("Deformable").unwrap();
            self.arc_deform_session = Some(s);
            self.arc_deform_prompt();
            self.command_input.clear();
            return true;
        }
        if !root {
            return false;
        }
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if self.arc_deform_session.unwrap().pending().is_some() {
            let result = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.arc_deform_session.as_mut().unwrap().default_value()
            } else {
                match text.trim().parse::<f64>() {
                    Ok(n) => self.arc_deform_session.as_mut().unwrap().number(n),
                    Err(_) => Err(viboceros_command::CommandError::Usage(
                        "Arc setting requires an integer",
                    )),
                }
            };
            match result {
                Ok(()) => self.arc_deform_prompt(),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else if word.eq_ignore_ascii_case("Extension") {
            self.start_arc_extension(true);
        } else if word.eq_ignore_ascii_case("Tangent") {
            self.start_arc_tangent_from_prompt();
        } else if ["Center", "StartPoint"]
            .iter()
            .any(|v| word.eq_ignore_ascii_case(v))
        {
            let settings = self.arc_deform_session;
            if !self.try_start_interactive_command(&format!("Arc {word}")) {
                return false;
            }
            self.arc_deform_session = settings;
        } else {
            match self.arc_deform_session.as_mut().unwrap().option(word) {
                Ok(true) => self.arc_deform_prompt(),
                Ok(false) => return false,
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn start_arc_tangent_from_prompt(&mut self) {
        let output = self.arc_deform_session;
        self.start_tangent_arc();
        self.arc_deform_session = output;
    }
    pub(super) fn start_tilted_arc_from_prompt(&mut self, center: Point3) {
        let output = self.arc_deform_session;
        self.start_arc_tilted(center);
        self.arc_deform_session = output;
    }
    pub(super) fn arc_preview(&self) -> Option<viboceros_command::arc_input::Preview> {
        use viboceros_command::arc_input::Preview;
        match self.active_command? {
            InteractiveCommand::ArcCenter {
                center: Some(center),
                start: Some(start),
                clockwise,
            }
            | InteractiveCommand::ArcStartCenter {
                center: Some(center),
                start: Some(start),
                clockwise,
            } => Some(Preview::Center {
                center,
                start,
                midpoint: false,
                clockwise,
            }),
            InteractiveCommand::ArcCenterMidpoint {
                center: Some(center),
                midpoint: Some(start),
                clockwise,
            } => Some(Preview::Center {
                center,
                start,
                midpoint: true,
                clockwise,
            }),
            InteractiveCommand::Arc {
                points: [Some(start), Some(through)],
            } => Some(Preview::ThreePoints { start, through }),
            InteractiveCommand::ArcStartPoint {
                start: Some(start),
                end: Some(end),
            } => Some(Preview::StartEnd { start, end }),
            InteractiveCommand::ArcStartDirection {
                start: Some(start),
                tangent: Some(tangent),
            } => Some(Preview::Direction { start, tangent }),
            _ => None,
        }
    }
    pub(super) fn finish_arc_command(&mut self, command: &str, plane: Option<Frame3>) -> bool {
        let plane = plane
            .or(self.arc_length_session.map(|s| s.plane))
            .or(self.drafting_plane)
            .or_else(|| Some(self.viewports[self.active_viewport].construction_plane()));
        let command = if let Some(s) = self.arc_deform_session {
            let d = s.output().deformation;
            if d.point_count <= d.degree {
                self.push_log("Error: deformable Arc point count must exceed its degree".into());
                return false;
            }
            s.output().wrap_command(command)
        } else {
            command.into()
        };
        self.try_execute_command_with_plane(&command, plane)
    }
    pub(super) fn execute_arc_command(&mut self, command: &str) {
        self.finish_arc_command(command, None);
    }
}
