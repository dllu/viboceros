//! Frozen source-face selection followed by a path-curve picker.
use super::*;
mod subcurve;
use subcurve::PathPoints;
use viboceros_command::extrude_surface::SurfaceFaces;
use viboceros_command::extrude_surface_path::{
    SurfacePathInput, SurfacePathOptions, SurfacePathSelector, resolve_surface_path,
};

pub(super) struct SurfacePathSession {
    pub(super) sources: extrude_surface::SurfaceExtrusionSources,
    pub(super) options: SurfacePathOptions,
    points: Option<PathPoints>,
}

impl VibocerosApp {
    pub(super) fn try_start_surface_path_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrfAlongCrv")
        }) {
            return false;
        }
        let Ok(parsed) = SurfacePathInput::parse_with_options(
            &words.collect::<Vec<_>>(),
            self.commands.surface_path_settings(),
        ) else {
            return false;
        };
        let sources = match self.prepare_surface_extrusion_sources(parsed.faces) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.surface_path_prompt = Some(SurfacePathSession {
            sources,
            options: parsed.options,
            points: None,
        });
        self.command_input.clear();
        if let Some(selector) = parsed.path {
            self.select_surface_path(Some(&selector), None);
        } else {
            self.log_surface_path_prompt();
        }
        true
    }
    pub(super) fn try_continue_surface_path(&mut self, input: &str) -> bool {
        let Some(session) = self.surface_path_prompt.as_ref() else {
            return false;
        };
        let options = session.options;
        if session.points.is_some() {
            return self.continue_surface_path_points(input);
        }
        let keyword = input.trim_start_matches(['_', '-']);
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() || keyword.eq_ignore_ascii_case("Enter") {
            self.select_surface_path(None, None);
            self.command_input.clear();
            return true;
        }
        if !words.iter().all(|word| {
            word.contains('=')
                || ["Solid", "DeleteInput", "SubCurve", "SplitAtTangents"]
                    .iter()
                    .any(|key| word.trim_start_matches('_').eq_ignore_ascii_case(key))
        }) {
            return false;
        }
        let mut normalized = Vec::new();
        for word in words {
            if let Some((key, value)) = word.split_once('=')
                && key
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("PathLength")
                && let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                    value,
                    self.document.units(),
                )
            {
                match length {
                    Ok(n) => normalized.push(format!("PathLength={n}")),
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        return true;
                    }
                }
            } else {
                normalized.push(word.to_string());
            }
        }
        let refs = normalized.iter().map(String::as_str).collect::<Vec<_>>();
        let result = (|| {
            let parsed = SurfacePathInput::parse_with_options(&refs, options)?;
            if !matches!(parsed.faces, SurfaceFaces::Single) {
                return Err(viboceros_command::CommandError::Usage(
                    "Select source faces before choosing the path",
                ));
            }
            let path = parsed
                .path
                .as_ref()
                .map(|selector| resolve_surface_path(&self.document, Some(selector)))
                .transpose()?;
            Ok((parsed.options, path))
        })();
        match result {
            Ok((options, path)) => {
                self.surface_path_prompt.as_mut().unwrap().options = options;
                if let Some(path) = path {
                    self.accept_surface_path(path, None);
                } else {
                    self.log_surface_path_prompt();
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn pick_surface_extrusion_path(
        &mut self,
        id: Option<ObjectId>,
        pick: Option<(usize, egui::Pos2)>,
    ) -> bool {
        let Some(session) = self.surface_path_prompt.as_ref() else {
            return false;
        };
        if let Some(id) = id.filter(|_| session.points.is_none()) {
            self.select_surface_path(Some(&SurfacePathSelector::Id(id)), pick);
        }
        true
    }

    fn select_surface_path(
        &mut self,
        selector: Option<&SurfacePathSelector>,
        pick: Option<(usize, egui::Pos2)>,
    ) {
        match resolve_surface_path(&self.document, selector) {
            Ok(path) => self.accept_surface_path(path, pick),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }

    fn accept_surface_path(&mut self, path: ObjectId, pick: Option<(usize, egui::Pos2)>) {
        let session = self.surface_path_prompt.as_ref().unwrap();
        if !session.sources.is_current(&self.document) {
            self.push_log("Error: surface source changed; select the faces again".into());
            return;
        }
        let options = session.options;
        let needs_points = options.subcurve && options.path_range.is_none();
        if !needs_points && (pick.is_none() || options.reverse.is_some()) {
            self.finish_surface_path(path, options);
            return;
        }
        let rail = self.document.object(path).unwrap();
        let curve = match rail.geometry().nurbs_curve_representation() {
            Ok(Some(curve)) => curve,
            Ok(None) => return,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return;
            }
        };
        let reverse = options.reverse.unwrap_or_else(|| {
            pick.and_then(|(viewport, pointer)| {
                let view = &self.viewports[viewport];
                view.pick_edge_parameter(&curve, pointer, view.screen_rect()?, false)
            })
            .is_some_and(|t| t > curve.domain().start().midpoint(*curve.domain().end()))
        });
        if needs_points {
            self.begin_surface_path_points(path, curve, reverse);
        } else {
            self.finish_surface_path(
                path,
                SurfacePathOptions {
                    reverse: Some(reverse),
                    ..options
                },
            );
        }
    }

    fn log_surface_path_prompt(&mut self) {
        let session = self.surface_path_prompt.as_ref().unwrap();
        let prompt = session.hint();
        if session.points.is_some() {
            self.push_log(prompt.into());
        } else {
            self.push_log(format!(
                "Select path curve near start ({})",
                session.options.command_options()
            ));
        }
    }

    fn finish_surface_path(&mut self, path: ObjectId, options: SurfacePathOptions) {
        let session = self.surface_path_prompt.take().unwrap();
        if !session.sources.is_current(&self.document) {
            self.surface_path_prompt = Some(session);
            self.push_log("Error: surface source changed; select the faces again".into());
            return;
        }
        let refs = session
            .sources
            .targets
            .iter()
            .map(|(id, index)| format!("{id}:{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let command = format!(
            "ExtrudeSrfAlongCrv Path={path} FaceRefs={refs} {}",
            options.command_options()
        );
        if self.try_execute_command(&command) {
            self.component_selection.clear();
        } else {
            self.surface_path_prompt = Some(session);
        }
    }
}
