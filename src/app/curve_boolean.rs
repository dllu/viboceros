//! Source selection, repeatable region picks and cancellable region previews.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::curve_boolean::{Options, Session, Source, parse, prepare_with_inputs};
mod subcurve;
#[derive(Debug)]
pub(super) struct Prompt {
    options: Options,
    session: Option<Session>,
    scene: Option<Document>,
    prior: Vec<ObjectId>,
    cplane: Frame3,
    postselected: bool,
    pub(super) subcurves: crate::app::uv_subcurve_input::SubcurveInputs,
}
impl Prompt {
    pub(super) fn subcurve_hover(&self) -> Option<f64> {
        self.subcurves.pending.as_ref()?.hover_parameter
    }
    pub(super) fn selecting(&self) -> bool {
        self.session.is_none()
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub(super) fn plane(&self) -> Option<Frame3> {
        self.session.as_ref().map(Session::plane)
    }
    pub(super) fn wants_point(&self) -> bool {
        self.session.is_some()
            || self
                .subcurves
                .pending
                .as_ref()
                .is_some_and(|p| p.object.is_some())
    }
    pub(super) fn hint(&self) -> String {
        if let Some(hint) = self.subcurves.hint() {
            return hint.into();
        }
        if let Some(session) = &self.session {
            format!(
                "Click inside regions to keep ({} of {}); AllRegions; DeleteInput={} CombineRegions={} Output={} SimplifyCurves={}; Enter accepts, Esc cancels",
                session.selected_count(),
                session.region_count(),
                self.options.delete_input.token(),
                if self.options.combine { "Yes" } else { "No" },
                self.options.output.token(),
                if self.options.simplify { "Yes" } else { "No" }
            )
        } else {
            "Select curves; Enter continues, Esc cancels".into()
        }
    }
}
impl VibocerosApp {
    pub(super) fn select_curve_boolean_sources(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        self.select_curve_boolean_source_ids(ids, mode, true);
    }
    fn select_curve_boolean_source_ids(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
        expand_groups: bool,
    ) {
        if self
            .curve_boolean_prompt
            .as_ref()
            .is_none_or(|p| !p.selecting())
        {
            return;
        }
        let ids = ids.into_iter().collect::<Vec<_>>();
        if self
            .curve_boolean_prompt
            .as_ref()
            .unwrap()
            .subcurves
            .pending
            .is_some()
        {
            if let [id] = ids.as_slice() {
                self.pick_curve_boolean_subcurve(*id, None);
            }
            return;
        }
        let requested = ids.into_iter().collect::<BTreeSet<_>>();
        let seeds = self
            .document
            .selectable_objects()
            .filter(|o| requested.contains(&o.id()) && o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        let groups = seeds
            .iter()
            .filter(|_| expand_groups)
            .filter_map(|id| self.document.object(*id)?.top_group())
            .collect::<BTreeSet<_>>();
        let expanded = groups
            .iter()
            .filter_map(|g| self.document.group(*g))
            .flat_map(|g| g.members())
            .chain(seeds)
            .collect::<BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|o| expanded.contains(&o.id()) && o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let previous = self.document.selected_object_ids().collect::<Vec<_>>();
        let old = previous.iter().copied().collect::<BTreeSet<_>>();
        let mut next = old.clone();
        for id in &ids {
            match mode {
                SelectionMode::Add | SelectionMode::Replace => {
                    next.insert(*id);
                }
                SelectionMode::Remove => {
                    next.remove(id);
                }
                SelectionMode::Toggle => {
                    if !next.remove(id) {
                        next.insert(*id);
                    }
                }
            }
        }
        let ordered = previous
            .into_iter()
            .filter(|id| next.contains(id))
            .chain(
                ids.into_iter()
                    .filter(|id| next.contains(id) && !old.contains(id)),
            )
            .collect::<Vec<_>>();
        match self.document.set_transient_selection(ordered) {
            Ok(_) => self.push_log(format!(
                "Selected {} object(s); Enter continues",
                self.document.selected_object_count()
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    pub(super) fn start_curve_boolean(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("CurveBoolean")
        }) {
            return false;
        }
        let parsed = match parse(&words[1..], self.commands.curve_boolean_options()) {
            Ok(input)
                if input.sources.is_some()
                    || input.all_regions
                    || !input.points.is_empty()
                    || !input.subcurves.is_empty() =>
            {
                return false;
            }
            Ok(input) => input,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let prior = self.document.selected_object_ids().collect::<Vec<_>>();
        let postselected = !self
            .document
            .selected_objects()
            .any(|o| o.geometry().curve_ref().is_some());
        self.cancel_interactive_command(false);
        self.curve_boolean_prompt = Some(Prompt {
            options: parsed.options,
            session: None,
            scene: None,
            prior,
            postselected,
            subcurves: Default::default(),
            cplane: self.viewports[self.active_viewport].construction_plane(),
        });
        self.commands.remember_curve_boolean_options(parsed.options);
        self.push_log(format!("> {input}"));
        if self
            .document
            .selected_objects()
            .any(|o| o.geometry().curve_ref().is_some())
        {
            self.prepare_curve_boolean();
        }
        self.log_curve_boolean();
        true
    }
    fn log_curve_boolean(&mut self) {
        if let Some(p) = &self.curve_boolean_prompt {
            self.push_log(format!("CurveBoolean: {}", p.hint()));
        }
    }
    pub(super) fn cancel_curve_boolean(&mut self) {
        self.discard_curve_boolean_subcurve();
        if let Some(p) = self.curve_boolean_prompt.take() {
            let ids = p
                .prior
                .into_iter()
                .filter(|id| self.document.is_object_selectable(*id))
                .collect::<Vec<_>>();
            let _ = self.document.set_transient_selection(ids);
        }
    }
    fn prepare_curve_boolean(&mut self) {
        self.validate_curve_boolean();
        let Some(p) = self.curve_boolean_prompt.as_ref() else {
            return;
        };
        let ids = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let mut inputs = ids.into_iter().map(Source::Whole).collect::<Vec<_>>();
        inputs.extend(p.subcurves.ranges.iter().map(|r| Source::Subcurve {
            object: r.object,
            edge: r.edge_reference.as_ref().and_then(|r| r.source().1),
            parameters: r.parameters,
            wrap_edge: r.edge_wrap,
        }));
        match prepare_with_inputs(&self.document, &inputs, p.cplane) {
            Ok(mut session) => {
                if p.postselected {
                    session.set_postselected();
                }
                self.curve_boolean_prompt.as_mut().unwrap().session = Some(session);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
    pub(super) fn validate_curve_boolean(&mut self) {
        if self.curve_boolean_prompt.as_ref().is_some_and(|p| {
            !p.subcurves.ranges.iter().all(|r| r.current(&self.document))
                || p.session
                    .as_ref()
                    .is_some_and(|s| !s.is_current(&self.document))
                || p.cplane != self.viewports[self.active_viewport].construction_plane()
        }) {
            self.cancel_curve_boolean();
            self.push_log(
                "CurveBoolean inputs or document settings changed; select curves again".into(),
            );
        }
    }
    fn update_curve_boolean_preview(&mut self) {
        let p = self.curve_boolean_prompt.as_mut().unwrap();
        p.scene = None;
        let Some(session) = &p.session else {
            return;
        };
        let result = (|| -> Result<Document, viboceros_command::CommandError> {
            let prepared = session.stage(p.options)?;
            let mut doc = self.document.clone();
            prepared.commit(&mut doc)?;
            doc.clear_history()?;
            Ok(doc)
        })();
        match result {
            Ok(doc) => self.curve_boolean_prompt.as_mut().unwrap().scene = Some(doc),
            Err(error) => self.push_log(format!("Preview error: {error}")),
        }
    }
    pub(super) fn accept_curve_boolean_point(&mut self, point: Point3) -> Option<bool> {
        self.curve_boolean_prompt.as_ref()?.session.as_ref()?;
        self.validate_curve_boolean();
        let Some(p) = self.curve_boolean_prompt.as_mut() else {
            return Some(false);
        };
        match p.session.as_mut().unwrap().toggle(point) {
            Ok(true) => self.update_curve_boolean_preview(),
            Ok(false) => self.push_log("Pick inside a bounded region".into()),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.log_curve_boolean();
        Some(true)
    }
    pub(super) fn continue_curve_boolean(&mut self, input: &str) -> bool {
        if self.continue_curve_boolean_subcurve(input) {
            return true;
        }
        let Some(p) = self.curve_boolean_prompt.as_ref() else {
            return false;
        };
        let first = input
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('_');
        if first.eq_ignore_ascii_case("Cancel") {
            self.cancel_curve_boolean();
            self.command_input.clear();
            return true;
        }
        if self.commands.recognizes(first) {
            self.cancel_curve_boolean();
            return false;
        }
        if input.is_empty() {
            if p.selecting() {
                self.prepare_curve_boolean();
                self.log_curve_boolean();
            } else {
                self.validate_curve_boolean();
                if let Some(p) = &self.curve_boolean_prompt {
                    let result = p
                        .session
                        .as_ref()
                        .unwrap()
                        .stage(p.options)
                        .and_then(|s| s.commit(&mut self.document));
                    match result {
                        Ok(ids) => {
                            self.curve_boolean_prompt = None;
                            self.push_log(format!(
                                "CurveBoolean: created {} region object(s)",
                                ids.len()
                            ));
                        }
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                }
            }
            self.command_input.clear();
            return true;
        }
        if p.selecting()
            && let Ok(id) = input.parse::<ObjectId>()
        {
            self.select_curve_boolean_source_ids([id], SelectionMode::Add, false);
            self.command_input.clear();
            return true;
        }
        let edit = if input.split_whitespace().count() == 2 && !input.contains('=') {
            let mut words = input.split_whitespace();
            format!("{}={}", words.next().unwrap(), words.next().unwrap())
        } else {
            input.to_owned()
        };
        match parse(&edit.split_whitespace().collect::<Vec<_>>(), p.options) {
            Ok(parsed) if parsed.sources.is_none() => {
                if p.selecting() && (parsed.all_regions || !parsed.points.is_empty()) {
                    self.prepare_curve_boolean();
                }
                self.validate_curve_boolean();
                if let Some(p) = self.curve_boolean_prompt.as_mut() {
                    p.options = parsed.options;
                    self.commands.remember_curve_boolean_options(p.options);
                    if let Some(session) = &mut p.session {
                        if parsed.all_regions {
                            session.all_regions();
                        }
                        for point in parsed.points {
                            if let Err(error) = session.toggle(point) {
                                self.push_log(format!("Error: {error}"));
                                break;
                            }
                        }
                    }
                    self.update_curve_boolean_preview();
                }
                self.log_curve_boolean();
            }
            Ok(_) => self.log_curve_boolean(),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
