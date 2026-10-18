//! Viewport-local plane resolution with a captured source frame and source set.
use super::*;
use viboceros_command::remap_cplane::{self as solver, Target};

#[derive(Clone, Copy)]
enum NamePrompt {
    CPlane,
    View,
}

pub(super) struct Session {
    source_plane: Frame3,
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    copy: bool,
    name_prompt: Option<NamePrompt>,
}

impl VibocerosApp {
    pub(super) fn try_start_remap_cplane(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let (name, tail) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        if !name
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("RemapCPlane")
        {
            return false;
        }
        let parsed = solver::arguments(tail).and_then(|args| {
            solver::options(&args, self.commands.copy_default("RemapCPlane").unwrap())
        });
        let options = match parsed {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let sources = picked.map_or_else(
            || {
                self.document
                    .objects()
                    .filter(|o| self.document.is_selected(o.id()))
                    .map(|o| o.id())
                    .collect()
            },
            <[ObjectId]>::to_vec,
        );
        let grips = self
            .document
            .selected_control_points()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if sources.is_empty() && grips.is_empty() {
            self.start_transform_source_prompt("RemapCPlane");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        self.commands.begin_copy_options("RemapCPlane");
        self.remap_cplane_session = Some(Session {
            source_plane: self.viewports[self.active_viewport].construction_plane(),
            sources,
            grips,
            postselected: picked.is_some(),
            copy: options.copy,
            name_prompt: None,
        });
        self.active_command = Some(InteractiveCommand::RemapCPlane);
        self.command_input.clear();
        if let Some(target) = options.target {
            self.remap_cplane_target(target);
        } else {
            self.remap_cplane_prompt();
        }
        true
    }

    fn remap_cplane_prompt(&mut self) {
        let Some(s) = &self.remap_cplane_session else {
            return;
        };
        self.push_log(match s.name_prompt {
            Some(NamePrompt::CPlane) => "RemapCPlane: name of target CPlane (Esc cancels)".into(),
            Some(NamePrompt::View) => "RemapCPlane: name of target view (Esc cancels)".into(),
            None => format!(
                "RemapCPlane: click the target viewport; Copy={}, CPlane, View (Esc cancels)",
                if s.copy { "Yes" } else { "No" }
            ),
        });
    }

    fn resolve_remap_cplane(&self, target: Target) -> Result<Frame3, String> {
        match target {
            Target::Frame(frame) => Ok(frame),
            Target::CPlane(name) => solver::world_frame(&name)
                .or_else(|| self.named_cplanes.get(&name).ok().map(|p| p.plane))
                .ok_or_else(|| format!("CPlane '{name}' does not exist")),
            Target::View(name) => {
                let matches = self
                    .viewports
                    .iter()
                    .filter(|v| v.view_label().eq_ignore_ascii_case(&name))
                    .collect::<Vec<_>>();
                match matches.as_slice() {
                    [view] => Ok(view.construction_plane()),
                    [] => self
                        .named_views
                        .get(&name)
                        .ok()
                        .map(|v| v.construction_plane())
                        .or_else(|| solver::world_frame(&name))
                        .ok_or_else(|| format!("View '{name}' does not exist")),
                    _ => Err(format!("View name '{name}' is ambiguous")),
                }
            }
        }
    }

    fn remap_cplane_target(&mut self, target: Target) -> bool {
        if let Some(session) = self.remap_cplane_session.as_mut() {
            session.name_prompt = match &target {
                Target::CPlane(_) => Some(NamePrompt::CPlane),
                Target::View(_) => Some(NamePrompt::View),
                Target::Frame(_) => None,
            };
        }
        match self.resolve_remap_cplane(target) {
            Ok(frame) => self.finish_remap_cplane(frame),
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn try_continue_remap_cplane(&mut self, input: &str) -> bool {
        let Some(s) = &self.remap_cplane_session else {
            return false;
        };
        if input.starts_with('\'') {
            return false;
        }
        if s.name_prompt.is_none()
            && let Some(first) = input.split_whitespace().next()
            && self.commands.recognizes(first)
            && !["Copy", "CPlane", "View"].iter().any(|name| {
                first
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case(name)
            })
        {
            return false;
        }
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.cancel_interactive_command(true);
            return true;
        }
        if let Some(prompt) = s.name_prompt {
            let name = input.trim().trim_matches('"').to_owned();
            self.remap_cplane_target(match prompt {
                NamePrompt::CPlane => Target::CPlane(name),
                NamePrompt::View => Target::View(name),
            });
        } else if word.eq_ignore_ascii_case("CPlane") || word.eq_ignore_ascii_case("View") {
            self.remap_cplane_session.as_mut().unwrap().name_prompt =
                Some(if word.eq_ignore_ascii_case("CPlane") {
                    NamePrompt::CPlane
                } else {
                    NamePrompt::View
                });
            self.remap_cplane_prompt();
        } else if word.eq_ignore_ascii_case("Copy") {
            self.remap_cplane_session.as_mut().unwrap().copy = !s.copy;
            self.remap_cplane_prompt();
        } else {
            let parsed = solver::arguments(input).and_then(|args| solver::options(&args, s.copy));
            match parsed {
                Ok(options) => {
                    self.remap_cplane_session.as_mut().unwrap().copy = options.copy;
                    if let Some(target) = options.target {
                        self.remap_cplane_target(target);
                    } else {
                        self.remap_cplane_prompt();
                    }
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_remap_cplane_view(&mut self, index: usize) -> bool {
        if self
            .remap_cplane_session
            .as_ref()
            .is_none_or(|s| s.name_prompt.is_some())
        {
            return false;
        }
        let Some(view) = self.viewports.get(index) else {
            return false;
        };
        self.finish_remap_cplane(view.construction_plane())
    }

    fn finish_remap_cplane(&mut self, target: Frame3) -> bool {
        let Some(s) = &self.remap_cplane_session else {
            return false;
        };
        if let Err(error) = solver::transform(s.source_plane, target) {
            self.push_log(format!("Error: {error}"));
            return false;
        }
        let command = format!(
            "RemapCPlane {} Copy={} {}={} {}",
            solver::frame_argument(target),
            if s.copy { "Yes" } else { "No" },
            if s.postselected {
                "PickedSources"
            } else {
                "Sources"
            },
            s.sources
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            transform_sources::grip_argument(&s.grips, s.postselected)
        );
        let result = self.commands.execute_in_context(
            &mut self.document,
            &command,
            viboceros_command::CommandContext {
                construction_plane: s.source_plane,
            },
        );
        match result {
            Ok(message) => {
                self.cancel_interactive_command(false);
                self.push_log(message);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
