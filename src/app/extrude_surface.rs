//! Frozen face sources and shared drafting input for surface extrusion.
use super::*;
mod linear;
use crate::viewport::surface_extrusion::{
    SurfaceExtrusionDraft, SurfaceExtrusionPreview, signed_height,
};
use viboceros_command::extrude_surface::{
    SurfaceExtrusionInput, SurfaceExtrusionOptions, SurfaceFaces, surface_extrusion_targets,
};

pub(super) struct SurfaceExtrusionSources {
    pub(super) targets: Vec<(ObjectId, usize)>,
    pub(super) apex: Option<extrude_surface_apex::Session>,
    pub(super) snapshots: Vec<(ObjectId, viboceros_document::GeometrySnapshot)>,
    linear: Option<LinearSession>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Stage {
    Distance,
    BasePoint,
    DirectionStart,
    DirectionEnd(Point3),
}

struct LinearSession {
    draft: Option<SurfaceExtrusionDraft>,
    plane: Frame3,
    base: Option<Point3>,
    hover: Option<f64>,
}

impl SurfaceExtrusionSources {
    pub(super) fn is_current(&self, document: &Document) -> bool {
        self.snapshots.iter().all(|(id, snapshot)| {
            document
                .object(*id)
                .is_some_and(|object| object.geometry_snapshot() == snapshot)
        })
    }

    pub(super) fn linear_preview(
        &self,
        document: &Document,
        command: Option<InteractiveCommand>,
    ) -> Option<SurfaceExtrusionPreview<'_>> {
        let Some(InteractiveCommand::ExtrudeSurface {
            stage: Stage::Distance,
            options,
        }) = command
        else {
            return None;
        };
        if !self.is_current(document) {
            return None;
        }
        let session = self.linear.as_ref()?;
        let draft = session.draft.as_ref()?;
        Some(SurfaceExtrusionPreview {
            draft,
            options,
            plane: session.plane,
            constraint: draft.constraint(session.base, options, session.plane),
            hover: session.hover,
        })
    }
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
        Ok(Some(SurfaceExtrusionSources {
            targets,
            snapshots,
            apex: None,
            linear: None,
        }))
    }

    pub(super) fn finish_surface_extrusion(
        &mut self,
        distance: f64,
        options: SurfaceExtrusionOptions,
    ) -> bool {
        self.finish_surface_face_command(&format!(
            "ExtrudeSrf {distance} {}",
            options.command_options()
        ))
    }
    pub(super) fn finish_surface_face_command(&mut self, command: &str) -> bool {
        let Some(sources) = self.surface_extrusion_sources.take() else {
            return false;
        };
        if !sources.is_current(&self.document) {
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
        let command = format!("{command} FaceRefs={refs}");
        let saved = self.active_command;
        let plane = self.drafting_plane;
        let command_plane = sources.linear.as_ref().map(|session| session.plane);
        let constraint = self.point_constraint.take();
        let filter = self.point_filter.take();
        if self.try_execute_command_with_plane(&command, command_plane) {
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
}
