//! Sweep1 curve selection and point ends share the ordinary drafting pipeline.
use super::curve_seam::Editor;
use super::object_selection::PendingObjectCommand;
use super::*;
use std::collections::BTreeSet;
use viboceros_command::{
    CommandError,
    chain_selection::Target,
    sweep::{Options, Rail, RailReference, Source},
};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::UnitVector3;

mod alignment;
mod axis;
mod input;
mod preferences;
mod preview;
mod rail_picks;
mod seams;
mod selection;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Start,
    Finish,
}
impl End {
    fn set_point(self, options: &mut Options, point: Point3) {
        match self {
            Self::Start => options.ends.start = Some(point),
            Self::Finish => options.ends.end = Some(point),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pick {
    Pole(End),
    AxisStart,
    AxisEnd(Point3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DraftChange {
    Pole(End, Point3),
    Axis(Option<UnitVector3>),
}

impl DraftChange {
    fn apply(self, options: &mut Options) {
        match self {
            Self::Pole(end, point) => end.set_point(options, point),
            Self::Axis(axis) => {
                options.roadlike = Some(true);
                options.surface_aligned = false;
                options.axis = axis;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct State {
    options: Options,
    inherited_surface_style: bool,
    sources: Vec<(ObjectId, GeometrySnapshot)>,
    rail: Option<Rail>,
    reference: Option<RailReference>,
    stage: ProfileStage,
    reversed_profiles: BTreeSet<ObjectId>,
}

#[derive(Clone, Debug)]
enum ProfileStage {
    Selecting,
    PointEndPicked,
    Review,
    Seams { editor: Editor, editing: bool },
}

impl ProfileStage {
    fn seams(&self) -> Option<&Editor> {
        match self {
            Self::Seams { editor, .. } => Some(editor),
            _ => None,
        }
    }

    fn reviewing(&self) -> bool {
        matches!(self, Self::Review | Self::Seams { editing: false, .. })
    }
}

impl State {
    fn capture_source(&self, document: &Document) -> Result<Source, CommandError> {
        let ids = self.sources.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if let Some(rail) = &self.rail {
            Source::with_rail(document, rail.clone(), &ids, &self.options)
        } else {
            Source::capture(document, &ids, &self.options)
        }
    }
}

#[derive(Debug)]
pub(super) struct Session {
    state: State,
    undo: Vec<State>,
    picking: Option<Pick>,
    value: Option<&'static str>,
    seam_id_pending: bool,
    alignment: Option<alignment::Alignment>,
    preview: preview::Preview,
    review_open_profiles: bool,
    pub(super) chain_auto: bool,
}
impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.picking.is_none()
            && self.value.is_none()
            && matches!(self.state.stage, ProfileStage::Selecting)
    }
    pub(super) fn hint(&self) -> &str {
        if self.state.stage.reviewing()
            && self.picking.is_none()
            && self.value.is_none()
            && self.alignment.is_none()
            && let Some(error) = self.preview.error()
        {
            return error;
        }
        match (self.picking, self.value, &self.state.stage) {
            (Some(Pick::Pole(End::Start)), _, _) => "Pick the Sweep1 start point; Esc cancels",
            (Some(Pick::Pole(End::Finish)), _, _) => "Pick the Sweep1 end point; Esc cancels",
            (Some(Pick::AxisStart), _, _) => {
                "Pick the Roadlike axis base point. Automatic uses the rail. Enter uses CPlane Z"
            }
            (Some(Pick::AxisEnd(_)), _, _) => {
                "Pick the second Roadlike axis point. Automatic uses the rail. Enter uses CPlane Z"
            }
            (_, Some(_), _) => "Enter the Sweep1 option value; Undo returns, Esc cancels",
            _ if self.alignment.is_some() && self.seam_id_pending => {
                "Enter a cross-section object ID"
            }
            _ if self.alignment.is_some() => {
                "Click a cross-section end to reverse it or use SelID. Press Enter to return to the preview"
            }
            (
                _,
                _,
                ProfileStage::Seams {
                    editor,
                    editing: true,
                },
            ) => {
                if self.seam_id_pending {
                    "Enter a closed profile object ID"
                } else {
                    editor.hint()
                }
            }
            (_, _, ProfileStage::PointEndPicked) => {
                "Enter accepts the profiles and point ends. StartPoint / EndPoint / Undo"
            }
            (_, _, ProfileStage::Review | ProfileStage::Seams { editing: false, .. })
                if self.can_close() =>
            {
                "Review the sweep. Closed / Style / SetAxis / ShapeBlending / UntrimmedMiters / RefitRail / Simplify / AlignCrossSections / Undo. Enter creates the sweep"
            }
            (_, _, ProfileStage::Review | ProfileStage::Seams { editing: false, .. }) => {
                "Review the sweep. Style / SetAxis / ShapeBlending / UntrimmedMiters / RefitRail / Simplify / AlignCrossSections / Undo. Enter creates the sweep"
            }
            _ if self.state.rail.is_none()
                && !self.state.options.has_explicit_rail()
                && self.state.options.parameters.is_some()
                && !self.awaiting_rail() =>
            {
                "Select profiles in parameter order, then the rail. RailEdge / Undo. Press Enter to continue"
            }
            _ if self.awaiting_rail() => {
                "Select a rail curve or surface edge. ChainEdges / RailId / RailEdge"
            }
            _ => {
                "Select profiles in order, then press Enter. Point / StartPoint / EndPoint / RefitRail / Style / SetAxis / Undo"
            }
        }
    }
    fn awaiting_rail(&self) -> bool {
        self.selecting()
            && self.state.rail.is_none()
            && !self.state.options.has_explicit_rail()
            && self
                .state
                .options
                .parameters
                .as_ref()
                .map_or(self.state.sources.is_empty(), |parameters| {
                    self.state.sources.len() == parameters.len()
                })
    }

    fn can_close(&self) -> bool {
        self.profiles() >= 2
            && self.state.options.ends.is_empty()
            && if let Some(rail) = &self.state.rail {
                rail.is_closed()
            } else {
                let source = if self.state.options.parameters.is_some() {
                    self.state.sources.last()
                } else {
                    self.state.sources.first()
                };
                source
                    .and_then(|(_, geometry)| geometry.curve_ref())
                    .is_some_and(|curve| curve.is_closed().unwrap_or(false))
            }
    }

    pub(super) fn rail_targets(&self) -> impl Iterator<Item = Target> + '_ {
        self.state.rail.iter().flat_map(Rail::targets)
    }

    fn profiles(&self) -> usize {
        if self.state.rail.is_none() && !self.state.options.has_explicit_rail() {
            self.state.sources.len().saturating_sub(1)
        } else {
            // Private chain picking excludes the already selected profiles.
            // An explicit loose rail can also be selected as an object.
            let rail_selected = self.state.reference.is_some_and(|reference| {
                matches!(reference.target, Target::Curve(rail) if self.state.sources.iter().any(|(id, _)| *id == rail))
            });
            self.state.sources.len() - usize::from(rail_selected)
        }
    }
}
impl VibocerosApp {
    pub(super) fn review_open_sweep_profiles(&self, command: &str) -> bool {
        !command.contains('-') && !self.command_file.is_dispatching()
    }

    pub(super) fn should_review_preselected_sweep(&self) -> bool {
        let mut curves = self
            .document
            .selected_objects()
            .filter_map(|object| object.geometry().curve_ref());
        let closed_rail = curves
            .next()
            .is_some_and(|rail| rail.is_closed().unwrap_or(false));
        let first = curves.next();
        let second = curves.next();
        (closed_rail && second.is_some())
            || first
                .into_iter()
                .chain(second)
                .chain(curves)
                .any(|curve| curve.is_closed().unwrap_or(false))
    }

    pub(super) fn initialize_sweep_prompt(&mut self, input: &str) {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "Sweep1")
        {
            return;
        }
        let Ok((options, inherited_surface_style)) = self.initial_sweep_options(input) else {
            return;
        };
        let sources = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect();
        let (reference, rail) = if options.has_explicit_rail() {
            match self.capture_sweep_reference(&options) {
                Ok((reference, rail)) => (Some(reference), Some(rail)),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        self.sweep_session = Some(Session {
            state: State {
                options,
                inherited_surface_style,
                sources,
                rail,
                reference,
                stage: ProfileStage::Selecting,
                reversed_profiles: BTreeSet::new(),
            },
            undo: vec![],
            picking: None,
            value: None,
            seam_id_pending: false,
            alignment: None,
            preview: Default::default(),
            review_open_profiles: input
                .split_whitespace()
                .next()
                .is_some_and(|command| self.review_open_sweep_profiles(command)),
            chain_auto: true,
        });
    }
    fn capture_sweep_reference(
        &self,
        options: &Options,
    ) -> Result<(RailReference, Rail), CommandError> {
        let reference = options
            .rail_reference(&self.document)?
            .ok_or(CommandError::Usage("Specify a Sweep1 rail"))?;
        let rail = Rail::capture_reference(&self.document, reference)?;
        Ok((reference, rail))
    }
    pub(super) fn start_sweep_getter(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Sweep1")
        }) {
            return false;
        }
        let chain = words
            .iter()
            .position(|word| rail_picks::is_chain_option(word));
        let getter = chain.or_else(|| {
            let name = words.last()?.trim_start_matches('_');
            ["Point", "StartPoint", "EndPoint", "SetAxis"]
                .iter()
                .any(|value| name.eq_ignore_ascii_case(value))
                .then_some(words.len() - 1)
        });
        let Some(getter) = getter else {
            return false;
        };
        let base = words[..getter].join(" ");
        let Ok(Some(description)) = self.commands.object_selection_prompt(&base) else {
            return false;
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let mut pending = PendingObjectCommand::selecting(description);
        pending.command_override = Some(base.clone());
        pending.selection_before = Some(before);
        self.object_prompt = Some(pending);
        self.initialize_sweep_prompt(&base);
        if chain.is_some() {
            self.start_sweep_one_rail_chain(&words[getter + 1..].join(" "));
        } else if words[getter]
            .trim_start_matches('_')
            .eq_ignore_ascii_case("SetAxis")
        {
            self.begin_sweep_axis();
        } else {
            self.begin_sweep_point(words[getter]);
        }
        self.command_input.clear();
        true
    }
    pub(super) fn log_sweep_input(&mut self) {
        if let Some(p) = &self.sweep_session {
            self.push_log(format!(
                "Sweep1: {}. {}",
                p.hint(),
                p.state.effective_options().arguments()
            ));
        }
    }
    fn sync_sweep_selection(&mut self) {
        self.sweep_session.as_mut().unwrap().preview.invalidate();
        self.restore_sweep_picks();
    }

    fn restore_sweep_picks(&mut self) {
        let state = &self.sweep_session.as_ref().unwrap().state;
        let ids = state.sources.iter().map(|(id, _)| *id).collect();
        self.object_prompt.as_mut().unwrap().command_override =
            Some(format!("Sweep1 {}", state.effective_options().arguments()));
        self.restore_curve_selection(ids);
    }
    pub(super) fn finish_sweep_input(&mut self) {
        let p = self.sweep_session.as_ref().unwrap();
        if p.profiles() == 0 {
            self.push_log("Select a rail and at least one Sweep1 profile".into());
            return;
        }
        if !self.validate_sweep_sources() {
            return;
        }
        if self.advance_sweep_review() {
            return;
        }
        self.commit_sweep_preview();
    }

    fn validate_sweep_sources(&mut self) -> bool {
        let p = self.sweep_session.as_ref().unwrap();
        let valid = p.state.sources.iter().all(|(id, snapshot)| {
            self.document.is_object_selectable(*id)
                && self
                    .document
                    .object(*id)
                    .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
        });
        let same_reference = p
            .state
            .options
            .rail_reference(&self.document)
            .is_ok_and(|reference| reference == p.state.reference);
        let current_rail = p
            .state
            .rail
            .as_ref()
            .is_none_or(|rail| rail.current(&self.document));
        let current_seams = p.state.stage.seams().is_none_or(|editor| {
            editor
                .sources()
                .all(|source| source.current(&self.document))
        });
        if !valid || !same_reference || !current_rail || !current_seams {
            self.cancel_object_prompt(false);
            self.push_log("Sweep1 sources changed; select them again".into());
            return false;
        }
        true
    }
    pub(super) fn clear_sweep_input(&mut self) {
        if let Some(session) = self.sweep_session.take()
            && !session.review_open_profiles
            && session.profiles() > 0
        {
            self.remember_sweep_options(&session.state.effective_options());
        }
        if self.active_command == Some(InteractiveCommand::SweepPoint) {
            self.active_command = None;
            self.drafting_plane = None;
        }
    }
}
