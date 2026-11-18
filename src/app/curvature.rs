//! Source selection and repeatable, nonmutating curvature inspection.
use super::*;
use crate::viewport::curvature::{Click, Guides, Hit, Input, Location};
use std::collections::BTreeSet;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{
    CommandError, CurvatureFeature, CurvatureSample, ObjectSelectionFilter,
    curve_curvature_features,
};
use viboceros_document::{GeometrySnapshot, HistoryGroup};

#[derive(Default)]
pub(super) struct Session {
    source: Option<Source>,
    hover: Option<Hover>,
    history: Option<HistoryGroup>,
}

struct Source {
    ids: BTreeSet<ObjectId>,
    snapshot: GeometrySnapshot,
    tolerance: Tolerance,
    curve: Option<CapturedCurve>,
    features: Vec<CurvatureFeature>,
}

impl Source {
    fn capture(document: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let object = document
            .object(id)
            .filter(|object| {
                document.is_object_selectable(id)
                    && ObjectSelectionFilter::Parametric.accepts_object(object)
            })
            .ok_or(CommandError::CurvatureRequiresCurveOrSurface)?;
        let snapshot = object.geometry_snapshot().clone();
        let (curve, features) = if snapshot.curve_ref().is_some() {
            let curve = CapturedCurve::capture(document, id, None)?;
            let features = curve_curvature_features(curve.curve_ref(), curve.curve());
            (Some(curve), features)
        } else {
            (None, Vec::new())
        };
        Ok(Self {
            ids: BTreeSet::from([id]),
            snapshot,
            curve,
            features,
            tolerance: document.tolerance(),
        })
    }

    fn id(&self) -> ObjectId {
        *self.ids.first().expect("captured curvature source")
    }

    fn current(&self, document: &Document) -> bool {
        document.tolerance() == self.tolerance
            && document.is_object_selectable(self.id())
            && document.object(self.id()).is_some_and(|object| {
                object
                    .geometry_snapshot()
                    .shares_storage_with(&self.snapshot)
            })
    }
}

struct Hover {
    viewport: usize,
    pixel: egui::Pos2,
    hit: Hit,
    sample: CurvatureSample,
    markers: Vec<Geometry>,
    status: String,
}

impl Session {
    pub(super) fn input(&self) -> Option<Input<'_>> {
        let source = self.source.as_ref()?;
        Some(Input {
            sources: &source.ids,
            curve: source.curve.as_ref(),
            features: &source.features,
        })
    }

    pub(super) fn guides(&self) -> Option<Guides<'_>> {
        let hover = self.hover.as_ref()?;
        Some(Guides {
            sample: hover.sample,
            markers: &hover.markers,
        })
    }

    fn sample(&self, document: &Document, hit: Hit) -> Result<CurvatureSample, CommandError> {
        let source = self
            .source
            .as_ref()
            .filter(|source| source.id() == hit.object && source.current(document))
            .ok_or(CommandError::Usage("Curvature source changed"))?;
        let geometry = &*source.snapshot;
        match hit.location {
            Location::Curve {
                parameter,
                inflection,
            } => {
                let curve = geometry
                    .curve_ref()
                    .ok_or(CommandError::CurvatureRequiresCurveOrSurface)?;
                Ok(CurvatureSample::on_curve(curve, parameter, inflection)?)
            }
            Location::Surface { face, uv } => {
                CurvatureSample::on_surface(geometry, face, uv, document.tolerance())
            }
        }
    }
}

