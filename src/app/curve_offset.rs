//! Curve offset input and the latest transient result.
use super::*;
use std::sync::{Arc, mpsc};
use viboceros_command::offset_curve::{self as solver, Draft, Kind, OptionName, Settings, Target};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{Curve3, NurbsCurve};

pub(super) struct Session {
    sources: Vec<(ObjectId, GeometrySnapshot)>,
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
    pub(super) fn preview(&self, tolerance: Tolerance) -> Option<crate::viewport::OffsetDraft<'_>> {
        self.menu.is_none().then_some(crate::viewport::OffsetDraft {
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
        let mut selected = document
            .selected_objects()
            .filter(|object| object.geometry().curve_ref().is_some());
        self.sources.iter().all(|(id, storage)| {
            selected.next().is_some_and(|object| {
                object.id() == *id && storage.shares_storage_with(object.geometry_snapshot())
            })
        }) && selected.next().is_none()
    }

    fn prompt(&self) -> String {
        match self.menu {
            Some(OptionName::Distance) if self.distance_start.is_some() => {
                format!("Second distance point <{}>", self.settings.distance)
            }
            Some(OptionName::Distance) => format!("Offset distance <{}>", self.settings.distance),
            Some(OptionName::Count) => format!("Offset count <{}>", self.settings.count),
            Some(option) => option.hint().into(),
            None => self.settings.prompt(self.kind),
        }
    }
}

impl VibocerosApp {
    pub(super) fn start_curve_offset(&mut self, input: &str, postselected: bool) -> bool {
        let mut words = input.split_whitespace();
        let Some(kind) = words.next().and_then(Kind::from_name) else {
            return false;
        };
        let words = words.collect::<Vec<_>>();
        if solver::is_positional(kind, &words) {
            return false;
        }
        let Ok((settings, menu)) = solver::parse_settings(
            kind,
            &words,
            self.commands.offset_settings(kind),
            self.document.units(),
        ) else {
            return false;
        };
        let sources = self
            .document
            .selected_objects()
            .filter_map(|object| {
                let curve = object.geometry().curve_ref()?.to_owned();
                Some(((object.id(), object.geometry_snapshot().clone()), curve))
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return false;
        }
        let (sources, curves): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
        let analytic = curves.len() <= 8
            && curves.iter().all(|curve| {
                matches!(curve, Curve3::Line(_) | Curve3::Circle(_) | Curve3::Arc(_),)
            });
        let plane = self.viewports[self.active_viewport].construction_plane();
        self.cancel_interactive_command(false);
        if postselected {
            self.document
                .mark_object_selection_temporary(sources.iter().map(|(id, _)| *id));
        }
        self.curve_offset_session = Some(Session {
            sources,
            curves: Arc::new(curves),
            postselected,
            kind,
            settings,
            plane,
            menu,
            distance_start: None,
            analytic,
            rendered: None,
            worker: None,
        });
        self.drafting_plane = Some(plane);
        self.push_log(format!("> {input}"));
        self.curve_offset_prompt();
        true
    }

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

    fn curve_offset_prompt(&mut self) {
        let Some(session) = &self.curve_offset_session else {
            return;
        };
        self.commands
            .remember_offset_settings(session.kind, session.settings);
        self.active_command = Some(InteractiveCommand::CurveOffset {
            kind: session.kind,
            anchor: session.distance_start,
        });
        self.command_input.clear();
        self.push_log(session.prompt());
    }

    pub(super) fn continue_curve_offset(&mut self, text: &str) -> bool {
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
            let ids = session
                .sources
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            self.commands
                .commit_offset_plan(&mut self.document, draft, &ids, plan)
        });
        match result {
            Ok(count) => {
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
