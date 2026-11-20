//! Divide's seam and numeric getters with an immutable station preview.
use super::*;
use crate::viewport::curve_seam::Event;
use std::collections::BTreeMap;
use std::sync::Arc;
use viboceros_command::CommandError;
use viboceros_command::divide::{self, Source, Specification, Traversal, prompt::Prompt};
use viboceros_document::GeometrySnapshot;

mod portions;
pub(super) use portions::Inputs;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    prompt: Prompt,
    sources: Vec<Source>,
    snapshots: BTreeMap<ObjectId, GeometrySnapshot>,
    selection: Vec<ObjectId>,
    tolerance: Tolerance,
    seam: Option<curve_seam::Editor>,
    seam_id_pending: bool,
    points: Option<Arc<[Point3]>>,
}

impl Session {
    fn new(doc: &Document, prompt: Prompt, inputs: &Inputs) -> Result<Self, CommandError> {
        let sources = inputs.sources(doc)?;
        let mut snapshots = inputs
            .snapshots()
            .map(|(id, snapshot)| (id, snapshot.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut closed = Vec::new();
        for object in doc.selected_objects() {
            let curve = object
                .geometry()
                .curve_ref()
                .ok_or(CommandError::UnsupportedDivideGeometry)?;
            if curve.is_closed()? {
                closed.push(object.id());
            }
            snapshots.insert(object.id(), object.geometry_snapshot().clone());
        }
        let seam = (!closed.is_empty())
            .then(|| curve_seam::Editor::capture(doc, closed, false, true))
            .transpose()?;
        let mut session = Self {
            prompt,
            sources,
            snapshots,
            selection: doc.selected_object_ids().collect(),
            tolerance: doc.tolerance(),
            seam,
            seam_id_pending: false,
            points: None,
        };
        if session.seam.is_none() {
            session.refresh_points(doc)?;
        }
        Ok(session)
    }

    pub(super) fn label(&self) -> &'static str {
        if self.seam.is_some() {
            "Curve seams"
        } else {
            self.prompt.label()
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.seam_id_pending {
            "Enter the closed curve object ID"
        } else if let Some(editor) = &self.seam {
            if editor.flipping() {
                "Select seam to flip"
            } else {
                "Drag seam markers. Flip / Automatic / Natural. Enter continues, Esc cancels"
            }
        } else if self.picking_direction() {
            "Enter a distance or pick a curve to flip its direction. NumberSegments returns to count. Enter accepts, Esc cancels"
        } else if !matches!(self.prompt.options.specification, Specification::Count(_)) {
            "Enter a distance. NumberSegments returns to count. Enter accepts, Esc cancels"
        } else {
            "Enter a count or choose Length, EqualChordLength or output options. Enter creates the previewed result, Esc cancels"
        }
    }

    fn picking_direction(&self) -> bool {
        self.seam.is_none()
            && matches!(self.prompt.options.specification, Specification::Length(_))
            && self.sources.iter().any(|source| source.portion.is_none())
    }

    pub(in crate::app) fn seam_editor(&self, doc: &Document) -> Option<&curve_seam::Editor> {
        self.seam.as_ref().filter(|_| self.matches_document(doc))
    }

    fn refresh_points(&mut self, doc: &Document) -> Result<(), CommandError> {
        self.points =
            Some(divide::preview_source_points(doc, self.prompt.options, &self.sources)?.into());
        Ok(())
    }

    fn accept_seams(&mut self, doc: &Document) -> Result<(), CommandError> {
        let editor = self.seam.take().unwrap();
        let mut placements = BTreeMap::new();
        for (source, (parameter, flip_seam)) in editor.sources().zip(editor.placements()) {
            let seam = (parameter != *source.curve_ref().domain().start()).then_some(parameter);
            placements.insert(
                source.object_id(),
                Traversal {
                    seam,
                    flip_seam,
                    ..Traversal::default()
                },
            );
        }
        for source in &mut self.sources {
            if source.portion.is_none()
                && let Some(traversal) = placements.get(&source.object)
            {
                source.traversal = *traversal;
            }
        }
        self.seam_id_pending = false;
        self.refresh_points(doc)
    }

    fn matches_document(&self, doc: &Document) -> bool {
        if self.tolerance != doc.tolerance()
            || !doc.selected_object_ids().eq(self.selection.iter().copied())
        {
            return false;
        }
        if self.snapshots.len() > 16 {
            doc.selectable_objects()
                .filter(|object| {
                    self.snapshots.get(&object.id()).is_some_and(|snapshot| {
                        snapshot.shares_storage_with(object.geometry_snapshot())
                    })
                })
                .count()
                == self.snapshots.len()
        } else {
            self.snapshots.iter().all(|(id, snapshot)| {
                doc.is_object_selectable(*id)
                    && doc
                        .object(*id)
                        .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
            })
        }
    }
}

impl VibocerosApp {
    fn divide_session(&self) -> Option<&Session> {
        self.object_prompt.as_ref()?.divide.as_ref()
    }

    pub(in crate::app) fn apply_divide_seam_event(
        &mut self,
        viewport: usize,
        event: Event,
    ) -> bool {
        let Some(editor) = self
            .object_prompt
            .as_mut()
            .and_then(|pending| pending.divide.as_mut())
            .filter(|session| session.matches_document(&self.document))
            .and_then(|session| session.seam.as_mut())
        else {
            return false;
        };
        let changed = editor.apply(viewport, event);
        if matches!(event, Event::Flip(_)) {
            self.log_object_prompt();
        }
        changed
    }

    pub(in crate::app) fn divide_direction_sources(&self) -> Option<BTreeSet<ObjectId>> {
        let session = self
            .divide_session()
            .filter(|_| self.picking_divide_direction())?;
        Some(
            session
                .sources
                .iter()
                .filter(|s| s.portion.is_none())
                .map(|s| s.object)
                .collect(),
        )
    }

    pub(in crate::app) fn picking_divide_direction(&self) -> bool {
        self.divide_session().is_some_and(|session| {
            session.picking_direction() && session.matches_document(&self.document)
        })
    }

    pub(in crate::app) fn divide_options_active(&self) -> bool {
        self.divide_session().is_some()
    }

    pub(in crate::app) fn pick_divide_direction(&mut self, id: Option<ObjectId>) -> bool {
        if self.plane_prompt.is_some() || self.nested_measurement.is_some() {
            return false;
        }
        let Some(session) = self.divide_session() else {
            return false;
        };
        if session.picking_direction()
            && let Some(index) = session
                .sources
                .iter()
                .position(|source| source.portion.is_none() && Some(source.object) == id)
        {
            if !self.check_divide_sources() {
                return true;
            }
            let mut session = self.divide_session().unwrap().clone();
            session.sources[index].traversal.reverse_measurement ^= true;
            match session.refresh_points(&self.document) {
                Ok(()) => self.object_prompt.as_mut().unwrap().divide = Some(session),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        true
    }

    pub(in crate::app) fn initialize_divide_prompt(&mut self) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "Divide" && p.phase == ObjectPromptPhase::Options)
        else {
            return false;
        };
        let postselected = pending.postselected;
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let curves = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if before != curves {
            if !postselected {
                self.object_prompt.as_mut().unwrap().selection_before = Some(before);
            }
            if let Err(error) = self
                .document
                .select_objects_direct(curves, SelectionMode::Replace)
            {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        }
        match Session::new(
            &self.document,
            self.commands.divide_prompt_default(),
            &self.object_prompt.as_ref().unwrap().divide_inputs,
        ) {
            Ok(session) => {
                let pending = self.object_prompt.as_mut().unwrap();
                pending.command_override = Some(session.prompt.command_line());
                pending.divide = Some(session);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(in crate::app) fn continue_divide_prompt(&mut self, input: &str) -> bool {
        let Some(session) = self.divide_session() else {
            return false;
        };
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        let name = normalized
            .split(|c: char| c == '=' || c.is_whitespace())
            .next()
            .unwrap_or("");
        let local_options: &[&str] = if session.seam.is_some() {
            &[
                "SelID",
                "Parameter",
                "Flip",
                "Natural",
                "Automatic",
                "Enter",
            ]
        } else {
            &[
                "Length",
                "EqualChordLength",
                "NumberSegments",
                "Split",
                "DeleteRemainder",
                "MarkEnds",
                "GroupOutput",
                "Preview",
                "Enter",
            ]
        };
        if !local_options
            .iter()
            .any(|option| option.eq_ignore_ascii_case(name))
            && self.commands.recognizes(name)
        {
            return false;
        }
        if !self.check_divide_sources() {
            return true;
        }
        let mut session = self.divide_session().unwrap().clone();
        let accept = input.is_empty() || normalized.eq_ignore_ascii_case("Enter");
        if let Some(editor) = &mut session.seam {
            if accept {
                if !editor.finish_profile_flip()
                    && let Err(error) = session.accept_seams(&self.document)
                {
                    self.push_log(format!("Error: {error}"));
                }
            } else if name.eq_ignore_ascii_case("SnapToKnots") {
                self.push_log(session.hint().into());
                return true;
            } else {
                let point = self.profile_seam_point(input);
                if let Err(error) = editor.edit_profile(
                    normalized,
                    &mut session.seam_id_pending,
                    self.active_viewport,
                    point,
                    session.tolerance,
                ) {
                    self.push_log(error.into());
                }
            }
        } else if accept {
            self.push_log(format!("> {}", session.prompt.command_line()));
            match self.commands.finish_divide_prompt(
                &mut self.document,
                session.prompt,
                &session.sources,
            ) {
                Ok(message) => {
                    self.object_prompt = None;
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        } else if session.picking_direction()
            && input.contains([',', '<'])
            && let Some(point) = self.profile_seam_point(input)
        {
            let picked = session
                .sources
                .iter()
                .filter(|source| source.portion.is_none())
                .filter_map(|source| {
                    let curve = session.snapshots.get(&source.object)?.curve_ref()?;
                    let parameter = curve.closest_parameter(point, session.tolerance).ok()?;
                    let distance = curve.evaluate(parameter).ok()?.distance_to(point).ok()?;
                    (distance <= session.tolerance.absolute()).then_some((source.object, distance))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((id, _)) = picked {
                self.pick_divide_direction(Some(id));
                self.command_input.clear();
            } else {
                self.push_log("Pick a selected curve to flip its direction".into());
            }
            return true;
        } else {
            let update = (|| {
                if !normalized.eq_ignore_ascii_case("Preview") {
                    session.prompt = session.prompt.updated(input)?;
                }
                session.refresh_points(&self.document)?;
                self.commands.accept_divide_prompt(session.prompt)
            })();
            if let Err(error) = update {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        }
        let pending = self.object_prompt.as_mut().unwrap();
        pending.command_override = Some(session.prompt.command_line());
        pending.divide = Some(session);
        self.log_object_prompt();
        self.command_input.clear();
        true
    }

    fn check_divide_sources(&mut self) -> bool {
        if self
            .divide_session()
            .is_some_and(|s| s.matches_document(&self.document))
        {
            return true;
        }
        self.push_log("Divide sources changed. Start the command again".into());
        self.cancel_object_prompt(false);
        false
    }

    pub(in crate::app) fn divide_preview_points(&self) -> Option<Arc<[Point3]>> {
        let session = self.divide_session()?;
        session.matches_document(&self.document).then_some(())?;
        session.points.clone()
    }
}