impl VibocerosApp {
    pub(super) fn start_curvature(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words
            .next()
            .is_some_and(|word| option_is(word, "Curvature"))
        {
            return false;
        }
        let Some(mark) = mark_option(words, self.curvature_mark) else {
            return false;
        };
        self.cancel_interactive_command(false);
        let mut session = Session::default();
        let preselected = if self.document.selected_object_count() == 1 {
            self.document
                .selected_objects()
                .find(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
                .map(|object| object.id())
        } else {
            None
        };
        if let Some(id) = preselected {
            match Source::capture(&self.document, id) {
                Ok(source) => session.source = Some(source),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.curvature_mark = mark;
        self.curvature_session = Some(session);
        self.active_command = Some(InteractiveCommand::Curvature { mark });
        self.command_input.clear();
        self.curvature_prompt();
        true
    }

    pub(super) fn selecting_curvature_source(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curvature_session
                .as_ref()
                .is_some_and(|s| s.source.is_none())
    }

    fn curvature_prompt(&mut self) {
        let text = if self.selecting_curvature_source() {
            "Select a curve or surface for curvature measurement".into()
        } else {
            format!(
                "Pick a point for curvature measurement or press Enter when done (MarkCurvature={})",
                if self.curvature_mark { "Yes" } else { "No" },
            )
        };
        self.push_log(text);
    }

    pub(super) fn pick_curvature_source(&mut self, id: Option<ObjectId>) -> bool {
        if !self.selecting_curvature_source() {
            return false;
        }
        if let Some(id) = id {
            match Source::capture(&self.document, id) {
                Ok(source) => {
                    self.document.clear_selection();
                    let session = self.curvature_session.as_mut().unwrap();
                    session.source = Some(source);
                    session.hover = None;
                    self.curvature_prompt();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        true
    }

    pub(super) fn validate_curvature_source(&mut self) {
        if let Some(session) = &mut self.curvature_session
            && session
                .source
                .as_ref()
                .is_some_and(|source| !source.current(&self.document))
        {
            session.source = None;
            session.hover = None;
            self.push_log("Curvature source changed. Select a curve or surface again.".into());
        }
    }

    pub(super) fn continue_curvature(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Curvature { mark }) = self.active_command else {
            return false;
        };
        if self.plane_prompt.is_some() || self.curvature_session.is_none() {
            return false;
        }
        let text = input.trim();
        if text.is_empty() || option_is(text, "Enter") {
            self.cancel_interactive_command(false);
        } else if option_is(text.split('=').next().unwrap_or(""), "MarkCurvature") {
            if let Some(mark) = mark_option(std::iter::once(text), mark) {
                self.curvature_mark = mark;
                self.active_command = Some(InteractiveCommand::Curvature { mark });
                self.curvature_prompt();
            } else {
                self.push_log("MarkCurvature takes Yes or No".into());
            }
        } else if self.selecting_curvature_source()
            && let Some(id) = text.parse().ok().or_else(|| {
                let (key, value) = text.split_once(char::is_whitespace)?;
                option_is(key, "SelID")
                    .then(|| value.trim().parse().ok())
                    .flatten()
            })
        {
            self.pick_curvature_source(Some(id));
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    pub(super) fn curvature_status(&self) -> Option<&str> {
        self.curvature_session
            .as_ref()?
            .hover
            .as_ref()
            .map(|hover| hover.status.as_str())
    }

    pub(super) fn update_curvature_hover(&mut self, hit: Option<(usize, egui::Pos2, Hit)>) -> bool {
        let Some(session) = &mut self.curvature_session else {
            return false;
        };
        if session
            .hover
            .as_ref()
            .is_some_and(|old| hit == Some((old.viewport, old.pixel, old.hit)))
        {
            return false;
        }
        let hover = hit.and_then(|(viewport, pixel, hit)| {
            let sample = session.sample(&self.document, hit).ok()?;
            let status = match sample {
                CurvatureSample::Curve { curvature, .. } => {
                    let magnitude = curvature.length().ok()?;
                    if magnitude == 0. {
                        "Radius = infinite".into()
                    } else {
                        format!("Radius = {:.8}", 1. / magnitude)
                    }
                }
                CurvatureSample::Surface { curvature, .. } => {
                    let [k1, k2] = curvature.principal;
                    format!("Principal curvatures = {k1:.6}, {k2:.6}")
                }
            };
            Some(Hover {
                viewport,
                pixel,
                hit,
                sample,
                markers: sample
                    .markers(self.document.tolerance())
                    .unwrap_or_default(),
                status,
            })
        });
        let changed = session.hover.is_some() || hover.is_some();
        session.hover = hover;
        changed
    }

    pub(super) fn pick_curvature_location(&mut self, click: Click) -> bool {
        let Some(InteractiveCommand::Curvature { mark }) = self.active_command else {
            return false;
        };
        let Some(session) = &self.curvature_session else {
            return false;
        };
        let visible = session.hover.as_ref().filter(|hover| {
            hover.viewport == click.viewport
                && hover.pixel == click.pixel
                && session
                    .source
                    .as_ref()
                    .is_some_and(|source| source.current(&self.document))
        });
        let sample = if let Some(hover) = visible {
            Ok(hover.sample)
        } else if let Some(hit) = click.hit {
            session.sample(&self.document, hit)
        } else {
            return true;
        };
        self.apply_curvature_sample(sample, mark, true);
        true
    }

    pub(super) fn finish_curvature_point(&mut self, point: Point3, mark: bool) -> bool {
        if self.selecting_curvature_source() {
            self.curvature_prompt();
            return false;
        }
        let sample = if let Some(source) = self
            .curvature_session
            .as_ref()
            .and_then(|session| session.source.as_ref())
        {
            CurvatureSample::nearest(
                self.document
                    .object(source.id())
                    .filter(|_| source.current(&self.document))
                    .into_iter(),
                point,
                self.document.tolerance(),
            )
        } else {
            CurvatureSample::nearest(
                self.document.selected_objects(),
                point,
                self.document.tolerance(),
            )
        };
        self.apply_curvature_sample(sample, mark, false)
    }

    fn apply_curvature_sample(
        &mut self,
        sample: Result<CurvatureSample, CommandError>,
        mark: bool,
        repeat: bool,
    ) -> bool {
        let result: Result<String, CommandError> = (|| {
            let sample = sample?;
            let report = sample.report()?;
            if mark {
                let markers = sample.markers(self.document.tolerance())?;
                let session = self
                    .curvature_session
                    .as_mut()
                    .ok_or(CommandError::Usage("Curvature picker is unavailable"))?;
                if session.history.is_none() {
                    session.history = Some(self.document.begin_history_group("Curvature")?);
                }
                let group = session.history.as_mut().unwrap();
                self.document.begin_group_transaction(group)?;
                let insertion = markers
                    .into_iter()
                    .try_for_each(|marker| self.document.add_geometry(marker).map(|_| ()));
                if let Err(error) = insertion
                    .and_then(|_| self.document.commit_group_transaction(group).map(|_| ()))
                {
                    self.document.rollback_transaction()?;
                    return Err(error.into());
                }
            }
            Ok(report)
        })();
        match result {
            Ok(report) => {
                self.snaps.model_override = None;
                if !repeat {
                    self.cancel_interactive_command(false);
                }
                self.push_log(report);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}

fn option_is(word: &str, name: &str) -> bool {
    word.trim_start_matches(['_', '-', '\''])
        .eq_ignore_ascii_case(name)
}

fn mark_option<'a>(options: impl Iterator<Item = &'a str>, default: bool) -> Option<bool> {
    let mut mark = default;
    for option in options {
        if option_is(option, "MarkCurvature") {
            mark = !mark;
        } else if let Some((key, value)) = option.split_once('=')
            && option_is(key, "MarkCurvature")
        {
            mark = if option_is(value, "Yes") {
                true
            } else if option_is(value, "No") {
                false
            } else {
                return None;
            };
        } else {
            return None;
        }
    }
    Some(mark)
}
