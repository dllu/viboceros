//! Ordered rail and profile picks followed by a review of the exact surface.
use super::{object_selection::PendingObjectCommand, *};
use std::collections::BTreeSet;
use viboceros_command::chain_selection::Target;
use viboceros_command::sweep_two_rails::{
    MAX_SECTIONS, Options, Rail, SectionFit, ShapeOptions, Source, point_count,
};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::SweepEndPoints;

mod input;
mod points;
mod preview;
use points::End;

fn chain_option(word: &str) -> bool {
    ["ChainEdges", "Chain"].iter().any(|name| {
        word.trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case(name)
    })
}

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
    rails: Vec<Rail>,
    picks: Vec<Pick>,
    shape: ShapeOptions,
    rebuild: usize,
    refit: f64,
    review: bool,
}

#[derive(Debug)]
pub(super) struct Session {
    options: Options,
    named_rails: Option<[ObjectId; 2]>,
    pub(super) chain_auto: bool,
    state: State,
    undo: Vec<State>,
    value: Option<&'static str>,
    picking: Option<End>,
    preview: preview::Preview,
}

impl State {
    fn capture(&self, document: &Document) -> Result<Source, viboceros_command::CommandError> {
        let [first, second] = self.rails.as_slice() else {
            return Err(viboceros_command::CommandError::Usage(
                "Select both Sweep2 rails",
            ));
        };
        let profiles = self.picks.iter().map(|pick| pick.id).collect::<Vec<_>>();
        Source::with_rails(document, [first.clone(), second.clone()], &profiles)
    }
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        !self.state.review && self.value.is_none() && self.picking.is_none()
    }

    fn rail_count(&self) -> usize {
        self.state.rails.len()
    }

    fn section_count(&self) -> usize {
        self.state.picks.len()
    }

    fn ready(&self) -> bool {
        self.rail_count() == 2 && self.section_count() > 0
    }

    fn profile_ids(&self) -> Vec<ObjectId> {
        self.state.picks.iter().map(|pick| pick.id).collect()
    }

    pub(super) fn rail_targets(&self) -> impl Iterator<Item = Target> + '_ {
        self.state.rails.iter().flat_map(Rail::targets)
    }

    fn ids(&self) -> Vec<ObjectId> {
        self.rail_targets()
            .filter_map(|target| match target {
                Target::Curve(id) if self.named_rails.is_none() => Some(id),
                _ => None,
            })
            .chain(self.state.picks.iter().map(|pick| pick.id))
            .collect()
    }

    pub(super) fn hint(&self) -> &'static str {
        if let Some(end) = self.picking {
            return end.prompt();
        }
        match self.value {
            Some("Simplify") => "Choose None, Rebuild or Refit",
            Some("Rebuild") => "Enter the cross-section control point count",
            Some("Refit") => "Enter the cross-section refit tolerance",
            Some("MaintainHeight") => "Maintain height? Choose Yes or No",
            Some("RefitRails") => "Refit rails? Choose Yes or No",
            _ if self.state.review => {
                "Simplify / Rebuild / Refit / RefitRails / MaintainHeight / StartPoint / EndPoint / Undo. Enter creates the sweep"
            }
            _ => match self.rail_count() {
                0 => "Select the first rail or choose ChainEdges",
                1 => "Select the second rail or choose ChainEdges or Undo",
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
        let words = words.collect::<Vec<_>>();
        let chain = words.iter().position(|word| chain_option(word));
        let arguments = &words[..chain.unwrap_or(words.len())];
        let options = match Options::parse(arguments) {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        let named_rails = match options.explicit_rails(&self.document) {
            Ok(rails) => rails,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let mut picks = before
            .iter()
            .copied()
            .filter(|id| named_rails.is_none_or(|rails| !rails.contains(id)))
            .filter_map(|id| Pick::new(&self.document, id))
            .collect::<Vec<_>>();
        let rail_ids = named_rails.map_or_else(
            || {
                picks
                    .drain(..picks.len().min(2))
                    .map(|pick| pick.id)
                    .collect::<Vec<_>>()
            },
            |rails| rails.to_vec(),
        );
        let rails = match rail_ids
            .into_iter()
            .map(|id| Rail::capture(&self.document, Target::Curve(id)))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(rails) => rails,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let mut session = Session {
            state: State {
                rails,
                picks,
                shape: options.shape,
                rebuild: match options.shape.fit {
                    SectionFit::Rebuild(count) => count,
                    _ => 5,
                },
                refit: match options.shape.fit {
                    SectionFit::Refit(distance) => distance,
                    _ => self.document.tolerance().absolute(),
                },
                review: false,
            },
            options,
            named_rails,
            chain_auto: true,
            undo: Vec::new(),
            value: None,
            picking: None,
            preview: Default::default(),
        };
        if session.ready() && chain.is_none() && (!arguments.is_empty() || command.contains('-')) {
            // Preserve direct scripted invocations with complete preselection.
            self.execute_command(input);
            return true;
        }
        if session.section_count() + point_count(session.state.shape.ends) > MAX_SECTIONS {
            self.push_log(format!(
                "Sweep2 accepts at most {MAX_SECTIONS} cross sections"
            ));
            return true;
        }
        if session.ready() {
            session.undo.push(session.state.clone());
            session.state.review = true;
        }
        let description = self
            .commands
            .object_selection_prompt(&format!("{command} {}", arguments.join(" ")))
            .unwrap()
            .unwrap();
        let mut pending = PendingObjectCommand::selecting(description);
        pending.selection_before = Some(before);
        self.restore_curve_selection(session.ids());
        self.object_prompt = Some(pending);
        self.sweep_two_rails = Some(session);
        self.push_log(format!("> {input}"));
        if let Some(chain) = chain {
            self.start_sweep_two_rail_chain(&words[chain + 1..].join(" "));
        } else {
            self.log_sweep_two_rails();
        }
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
            .filter(|session| session.selecting() && self.chain_prompt.is_none())
        else {
            return;
        };
        let limit = MAX_SECTIONS - point_count(session.state.shape.ends);
        let mut next = session.state.clone();
        let removable = if mode == SelectionMode::Toggle {
            next.picks.len()
        } else {
            0
        };
        let mut seen = BTreeSet::new();
        let mut changed = false;
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            let target = Target::Curve(id);
            if let Some(index) = next
                .rails
                .iter()
                .position(|rail| rail.targets().any(|piece| piece == target))
            {
                if matches!(mode, SelectionMode::Remove | SelectionMode::Toggle)
                    && session.named_rails.is_none()
                    && next.rails[index].targets().count() == 1
                {
                    next.rails.remove(index);
                    changed = true;
                }
                continue;
            }
            let position = next.picks.iter().position(|pick| pick.id == id);
            match (mode, position) {
                (SelectionMode::Remove | SelectionMode::Toggle, Some(index)) => {
                    next.picks.remove(index);
                    changed = true;
                }
                (SelectionMode::Remove, None) | (_, Some(_)) => {}
                (_, None) => {
                    if let Some(pick) = Pick::new(&self.document, id) {
                        if next.rails.len() < 2 {
                            match Rail::capture(&self.document, target) {
                                Ok(rail) => next.rails.push(rail),
                                Err(error) => {
                                    self.push_log(format!("Error: {error}"));
                                    return;
                                }
                            }
                        } else {
                            next.picks.push(pick);
                        }
                        changed = true;
                    }
                }
            }
            if next.picks.len() > limit + removable {
                break;
            }
        }
        if next.picks.len() > limit {
            self.push_log(format!(
                "Sweep2 accepts at most {MAX_SECTIONS} sections including point ends"
            ));
            return;
        }
        if !changed {
            return;
        }
        let session = self.sweep_two_rails.as_mut().unwrap();
        session
            .undo
            .push(std::mem::replace(&mut session.state, next));
        self.sync_sweep_two_rail_picks();
        self.log_sweep_two_rails();
    }

    fn start_sweep_two_rail_chain(&mut self, options: &str) {
        if !self.validate_sweep_two_rails() {
            return;
        }
        let session = self.sweep_two_rails.as_ref().unwrap();
        if !session.selecting() || session.rail_count() == 2 {
            self.push_log("Sweep2: ChainEdges is available while selecting rails".into());
            return;
        }
        let excluded = session
            .rail_targets()
            .chain(session.profile_ids().into_iter().map(Target::Curve))
            .collect::<Vec<_>>();
        self.begin_sweep_rail_chain(options, &excluded);
    }

    pub(in crate::app) fn accept_sweep_two_rail_chain(
        &mut self,
        path: &[(Target, bool)],
        gap: f64,
    ) -> bool {
        if self.sweep_two_rails.is_none() || !self.validate_sweep_two_rails() {
            return true;
        }
        let rail = match Rail::chain(&self.document, path, gap) {
            Ok(rail) => rail,
            Err(error) => {
                self.push_log(format!("Sweep2: {error}"));
                return false;
            }
        };
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.undo.push(session.state.clone());
        session.state.rails.push(rail);
        let last = session.rail_count() == 2;
        self.sync_sweep_two_rail_picks();
        if last {
            self.log_sweep_two_rails();
        } else {
            self.start_sweep_two_rail_chain("");
        }
        true
    }

    fn sync_sweep_two_rail_picks(&mut self) {
        let session = self.sweep_two_rails.as_mut().unwrap();
        session.preview.invalidate();
        let ids = session.ids();
        self.restore_curve_selection(ids);
    }

    pub(in crate::app) fn log_sweep_two_rails(&mut self) {
        let session = self.sweep_two_rails.as_ref().unwrap();
        let hint = session.hint();
        let fit = match session.state.shape.fit {
            SectionFit::Unchanged => "Cross sections unchanged".to_owned(),
            SectionFit::Rebuild(count) => format!("Rebuild={count}"),
            SectionFit::Refit(distance) => format!("Refit={distance}"),
        };
        let choice = |enabled| if enabled { "Yes" } else { "No" };
        let height = choice(session.state.shape.maintain_height);
        let rails = choice(session.state.shape.refit_rails);
        self.push_log(if session.state.review && session.value.is_none() {
            format!("Sweep2: {hint}. {fit}. MaintainHeight={height}. RefitRails={rails}")
        } else {
            format!("Sweep2: {hint}")
        });
    }

    pub(super) fn validate_sweep_two_rails(&mut self) -> bool {
        let Some(session) = &self.sweep_two_rails else {
            return false;
        };
        let current = session
            .options
            .explicit_rails(&self.document)
            .is_ok_and(|rails| rails == session.named_rails)
            && session
                .state
                .rails
                .iter()
                .all(|rail| rail.current(&self.document))
            && session
                .state
                .picks
                .iter()
                .all(|pick| pick.current(&self.document));
        if !current {
            self.cancel_object_prompt(false);
            self.push_log("Sweep2 sources changed. Select them again".into());
        }
        current
    }
}
