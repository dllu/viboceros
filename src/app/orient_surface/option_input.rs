//! Numeric presets are separate from the per-placement point getters.
use super::*;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Menu {
    Scale,
    Rotation,
}
#[derive(Clone, Copy)]
pub(super) enum ValuePrompt {
    Uniform,
    Axis(usize),
    Angle,
}
impl VibocerosApp {
    pub(super) fn orient_surface_option_prompt(&mut self) -> bool {
        let s = self.orient_surface_session.as_ref().unwrap();
        if let Some(prompt) = s.value_prompt {
            let (name, value) = match prompt {
                ValuePrompt::Uniform => ("uniform scale", s.options.scale),
                ValuePrompt::Axis(i) => (
                    ["X scale", "Y scale", "Z scale"][i],
                    s.options.axes.unwrap_or([s.options.scale; 3])[i],
                ),
                ValuePrompt::Angle => ("rotation angle", s.options.rotation),
            };
            self.push_log(format!(
                "OrientOnSrf: {name} <{value}>; Enter keeps the value"
            ));
            true
        } else if let Some(menu) = s.menu {
            self.push_log(match menu {Menu::Scale=>"OrientOnSrf scale: Prompt=Yes|No, Uniform=factor, X=factor, Y=factor, Z=factor; Enter returns to placement",Menu::Rotation=>"OrientOnSrf rotation: Prompt=Yes|No, Angle=degrees; Enter returns to placement"}.into());
            true
        } else {
            false
        }
    }
    pub(super) fn continue_orient_surface_option_input(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_surface_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if let Some(prompt) = s.value_prompt {
            let keep = input.is_empty() || word.eq_ignore_ascii_case("Enter");
            let value = if keep {
                match prompt {
                    ValuePrompt::Uniform => Some(s.options.scale),
                    ValuePrompt::Axis(i) => Some(s.options.axes.unwrap_or([s.options.scale; 3])[i]),
                    ValuePrompt::Angle => Some(s.options.rotation),
                }
            } else {
                input.trim_start_matches('_').parse::<f64>().ok()
            };
            let valid = value.filter(|v| {
                v.is_finite()
                    && match prompt {
                        ValuePrompt::Uniform => *v > 0.,
                        ValuePrompt::Angle => v.to_radians().is_finite(),
                        ValuePrompt::Axis(_) => *v != 0.,
                    }
            });
            if let Some(value) = valid {
                let s = self.orient_surface_session.as_mut().unwrap();
                match prompt {
                    ValuePrompt::Uniform => {
                        s.options.scale = value;
                        s.options.axes = None;
                        s.options.scale_prompt = false;
                    }
                    ValuePrompt::Axis(i) => {
                        let mut axes = s.options.axes.unwrap_or([s.options.scale; 3]);
                        axes[i] = value;
                        s.options.axes = Some(axes);
                        s.options.scale_prompt = false;
                    }
                    ValuePrompt::Angle => {
                        s.options.rotation = value;
                        s.options.rotation_prompt = false;
                    }
                }
                s.value_prompt = None;
                s.sync();
                self.commands.remember_orient_surface_options(s.options);
                self.orient_surface_prompt();
            } else {
                self.push_log(
                    "Error: enter a finite preset value; axis scales must be nonzero and uniform scale positive".into(),
                );
            }
            self.command_input.clear();
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.menu.is_some() {
                self.orient_surface_session.as_mut().unwrap().menu = None;
                self.orient_surface_prompt();
                self.command_input.clear();
                return true;
            }
            return false;
        }
        let menu = if word.eq_ignore_ascii_case("Scale") {
            Some(Menu::Scale)
        } else if word.eq_ignore_ascii_case("Rotation") {
            Some(Menu::Rotation)
        } else {
            None
        };
        if let Some(menu) = menu {
            let s = self.orient_surface_session.as_mut().unwrap();
            s.menu = Some(menu);
            self.orient_surface_prompt();
            self.command_input.clear();
            return true;
        }
        let menu = s.menu;
        let prompt = if word.eq_ignore_ascii_case("Uniform") {
            Some(ValuePrompt::Uniform)
        } else if word.eq_ignore_ascii_case("Angle") {
            Some(ValuePrompt::Angle)
        } else if word.eq_ignore_ascii_case("X") {
            Some(ValuePrompt::Axis(0))
        } else if word.eq_ignore_ascii_case("Y") {
            Some(ValuePrompt::Axis(1))
        } else if word.eq_ignore_ascii_case("Z") {
            Some(ValuePrompt::Axis(2))
        } else {
            None
        };
        if let Some(prompt) = prompt {
            self.orient_surface_session.as_mut().unwrap().value_prompt = Some(prompt);
            self.orient_surface_prompt();
            self.command_input.clear();
            return true;
        }
        if let Some(menu) = menu {
            let translated = if word.eq_ignore_ascii_case("Prompt") {
                format!(
                    "{}Prompt=Yes",
                    if menu == Menu::Scale {
                        "Scale"
                    } else {
                        "Rotation"
                    }
                )
            } else if word.to_ascii_lowercase().starts_with("prompt=") {
                format!(
                    "{}{}",
                    if menu == Menu::Scale {
                        "Scale"
                    } else {
                        "Rotation"
                    },
                    word
                )
            } else if input.parse::<f64>().is_ok() {
                format!(
                    "{}={input}",
                    if menu == Menu::Scale {
                        "Uniform"
                    } else {
                        "Angle"
                    }
                )
            } else {
                input.into()
            };
            let args = translated.split_whitespace().collect::<Vec<_>>();
            let allowed = args.iter().all(|arg| {
                let key = arg
                    .split('=')
                    .next()
                    .unwrap()
                    .trim_start_matches(['_', '-'])
                    .to_ascii_lowercase();
                match menu {
                    Menu::Scale => [
                        "scale",
                        "uniform",
                        "scalexyz",
                        "x",
                        "y",
                        "z",
                        "scalex",
                        "scaley",
                        "scalez",
                        "scaleprompt",
                    ]
                    .contains(&key.as_str()),
                    Menu::Rotation => {
                        ["rotation", "angle", "rotationprompt"].contains(&key.as_str())
                    }
                }
            });
            if !allowed {
                self.push_log(
                    "Error: enter an option from the current scale or rotation menu".into(),
                );
                self.command_input.clear();
                return true;
            }
            match solver::options(&args, s.options) {
                Ok((rest, o, r))
                    if rest.is_empty()
                        && r.id.is_none()
                        && r.face.is_none()
                        && r.name.is_none() =>
                {
                    let s = self.orient_surface_session.as_mut().unwrap();
                    s.options = o;
                    s.sync();
                    self.commands.remember_orient_surface_options(o);
                    self.orient_surface_prompt();
                }
                _ => self.push_log("Error: enter a valid scale or rotation option".into()),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
}
