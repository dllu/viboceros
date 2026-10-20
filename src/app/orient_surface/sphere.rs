//! Sphere input shares surface picking, immutable source ownership and history.
use super::*;
impl VibocerosApp {
    pub(super) fn continue_splop_sphere_input(&mut self, input: &str) -> bool {
        let Some(s) = self
            .orient_surface_session
            .as_ref()
            .filter(|s| s.kind == Kind::Splop)
        else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            match s.phase {
                Phase::Reference => {
                    self.continue_splop_sphere_input("1");
                }
                Phase::Placement(PreviewPhase::Sphere(_, _)) => {
                    if let Some(point) = s.cursor {
                        self.accept_orient_surface_point(point);
                    } else {
                        self.push_log("Pick a sphere direction on the destination plane".into());
                    }
                }
                Phase::Placement(PreviewPhase::Target) => {
                    let s = self.orient_surface_session.as_mut().unwrap();
                    if s.applied
                        && s.options.copy
                        && let Err(e) = viboceros_command::splop::finish_copies(
                            &mut self.document,
                            &mut s.group,
                            &s.sources,
                        )
                    {
                        self.push_log(format!("Error: {e}"));
                        return true;
                    }
                    self.cancel_interactive_command(true);
                }
                _ => self.push_log("Pick the next sphere reference".into()),
            }
            self.command_input.clear();
            return true;
        }
        if let Some(toggle) = s.toggle {
            let input = format!("{toggle}={input}");
            let mut options = viboceros_command::splop::Options {
                copy: s.options.copy,
                rigid: s.options.rigid,
                flip: s.options.flip,
            };
            if let Err(error) = options.accept(&input) {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
            self.orient_surface_session.as_mut().unwrap().toggle = None;
            return self.continue_splop_sphere_input(&input);
        }
        if let Some(name) = ["Copy", "Rigid", "Flip"]
            .into_iter()
            .find(|name| word.eq_ignore_ascii_case(name))
        {
            self.orient_surface_session.as_mut().unwrap().toggle = Some(name);
            self.push_log(format!("{name}: Yes or No"));
            self.command_input.clear();
            return true;
        }
        if input.contains('=') {
            let args = input.split_whitespace().collect::<Vec<_>>();
            let allowed = args.iter().all(|word| {
                word.split_once('=').is_some_and(|(name, _)| {
                    ["copy", "rigid", "flip", "surface", "face"].contains(
                        &name
                            .trim_start_matches(['_', '-'])
                            .to_ascii_lowercase()
                            .as_str(),
                    )
                })
            });
            let parsed = allowed.then(|| solver::options(&args, s.options));
            match parsed {
                Some(Ok((rest, options, reference))) if rest.is_empty() => {
                    let session = self.orient_surface_session.as_mut().unwrap();
                    session.options = options;
                    session.sync();
                    if let Some(id) = reference.id {
                        self.accept_orient_surface(id, reference.face);
                    } else {
                        self.orient_surface_prompt();
                    }
                }
                _ => self.push_log("Error: use Copy, Rigid, Flip, or Surface=id Face=index".into()),
            }
            self.command_input.clear();
            return true;
        }
        let value = if let Ok(number) = input.parse::<f64>() {
            Some(number)
        } else if let Some(Ok(viboceros_drafting::PointConstraintInput::Distance(number))) =
            viboceros_drafting::PointConstraintInput::parse_with_units(input, self.document.units())
        {
            Some(number)
        } else {
            None
        };
        if let Some(radius) = value {
            if !radius.is_finite() || radius <= 0. {
                self.push_log("Error: sphere radius must be finite and positive".into());
            } else {
                match s.phase {
                    Phase::Reference => {
                        let point = s
                            .plane
                            .x_axis()
                            .as_vector()
                            .scaled(radius)
                            .and_then(|v| s.base.unwrap().translated(v));
                        match point {
                            Ok(p) => {
                                self.accept_orient_surface_point(p);
                            }
                            Err(e) => self.push_log(format!("Error: {e}")),
                        }
                    }
                    Phase::Placement(PreviewPhase::Sphere(pose, _)) => {
                        let s = self.orient_surface_session.as_mut().unwrap();
                        s.options.scale = radius / s.placement.as_ref().unwrap().reference_length;
                        s.phase = Phase::Placement(PreviewPhase::Sphere(pose, Some(radius)));
                        s.sync();
                        self.orient_surface_prompt();
                    }
                    _ => self.push_log("Pick the sphere center before entering its radius".into()),
                }
            }
            self.command_input.clear();
            return true;
        }
        false
    }
}
