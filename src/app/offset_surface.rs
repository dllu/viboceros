//! Offset distance and direction editing; candidates never enter model history.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::offset_surface::{self as solver, Options, Prepared};
use viboceros_document::Object;

mod input;

pub(super) struct Session {
    sources: Vec<Object>,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    units: viboceros_geometry::LengthUnitSystem,
    options: Options,
    flips: BTreeSet<ObjectId>,
    pub(super) postselected: bool,
    distance_menu: bool,
    through_point: bool,
    distance_start: Option<Point3>,
    pub(super) candidate: Option<Prepared>,
    scene: Option<Document>,
    original_count: usize,
    directions: viboceros_command::direction::analysis::View,
}

impl Session {
    #[cfg(test)]
    pub(super) fn getter(&self) -> Option<&'static str> {
        if self.distance_menu {
            Some("Distance")
        } else if self.through_point {
            Some("ThroughPoint")
        } else {
            None
        }
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        // Point getters snap to model geometry, never provisional offset skins.
        if self.wants_point() {
            None
        } else {
            self.scene.as_ref()
        }
    }
    pub(super) fn directions(&self) -> &viboceros_command::direction::analysis::View {
        &self.directions
    }
    pub(super) fn ids(&self) -> BTreeSet<ObjectId> {
        self.sources.iter().map(Object::id).collect()
    }
    pub(super) fn wants_point(&self) -> bool {
        self.distance_menu || self.through_point
    }
    pub(super) fn hint(&self) -> &'static str {
        if self.through_point {
            "Through point; accepts the offset immediately"
        } else if self.distance_start.is_some() {
            "Second point for offset distance"
        } else if self.distance_menu {
            "Positive offset distance or first distance point; Enter accepts a positive default"
        } else {
            "Click a source to flip; Distance / ThroughPoint / Solid / BothSides / FlipAll; Enter accepts, Esc cancels"
        }
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.objects().len() == self.original_count
            && self.scene.as_ref().is_none_or(|s| {
                doc.objects().eq(s.objects().take(self.original_count))
                    && doc.layers().eq(s.layers())
                    && doc.groups().eq(s.groups())
            })
            && self.layer == doc.current_layer_id()
            && &self.units == doc.units()
            && doc
                .selected_objects()
                .map(Object::id)
                .eq(self.sources.iter().map(Object::id))
            && self.sources.iter().all(|old| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
}

impl VibocerosApp {
    pub(super) fn picking_offset_surface_sources(&self) -> bool {
        self.plane_prompt.is_none()
            && self.object_prompt.as_ref().is_some_and(|p| {
                p.description.command == "OffsetSrf"
                    && p.phase == object_selection::ObjectPromptPhase::Selecting
            })
    }
    pub(super) fn start_offset_surface(&mut self, text: &str, postselected: bool) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("OffsetSrf")
        }) {
            return false;
        }
        let input = match solver::parse(
            &words[1..],
            self.commands.offset_surface_options(),
            self.document.units(),
        ) {
            Ok(input) => input,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        if input.distance_supplied
            || input.through_point.is_some()
            || self.document.selected_object_count() == 0
        {
            return false;
        }
        let filter = if cfg!(feature = "native-smlib") {
            viboceros_command::ObjectSelectionFilter::SurfaceComponents
        } else {
            viboceros_command::ObjectSelectionFilter::Surfaces
        };
        if self
            .document
            .selected_objects()
            .any(|o| !filter.accepts_object(o))
        {
            return false;
        }
        self.cancel_interactive_command(false);
        let sources = self
            .document
            .selected_objects()
            .cloned()
            .collect::<Vec<_>>();
        let markers = viboceros_command::direction::collect_markers(
            sources.iter().map(|o| (o.id(), o.geometry())),
            self.document.tolerance(),
        );
        let count = sources.len();
        self.offset_surface_session = Some(Session {
            sources,
            tolerance: self.document.tolerance(),
            layer: self.document.current_layer_id(),
            units: self.document.units().clone(),
            options: input.options,
            flips: BTreeSet::new(),
            postselected,
            distance_menu: false,
            through_point: false,
            distance_start: None,
            candidate: None,
            scene: None,
            original_count: self.document.objects().len(),
            directions: viboceros_command::direction::analysis::View {
                options: viboceros_command::direction::analysis::Options {
                    u: false,
                    v: false,
                    ..Default::default()
                },
                markers: markers.into(),
                sources: count,
            },
        });
        self.commands
            .remember_offset_surface_options(input.options)
            .expect("validated offset options");
        self.active_command = Some(InteractiveCommand::OffsetSurface { anchor: None });
        self.update_offset_surface_preview();
        self.log_offset_surface();
        true
    }
    fn log_offset_surface(&mut self) {
        if let Some(s) = &self.offset_surface_session {
            if s.distance_menu {
                let default = if s.options.distance == 0. {
                    1.
                } else {
                    s.options.distance
                };
                self.push_log(format!("OffsetSrf: {} <{}>", s.hint(), default));
                return;
            }
            self.push_log(format!(
                "OffsetSrf: {} ({})",
                s.hint(),
                s.options.prompt_arguments()
            ));
        }
    }
    pub(super) fn validate_offset_surface(&mut self) {
        if self
            .offset_surface_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log(
                "OffsetSrf sources or document settings changed; select the surfaces again".into(),
            );
        }
    }
    fn update_offset_surface_preview(&mut self) {
        let Some(s) = &mut self.offset_surface_session else {
            return;
        };
        s.candidate = None;
        s.scene = None;
        let mut markers = viboceros_command::direction::collect_markers(
            s.sources.iter().map(|o| (o.id(), o.geometry())),
            s.tolerance,
        );
        for marker in &mut markers {
            if s.options.flip_all ^ s.flips.contains(&marker.object) {
                marker.axes[2] = marker.axes[2].and_then(|v| v.scaled(-1.).ok());
            }
        }
        s.directions.markers = markers.into();
        let result = solver::prepare(&self.document, s.options, &s.flips).and_then(|p| {
            let scene = p.preview(&self.document)?;
            Ok((p, scene))
        });
        match result {
            Ok((p, scene)) => {
                s.candidate = Some(p);
                s.scene = Some(scene);
            }
            Err(e) => self.push_log(format!("OffsetSrf preview: {e}")),
        }
    }
    fn commit_offset_surface(&mut self) {
        self.validate_offset_surface();
        let Some(s) = &self.offset_surface_session else {
            return;
        };
        let Some(p) = &s.candidate else {
            self.push_log("No valid offset; change the distance or cancel".into());
            return;
        };
        match p.commit(&mut self.document, s.postselected) {
            Ok(message) => {
                self.offset_surface_session = None;
                self.cancel_interactive_command(false);
                self.push_log(message);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn flip_offset_surface_input(&mut self, id: Option<ObjectId>) -> bool {
        if self.offset_surface_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.validate_offset_surface();
        let Some(s) = &mut self.offset_surface_session else {
            return true;
        };
        if s.wants_point() {
            return false;
        }
        if let Some(id) = id.filter(|id| s.sources.iter().any(|o| o.id() == *id)) {
            if !s.flips.remove(&id) {
                s.flips.insert(id);
            }
            self.update_offset_surface_preview();
        }
        true
    }
}
