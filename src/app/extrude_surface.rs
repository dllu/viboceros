//! Frozen face sources and shared drafting input for surface extrusion.
use super::*;
use viboceros_command::extrude_surface::{
    SurfaceExtrusionInput, SurfaceExtrusionOptions, SurfaceFaces, surface_extrusion_targets,
};

pub(super) struct SurfaceExtrusionSources {
    pub(super) targets: Vec<(ObjectId, usize)>,
    pub(super) snapshots: Vec<(ObjectId, viboceros_document::GeometrySnapshot)>,
}
impl VibocerosApp {
    pub(super) fn prepare_surface_extrusion_sources(
        &self,
        mut faces: SurfaceFaces,
    ) -> Result<Option<SurfaceExtrusionSources>, String> {
        if matches!(faces, SurfaceFaces::Single) {
            let picks = self
                .component_selection
                .checked_picks(&self.document)
                .map_err(str::to_string)?;
            if !picks.is_empty() {
                if picks
                    .iter()
                    .any(|p| p.kind != viboceros_command::ComponentSelectionKind::BrepFace)
                {
                    return Err("surface extrusion requires surface faces".into());
                }
                faces =
                    SurfaceFaces::References(picks.iter().map(|p| (p.object, p.index)).collect());
            }
        }
        if !matches!(faces, SurfaceFaces::References(_))
            && !self.document.selected_objects().any(|o| {
                viboceros_command::ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
            })
        {
            return Ok(None);
        }
        let targets =
            surface_extrusion_targets(&self.document, &faces).map_err(|e| e.to_string())?;
        let objects = targets
            .iter()
            .map(|(id, _)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        let snapshots = objects
            .into_iter()
            .map(|id| {
                (
                    id,
                    self.document
                        .object(id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone(),
                )
            })
            .collect();
        Ok(Some(SurfaceExtrusionSources { targets, snapshots }))
    }

    pub(super) fn try_start_surface_extrusion_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrf")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceExtrusionInput::parse(&words.collect::<Vec<_>>()) else {
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
        self.surface_extrusion_sources = Some(sources);
        self.active_command = Some(InteractiveCommand::ExtrudeSurface {
            base: None,
            options: parsed.options,
        });
        self.command_input.clear();
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
    pub(super) fn finish_surface_extrusion(
        &mut self,
        distance: &str,
        options: SurfaceExtrusionOptions,
    ) -> bool {
        let Some(sources) = self.surface_extrusion_sources.take() else {
            return false;
        };
        if sources.snapshots.iter().any(|(id, s)| {
            self.document
                .object(*id)
                .is_none_or(|o| o.geometry_snapshot() != s)
        }) {
            self.surface_extrusion_sources = Some(sources);
            self.push_log("Error: surface source changed; select the faces again".into());
            return false;
        }
        let refs = sources
            .targets
            .iter()
            .map(|(id, index)| format!("{id}:{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let command = format!(
            "ExtrudeSrf {distance} {} FaceRefs={refs}",
            options.command_options()
        );
        let saved = self.active_command;
        let plane = self.drafting_plane;
        let constraint = self.point_constraint.take();
        let filter = self.point_filter.take();
        if self.try_execute_command(&command) {
            self.active_command = None;
            self.drafting_plane = None;
            self.component_selection.clear();
            true
        } else {
            self.active_command = saved;
            self.drafting_plane = plane;
            self.point_constraint = constraint;
            self.point_filter = filter;
            self.surface_extrusion_sources = Some(sources);
            false
        }
    }
    pub(super) fn try_continue_surface_extrusion(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeSurface {
            mut base,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return false;
        }
        if words.iter().all(|s| s.contains('=')) {
            match options.update(&words) {
                Ok(()) => {
                    self.active_command =
                        Some(InteractiveCommand::ExtrudeSurface { base, options });
                    self.push_log(options.command_options());
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Direction")
        {
            base = None;
            options.direction = None;
            self.active_command = Some(InteractiveCommand::ExtrudeSurface { base, options });
            self.command_input.clear();
            self.push_log("ExtrudeSrf: pick two direction points".into());
            return true;
        }
        if base.is_none()
            && let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match length {
                Ok(h) => {
                    self.finish_surface_extrusion(&h.to_string(), options);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
}
