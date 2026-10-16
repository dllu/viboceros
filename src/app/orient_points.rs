//! Point-based orientation retains source identities across its reference getters.
use super::*;
use viboceros_command::orient_points::{self as solver, Kind, Options};
use viboceros_geometry::AffineTransform3;

pub(super) struct Session {
    group: viboceros_document::HistoryGroup,
    kind: Kind,
    options: Options,
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    references: Vec<Point3>,
    targets: Vec<Point3>,
    preview: Option<AffineTransform3>,
    applied: bool,
}
impl Session {
    pub(super) fn markers(&self) -> Vec<Point3> {
        self.references
            .iter()
            .chain(&self.targets)
            .copied()
            .collect()
    }
    pub(super) fn preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        if self.references.len() != self.kind.count() {
            return None;
        }
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            grips: &self.grips,
            definition: crate::viewport::affine_preview::Mapping::Orient(solver::Preview {
                references: [self.references[0], self.references[1]],
                third: self.references.get(2).copied(),
                targets: [self.targets.first().copied(), self.targets.get(1).copied()],
                scale: self.options.scale,
            }),
            frame: None,
            copy: self.options.copy,
            last_transform: self.preview,
            rigid_layout: None,
        })
    }
    pub(super) fn update_preview(&mut self, preview: Option<AffineTransform3>) -> bool {
        if self.preview == preview {
            false
        } else {
            self.preview = preview;
            true
        }
    }
}
impl VibocerosApp {
    pub(super) fn try_start_orient_points_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut words = input.split_whitespace();
        let Some(kind) = words.next().and_then(Kind::from_name) else {
            return false;
        };
        let args = words.collect::<Vec<_>>();
        let defaults = Options {
            scale: self.commands.orient_point_scale(kind),
            ..Default::default()
        };
        let Ok((rest, mut options)) = solver::options(kind, &args, defaults) else {
            return false;
        };
        if !rest.is_empty() {
            return false;
        }
        self.cancel_interactive_command(false);
        let remembered = self.commands.begin_copy_options(kind.name()).unwrap();
        if !args.iter().any(|a| {
            a.split('=').next().is_some_and(|k| {
                k.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Copy")
            })
        }) {
            options.copy = remembered;
        }
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect::<Vec<_>>(),
            |p| p.to_vec(),
        );
        let grips = self
            .document
            .selected_control_points()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if sources.is_empty() && grips.is_empty() {
            self.start_transform_source_prompt(kind.name());
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        let group = match self.document.begin_history_group(kind.name()) {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.commands
            .remember_orient_point_scale(kind, options.scale);
        self.orient_points_session = Some(Session {
            group,
            kind,
            options,
            sources,
            grips,
            postselected: picked.is_some(),
            references: Vec::new(),
            targets: Vec::new(),
            preview: None,
            applied: false,
        });
        self.active_command = Some(InteractiveCommand::OrientPoints { kind, last: None });
        self.orient_points_prompt();
        true
    }
    fn orient_points_prompt(&mut self) {
        let s = self.orient_points_session.as_ref().unwrap();
        let (phase, index) = if s.references.len() < s.kind.count() {
            ("reference", s.references.len())
        } else {
            ("target", s.targets.len())
        };
        self.push_log(format!(
            "{}: pick {} {phase} point; {}; Esc cancels",
            s.kind.name(),
            ["first", "second", "third"][index],
            s.options.arguments(s.kind)
        ));
    }
    pub(super) fn try_continue_orient_points(&mut self, input: &str) -> bool {
        let Some(s) = self.orient_points_session.as_ref() else {
            return false;
        };
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if s.kind == Kind::Two && s.references.len() == 2 && s.targets.is_empty() {
                let point = s.references[0];
                self.accept_drafting_point(point);
            } else {
                self.cancel_interactive_command(true);
            }
            return true;
        }
        let args = input.split_whitespace().collect::<Vec<_>>();
        if args.first().is_some_and(|a| {
            matches!(
                a.trim_start_matches(['_', '-'])
                    .split('=')
                    .next()
                    .unwrap()
                    .to_ascii_lowercase()
                    .as_str(),
                "copy" | "scale"
            )
        }) {
            match solver::options(s.kind, &args, s.options) {
                Ok((rest, options)) if rest.is_empty() => {
                    let s = self.orient_points_session.as_mut().unwrap();
                    s.options = options;
                    self.commands
                        .remember_orient_point_scale(s.kind, options.scale);
                    s.preview = None;
                    self.orient_points_prompt();
                }
                _ => self.push_log("Error: enter a valid Copy or Scale option".into()),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
    pub(super) fn accept_orient_points_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.orient_points_session.as_mut()?;
        if s.references.len() < s.kind.count() {
            s.references.push(point);
            if let Err(error) =
                solver::validate_references(&s.references, self.document.tolerance())
            {
                s.references.pop();
                self.push_log(format!("Error: {error}"));
                return Some(false);
            }
        } else {
            s.targets.push(point);
            if let Err(error) = solver::validate_references(&s.targets, self.document.tolerance()) {
                s.targets.pop();
                self.push_log(format!("Error: {error}"));
                return Some(false);
            }
            if s.targets.len() == s.kind.count() {
                let points = s
                    .references
                    .iter()
                    .chain(&s.targets)
                    .map(|p| format_model_point(*p))
                    .collect::<Vec<_>>()
                    .join(" ");
                let sources = if s.sources.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{}={}",
                        if s.postselected {
                            "PickedSources"
                        } else {
                            "Sources"
                        },
                        s.sources
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                let input = format!(
                    "{} {points} {} {sources} {}",
                    s.kind.name(),
                    s.options.arguments(s.kind),
                    super::transform_sources::grip_argument(&s.grips, s.postselected)
                );
                match self.commands.execute_in_history_group(
                    &mut self.document,
                    &input,
                    viboceros_command::CommandContext::default(),
                    &mut s.group,
                ) {
                    Ok(message) => {
                        s.applied = true;
                        let copy = s.options.copy;
                        s.targets.clear();
                        s.preview = None;
                        self.active_command = Some(InteractiveCommand::OrientPoints {
                            kind: s.kind,
                            last: Some(point),
                        });
                        self.push_log(message);
                        if copy {
                            self.orient_points_prompt();
                        } else {
                            self.cancel_interactive_command(false);
                        }
                        return Some(true);
                    }
                    Err(error) => {
                        s.targets.pop();
                        self.push_log(format!("Error: {error}"));
                        return Some(false);
                    }
                }
            }
        }
        self.active_command = Some(InteractiveCommand::OrientPoints {
            kind: s.kind,
            last: Some(point),
        });
        s.preview = None;
        self.orient_points_prompt();
        Some(true)
    }
    pub(super) fn finish_orient_points_session(&mut self) -> bool {
        let Some(s) = self.orient_points_session.take() else {
            return false;
        };
        if s.applied && s.postselected {
            self.document.clear_selection();
        }
        s.applied
    }
}
