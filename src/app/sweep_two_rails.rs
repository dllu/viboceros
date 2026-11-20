//! Ordered rail and profile picks followed by a review of the exact surface.
use super::{object_selection::PendingObjectCommand, *};
use std::collections::BTreeSet;
use viboceros_command::sweep_two_rails::{MAX_SECTIONS, Options, SectionFit, Source, point_count};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::SweepEndPoints;

mod input;
mod points;
mod preview;
use points::End;

#[derive(Clone, Debug)]
struct Pick {
    id: ObjectId,
    geometry: GeometrySnapshot,
}

impl Pick {
    fn new(document: &Document, id: ObjectId) -> Option<Self> {
        let object = document.object(id)?;
        (document.is_object_selectable(id) && object.geometry().curve_ref().is_some()).then(|| {
            Self {
                id,
                geometry: object.geometry_snapshot().clone(),
            }
        })
    }

    fn current(&self, document: &Document) -> bool {
        document.is_object_selectable(self.id)
            && document.object(self.id).is_some_and(|object| {
                self.geometry
                    .shares_storage_with(object.geometry_snapshot())
            })
    }
}

#[derive(Clone, Debug)]
struct State {
    picks: Vec<Pick>,
    fit: SectionFit,
    ends: SweepEndPoints,
    rebuild: usize,
    refit: f64,
    review: bool,
}

#[derive(Debug)]
pub(super) struct Session {
    options: Options,
    named_rails: Option<[Pick; 2]>,
    state: State,
    undo: Vec<State>,
    value: Option<&'static str>,
    picking: Option<End>,
    preview: preview::Preview,
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        !self.state.review && self.value.is_none() && self.picking.is_none()
    }

    fn rail_count(&self) -> usize {
        self.named_rails
            .as_ref()
            .map_or(self.state.picks.len().min(2), |_| 2)
    }

    fn section_count(&self) -> usize {
        self.state.picks.len()
            - if self.named_rails.is_some() {
                0
            } else {
                self.rail_count()
            }
    }

    fn ready(&self, ends: SweepEndPoints) -> bool {
        self.section_count() > 0 && self.section_count() + point_count(ends) >= 2
    }

    fn ids(&self) -> Vec<ObjectId> {
        self.state.picks.iter().map(|pick| pick.id).collect()
    }

    pub(super) fn hint(&self) -> &'static str {
        if let Some(end) = self.picking {
            return end.prompt();
        }
        match self.value {
            Some("Simplify") => "Choose None, Rebuild or Refit",
            Some("Rebuild") => "Enter the cross-section control point count",
            Some("Refit") => "Enter the cross-section refit tolerance",
            _ if self.state.review => {
                "Simplify / Rebuild / Refit / StartPoint / EndPoint / Undo. Enter creates the sweep"
            }
            _ => match self.rail_count() {
                0 => "Select the first rail",
                1 => "Select the second rail or use Undo",
                _ => "Select cross sections in order or choose Point. Enter reviews the sweep",
            },
        }
    }
}

