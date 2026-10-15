//! Frozen source-face selection followed by a path-curve picker.
use super::*;
use viboceros_command::extrude_surface_path::{
    SurfacePathInput, SurfacePathOptions, SurfacePathSelector, resolve_surface_path,
};
pub(super) struct SurfacePathSession {
    pub(super) sources: extrude_surface::SurfaceExtrusionSources,
    pub(super) options: SurfacePathOptions,
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
        let Ok(parsed) = SurfacePathInput::parse(&words.collect::<Vec<_>>()) else {
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
        });
        self.command_input.clear();
        if let Some(selector) = parsed.path {
            match resolve_surface_path(&self.document, Some(&selector)) {
                Ok(id) => {
                    self.finish_surface_path(id);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.push_log(format!(
                "ExtrudeSrfAlongCrv: select path curve ({})",
                parsed.options.command_options()
            ));
        }
        true
    }
    pub(super) fn try_continue_surface_path(&mut self, input: &str) -> bool {
        let Some(session) = self.surface_path_prompt.as_ref() else {
            return false;
        };
        let mut options = session.options;
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            match resolve_surface_path(&self.document, None) {
                Ok(id) => {
                    self.finish_surface_path(id);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        if !words.iter().all(|w| w.contains('=')) {
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
            let parsed = SurfacePathInput::parse(&refs)?;
            if !matches!(
                parsed.faces,
                viboceros_command::extrude_surface::SurfaceFaces::Single
            ) {
                return Err(viboceros_command::CommandError::Usage(
                    "Select source faces before choosing the path",
                ));
            }
            let option_words = refs
                .iter()
                .copied()
                .filter(|w| {
                    !w.trim_start_matches('_')
                        .split_once('=')
                        .is_some_and(|(k, _)| {
                            k.eq_ignore_ascii_case("Path") || k.eq_ignore_ascii_case("PathName")
                        })
                })
                .collect::<Vec<_>>();
            options.update(&option_words)?;
            let path = parsed
                .path
                .as_ref()
                .map(|s| resolve_surface_path(&self.document, Some(s)))
                .transpose()?;
            Ok::<_, viboceros_command::CommandError>(path)
        })();
        match result {
            Ok(path) => {
                self.surface_path_prompt.as_mut().unwrap().options = options;
                if let Some(id) = path {
                    self.finish_surface_path(id);
                } else {
                    self.push_log(options.command_options());
                }
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn pick_surface_extrusion_path(&mut self, id: Option<ObjectId>) -> bool {
        if self.surface_path_prompt.is_none() {
            return false;
        }
        if let Some(id) = id {
            match resolve_surface_path(&self.document, Some(&SurfacePathSelector::Id(id))) {
                Ok(id) => {
                    self.finish_surface_path(id);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        true
    }
    fn finish_surface_path(&mut self, path: ObjectId) -> bool {
        let Some(session) = self.surface_path_prompt.take() else {
            return false;
        };
        if session.sources.snapshots.iter().any(|(id, s)| {
            self.document
                .object(*id)
                .is_none_or(|o| o.geometry_snapshot() != s)
        }) {
            self.surface_path_prompt = Some(session);
            self.push_log("Error: surface source changed; select the faces again".into());
            return false;
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
            session.options.command_options()
        );
        if self.try_execute_command(&command) {
            self.component_selection.clear();
            true
        } else {
            self.surface_path_prompt = Some(session);
            false
        }
    }
}
