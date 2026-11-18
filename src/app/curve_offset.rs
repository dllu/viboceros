//! Curve offset input and the latest transient result.
use super::*;
use std::sync::{Arc, mpsc};
use viboceros_command::offset_curve::{self as solver, Draft, Kind, OptionName, Settings, Target};
use viboceros_geometry::{Curve3, NurbsCurve};

mod sources;
use sources::{Input, Source};

pub(super) struct Session {
    sources: Vec<Source>,
    owners: Vec<solver::SourceOwner>,
    portions: Vec<NurbsCurve>,
    selecting: Option<Vec<Input>>,
    curves: Arc<Vec<Curve3>>,
    postselected: bool,
    kind: Kind,
    settings: Settings,
    plane: Frame3,
    menu: Option<OptionName>,
    distance_start: Option<Point3>,
    analytic: bool,
    rendered: Option<(Draft, Vec<NurbsCurve>)>,
    worker: Option<mpsc::Receiver<(Draft, Vec<NurbsCurve>)>>,
}

impl Session {
    pub(super) fn takes_distance_measurement(&self) -> bool {
        self.menu == Some(OptionName::Distance)
    }

    pub(super) fn portion_guides(&self) -> &[NurbsCurve] {
        &self.portions
    }

    pub(super) fn preview(&self, tolerance: Tolerance) -> Option<crate::viewport::OffsetDraft<'_>> {
        (self.selecting.is_none() && self.menu.is_none()).then_some(crate::viewport::OffsetDraft {
            kind: self.kind,
            settings: self.settings,
            normal: self.plane.z_axis(),
            tolerance,
            rendered: self.rendered.as_ref(),
            analytic_sources: (self.analytic && !self.settings.loose && !self.settings.in_cplane)
                .then_some(self.curves.as_slice()),
        })
    }

    fn sync_preview(&mut self, draft: Option<Draft>, ctx: &egui::Context) {
        if let Some(worker) = &self.worker {
            match worker.try_recv() {
                Ok(result) => {
                    if Some(result.0) == draft {
                        self.rendered = Some(result);
                        ctx.request_repaint();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
            self.worker = None;
        }
        let Some(draft) = draft else {
            self.rendered = None;
            return;
        };
        if self.rendered.as_ref().is_some_and(|(key, _)| *key == draft) {
            return;
        }
        let curves = Arc::clone(&self.curves);
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        if std::thread::Builder::new()
            .name("curve-offset-preview".into())
            .spawn(move || {
                let wires = draft.preview(&curves);
                let _ = sender.send((draft, wires));
                ctx.request_repaint();
            })
            .is_ok()
        {
            self.worker = Some(receiver);
        }
    }

    fn current(&self, document: &viboceros_document::Document) -> bool {
        if self.selecting.is_some() {
            return true;
        }
        let mut whole = self.sources.iter().filter(|source| source.whole);
        let portions = self
            .sources
            .iter()
            .filter(|s| !s.whole && s.edge.is_none())
            .map(|s| s.object)
            .collect::<std::collections::BTreeSet<_>>();
        for object in document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
        {
            if portions.contains(&object.id()) {
                continue;
            }
            if !whole.next().is_some_and(|expected| {
                object.id() == expected.object
                    && expected
                        .snapshot
                        .shares_storage_with(object.geometry_snapshot())
            }) {
                return false;
            }
        }
        whole.next().is_none()
            && sources::all_current(document, self.sources.iter().filter(|s| !s.whole))
    }

    fn prompt(&self) -> String {
        match self.menu {
            Some(OptionName::Distance) if self.distance_start.is_some() => {
                format!("Second distance point <{}>", self.settings.distance)
            }
            Some(OptionName::Distance) => format!("Offset distance <{}>", self.settings.distance),
            Some(OptionName::Count) => format!("Offset count <{}>", self.settings.count),
            Some(option) => option.hint().into(),
            None => self.settings.prompt(self.kind, self.selecting.is_some()),
        }
    }
}

impl VibocerosApp {
    pub(super) fn release_curve_offset(&mut self) {
        if self
            .curve_offset_session
            .take()
            .is_some_and(|s| s.postselected)
        {
            self.document.clear_temporary_object_selection();
        }
    }

    pub(super) fn validate_curve_offset(&mut self) {
        if self
            .curve_offset_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log("Offset sources changed. Select the curves again.".into());
        }
    }

    pub(super) fn sync_curve_offset_preview(&mut self, draft: Option<Draft>, ctx: &egui::Context) {
        if let Some(session) = &mut self.curve_offset_session {
            session.sync_preview(draft, ctx);
        }
    }

    pub(super) fn curve_offset_prompt(&mut self) {
        let Some(session) = &self.curve_offset_session else {
            return;
        };
        self.commands
            .remember_offset_settings(session.kind, session.settings);
        self.active_command = (session.selecting.is_none()
            || session.menu == Some(OptionName::Distance))
        .then_some(InteractiveCommand::CurveOffset {
            kind: session.kind,
            anchor: session.distance_start,
        });
        self.command_input.clear();
        self.push_log(session.prompt());
    }

    pub(super) fn continue_curve_offset(&mut self, text: &str) -> bool {
        if self
            .subcurve_prompt
            .as_ref()
            .is_some_and(|p| p.temporary == Some(subcurve_prompt::Temporary::Offset))
        {
            return false;
        }
        let Some(session) = &mut self.curve_offset_session else {
            return false;
        };
        if text
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            self.cancel_interactive_command(true);
            return true;
        }
        let enter = text.is_empty() || text.trim_start_matches('_').eq_ignore_ascii_case("Enter");
        if let Some(option) = session.menu {
            if enter {
                session.menu = None;
                session.distance_start = None;
            } else if option == OptionName::Distance
                && viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
                    .is_some()
                && solver::parse_length(text, self.document.units()).is_err()
            {
                return false;
            } else if let Err(error) =
                option.set(&mut session.settings, Some(text), self.document.units())
            {
                self.push_log(format!("Error: {error}"));
                return true;
            } else {
                session.menu = None;
                session.distance_start = None;
            }
            self.curve_offset_prompt();
            return true;
        }
        if session.selecting.is_some() && self.continue_curve_offset_sources(text, enter) {
            return true;
        }
        let session = self.curve_offset_session.as_mut().unwrap();
        if enter {
            if session.settings.both_sides && !session.settings.through {
                self.finish_curve_offset(Target::BothSides);
            } else {
                self.curve_offset_prompt();
            }
            return true;
        }
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_some_and(|word| {
            let name = word.split('=').next().unwrap();
            OptionName::parse(session.kind, name).is_some()
                || solver::parse_length(text, self.document.units()).is_ok()
        }) {
            if let Ok(distance) = solver::parse_length(text, self.document.units()) {
                if session.settings.through {
                    return false;
                }
                if distance <= 0. {
                    self.push_log("Offset distance must be positive".into());
                    return true;
                }
                session.settings.distance = distance;
            } else {
                match solver::parse_settings(
                    session.kind,
                    &words,
                    session.settings,
                    self.document.units(),
                ) {
                    Ok((settings, menu)) => {
                        session.settings = settings;
                        session.menu = menu;
                    }
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                }
            }
            self.curve_offset_prompt();
            return true;
        }
        false
    }

    pub(super) fn accept_curve_offset_point(&mut self, point: Point3) -> Option<bool> {
        if self
            .subcurve_prompt
            .as_ref()
            .is_some_and(|p| p.temporary == Some(subcurve_prompt::Temporary::Offset))
        {
            return None;
        }
        let session = self.curve_offset_session.as_mut()?;
        if let Some(option) = session.menu {
            if option != OptionName::Distance {
                return Some(false);
            }
            if let Some(start) = session.distance_start {
                let Ok(distance) = start.distance_to(point) else {
                    return Some(false);
                };
                if distance <= 0. {
                    self.push_log("Offset distance must be positive".into());
                    return Some(false);
                }
                session.settings.distance = distance;
                session.settings.through = false;
                session.menu = None;
                session.distance_start = None;
            } else {
                session.distance_start = Some(point);
            }
            self.last_point = Some(point);
            self.curve_offset_prompt();
            return Some(true);
        }
        if session.selecting.is_some() {
            return None;
        }
        let target = if session.settings.through {
            Target::Through(point)
        } else {
            Target::Side(point)
        };
        let result = self.finish_curve_offset(target);
        if result {
            self.last_point = Some(point);
        }
        Some(result)
    }

    fn finish_curve_offset(&mut self, target: Target) -> bool {
        let session = self.curve_offset_session.as_ref().unwrap();
        if !session.current(&self.document) {
            self.cancel_interactive_command(false);
            self.push_log("Offset sources changed. Select the curves again.".into());
            return false;
        }
        let draft = Draft {
            kind: session.kind,
            settings: session.settings,
            normal: session.plane.z_axis(),
            tolerance: self.document.tolerance(),
            target,
        };
        let result = draft.solve(&session.curves).and_then(|plan| {
            self.commands.commit_offset_plan(
                &mut self.document,
                draft,
                &session.owners,
                session.sources.iter().map(|source| source.object),
                plan,
            )
        });
        match result {
            Ok(count) => {
                let components = session
                    .sources
                    .iter()
                    .filter(|s| s.edge.is_some())
                    .map(|s| s.object)
                    .collect::<Vec<_>>();
                if !components.is_empty() {
                    self.component_selection.release_objects(&components);
                }
                self.cancel_interactive_command(false);
                self.command_input.clear();
                self.push_log(format!("Created {count} offset curve(s)"));
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                false
            }
        }
    }
}
