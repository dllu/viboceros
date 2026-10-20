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
                Phase::Reference if s.sphere_input.is_some_and(|input| input.requests_size()) => {
                    let size = s.sphere_input.unwrap().default_size().unwrap();
                    self.continue_splop_sphere_input(&size.to_string());
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
        if matches!(s.phase, Phase::Base | Phase::Reference)
            && self
                .orient_surface_session
                .as_mut()
                .unwrap()
                .sphere_input
                .as_mut()
                .unwrap()
                .option(word)
        {
            self.orient_surface_prompt();
            self.command_input.clear();
            return true;
        }
        let s = self.orient_surface_session.as_ref().unwrap();
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
                    Phase::Base | Phase::Reference => {
                        let result = self
                            .orient_surface_session
                            .as_mut()
                            .unwrap()
                            .sphere_input
                            .as_mut()
                            .unwrap()
                            .number(radius);
                        self.accept_splop_reference_result(result);
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

impl VibocerosApp {
    pub(super) fn accept_splop_reference_point(&mut self, point: Point3) -> bool {
        let result = self
            .orient_surface_session
            .as_mut()
            .unwrap()
            .sphere_input
            .as_mut()
            .unwrap()
            .point(point);
        self.accept_splop_reference_result(result)
    }
    fn accept_splop_reference_result(
        &mut self,
        result: Result<
            Option<viboceros_command::circle_input::CircleDefinition>,
            viboceros_geometry::GeometryError,
        >,
    ) -> bool {
        let definition = match result {
            Ok(v) => v,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let s = self.orient_surface_session.as_mut().unwrap();
        if let Some(definition) = definition {
            let Ok(point) = definition
                .frame
                .x_axis()
                .as_vector()
                .scaled(definition.radius)
                .and_then(|v| definition.frame.origin().translated(v))
            else {
                return false;
            };
            s.plane = definition.frame;
            s.base = Some(definition.frame.origin());
            s.reference = Some(point);
            s.sphere_input = None;
            s.phase = Phase::Guide;
        } else {
            let anchor = s.sphere_input.unwrap().anchor();
            s.phase = if anchor.is_some() {
                Phase::Reference
            } else {
                Phase::Base
            };
            self.active_command = Some(Kind::Splop.command(anchor));
        }
        self.orient_surface_prompt();
        true
    }
}