impl VibocerosApp {
    pub(super) fn start_sweep_two_rails(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(command) = words.next().filter(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Sweep2")
        }) else {
            return false;
        };
        let arguments = words.collect::<Vec<_>>();
        let options = match Options::parse(&arguments) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let named_rails = match options.explicit_rails(&self.document) {
            Ok(Some([first, second])) => {
                match [
                    Pick::new(&self.document, first),
                    Pick::new(&self.document, second),
                ] {
                    [Some(first), Some(second)] => Some([first, second]),
                    _ => {
                        self.push_log("Sweep2 named rails must be selectable curves".into());
                        return true;
                    }
                }
            }
            Ok(None) => None,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let picks = before
            .iter()
            .copied()
            .filter(|id| {
                named_rails
                    .as_ref()
                    .is_none_or(|rails| rails.iter().all(|rail| rail.id != *id))
            })
            .filter_map(|id| Pick::new(&self.document, id))
            .collect::<Vec<_>>();
        let mut session = Session {
            state: State {
                picks,
                fit: options.fit,
                ends: options.ends,
                rebuild: match options.fit {
                    SectionFit::Rebuild(count) => count,
                    _ => 5,
                },
                refit: match options.fit {
                    SectionFit::Refit(distance) => distance,
                    _ => self.document.tolerance().absolute(),
                },
                review: false,
            },
            options,
            named_rails,
            undo: Vec::new(),
            value: None,
            picking: None,
            preview: Default::default(),
        };
        if session.ready(session.state.ends) && (!arguments.is_empty() || command.contains('-')) {
            // Preserve direct scripted invocations with complete preselection.
            self.execute_command(input);
            return true;
        }
        if session.section_count() + point_count(session.state.ends) > MAX_SECTIONS {
            self.push_log(format!(
                "Sweep2 accepts at most {MAX_SECTIONS} cross sections"
            ));
            return true;
        }
        if session.ready(session.state.ends) {
            session.undo.push(session.state.clone());
            session.state.review = true;
        }
        let description = self
            .commands
            .object_selection_prompt(input)
            .unwrap()
            .unwrap();
        let mut pending = PendingObjectCommand::selecting(description);
        pending.selection_before = Some(before);
        self.restore_curve_selection(session.ids());
        self.object_prompt = Some(pending);
        self.sweep_two_rails = Some(session);
        self.push_log(format!("> {input}"));
        self.log_sweep_two_rails();
        true
    }

    pub(super) fn select_sweep_two_rail_curves(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(session) = self
            .sweep_two_rails
            .as_ref()
            .filter(|session| session.selecting())
        else {
            return;
        };
        let limit = MAX_SECTIONS - point_count(session.state.ends)
            + if session.named_rails.is_some() { 0 } else { 2 };
        let mut next = session.state.picks.clone();
        let removable = if mode == SelectionMode::Toggle {
            next.len()
        } else {
            0
        };
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id)
                || session
                    .named_rails
                    .as_ref()
                    .is_some_and(|rails| rails.iter().any(|rail| rail.id == id))
            {
                continue;
            }
            let position = next.iter().position(|pick| pick.id == id);
            match (mode, position) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    next.remove(index);
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => {
                    if let Some(pick) = Pick::new(&self.document, id) {
                        next.push(pick);
                    }
                }
            }
            if next.len() > limit + removable {
                break;
            }
        }
        if next.len() > limit {
            self.push_log(format!(
                "Sweep2 accepts at most {MAX_SECTIONS} cross sections"
            ));
            return;
        }
        if next
            .iter()
            .map(|pick| pick.id)
            .eq(session.state.picks.iter().map(|pick| pick.id))
        {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.undo.push(session.state.clone());
        session.state.picks = next;
        self.sync_sweep_two_rail_picks();
        self.log_sweep_two_rails();
    }

    fn sync_sweep_two_rail_picks(&mut self) {
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.preview.invalidate();
        let ids = session.ids();
        self.restore_curve_selection(ids);
    }

    fn log_sweep_two_rails(&mut self) {
        let session = self.sweep_two_rails.as_ref().unwrap();
        let hint = session.hint();
        let fit = match session.state.fit {
            SectionFit::Unchanged => "Cross sections unchanged".to_owned(),
            SectionFit::Rebuild(count) => format!("Rebuild={count}"),
            SectionFit::Refit(distance) => format!("Refit={distance}"),
        };
        self.push_log(if session.state.review && session.value.is_none() {
            format!("Sweep2: {hint}. {fit}")
        } else {
            format!("Sweep2: {hint}")
        });
    }

    fn validate_sweep_two_rails(&mut self) -> bool {
        let session = self.sweep_two_rails.as_ref().unwrap();
        let named_ids = session
            .named_rails
            .as_ref()
            .map(|rails| [rails[0].id, rails[1].id]);
        let current = session
            .options
            .explicit_rails(&self.document)
            .is_ok_and(|rails| rails == named_ids)
            && session
                .state
                .picks
                .iter()
                .chain(session.named_rails.iter().flatten())
                .all(|pick| pick.current(&self.document));
        if !current {
            self.cancel_object_prompt(false);
            self.push_log("Sweep2 sources changed. Select them again".into());
        }
        current
    }
}
