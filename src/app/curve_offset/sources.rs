//! Collect whole curves, surface edges and temporary portions in pick order.
use super::*;
use crate::app::object_selection::PendingObjectCommand;
use crate::viewport::{ComponentPick, EdgePick};
use std::collections::{BTreeMap, BTreeSet};
use viboceros_command::{
    ComponentSelectionKind,
    component_selection::ComponentAction,
    curve_reference::{CapturedCurve, parse_edge},
    offset_curve::SourceOwner,
};
use viboceros_document::GeometrySnapshot;

pub(super) struct Source {
    pub(super) object: ObjectId,
    pub(super) snapshot: GeometrySnapshot,
    pub(super) edge: Option<usize>,
    pub(super) whole: bool,
    portion: bool,
    guide: Option<NurbsCurve>,
}

pub(super) fn all_current<'a>(
    document: &Document,
    mut sources: impl Clone + Iterator<Item = &'a Source>,
) -> bool {
    if sources.clone().take(17).count() <= 16 {
        return sources.all(|source| source.current(document));
    }
    let wanted = sources
        .clone()
        .map(|source| source.object)
        .collect::<BTreeSet<_>>();
    let found = document
        .selectable_objects()
        .filter(|object| wanted.contains(&object.id()))
        .map(|object| (object.id(), object.geometry_snapshot()))
        .collect::<BTreeMap<_, _>>();
    sources.all(|source| {
        found
            .get(&source.object)
            .is_some_and(|snapshot| source.snapshot.shares_storage_with(snapshot))
    })
}

impl Source {
    pub(super) fn owner(&self) -> SourceOwner {
        SourceOwner {
            object: self.object,
            replaceable_curve: self.whole,
            inherit_layer: !self.portion,
        }
    }

    pub(super) fn current(&self, document: &Document) -> bool {
        document.is_object_selectable(self.object)
            && document.object(self.object).is_some_and(|object| {
                self.snapshot
                    .shares_storage_with(object.geometry_snapshot())
            })
    }

    fn capture(reference: &CapturedCurve) -> Self {
        let (snapshot, edge) = reference.source();
        Self {
            object: reference.object_id(),
            snapshot: snapshot.clone(),
            edge,
            whole: false,
            portion: false,
            guide: None,
        }
    }
}

pub(super) struct Input {
    source: Source,
    curve: Curve3,
}

impl Input {
    fn whole(object: &viboceros_document::Object) -> Option<Self> {
        let curve = object.geometry().curve_ref()?.to_owned();
        Some(Self {
            source: Source {
                object: object.id(),
                snapshot: object.geometry_snapshot().clone(),
                edge: None,
                whole: true,
                portion: false,
                guide: None,
            },
            curve,
        })
    }

    fn edge(reference: &CapturedCurve) -> Self {
        Self {
            source: Source::capture(reference),
            curve: reference.curve_ref().to_owned(),
        }
    }
}

impl VibocerosApp {
    pub(in crate::app) fn start_curve_offset(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let Some(kind) = words.next().and_then(Kind::from_name) else {
            return false;
        };
        let words = words.collect::<Vec<_>>();
        if solver::is_positional(kind, &words) {
            return false;
        }
        let subcurve = words.iter().any(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubCrv")
        });
        let options = words
            .iter()
            .copied()
            .filter(|word| {
                !word
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("SubCrv")
            })
            .collect::<Vec<_>>();
        let Ok((settings, menu)) = solver::parse_settings(
            kind,
            &options,
            self.commands.offset_settings(kind),
            self.document.units(),
        ) else {
            return false;
        };

        let mut inputs = self
            .document
            .selected_objects()
            .filter_map(Input::whole)
            .collect::<Vec<_>>();
        for pick in self.component_selection.valid_picks(&self.document) {
            if pick.kind == ComponentSelectionKind::BrepEdge {
                match CapturedCurve::capture(&self.document, pick.object, Some(pick.index)) {
                    Ok(reference) => inputs.push(Input::edge(&reference)),
                    Err(error) => {
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                }
            }
        }
        let reference = if subcurve && let [input] = inputs.as_slice() {
            Some((input.source.object, input.source.edge))
        } else {
            None
        };
        let postselected =
            inputs.is_empty() || (subcurve && kind == Kind::Offset && reference.is_none());
        let selecting = postselected || subcurve;
        let plane = self.viewports[self.active_viewport].construction_plane();
        self.cancel_interactive_command(false);
        if postselected {
            self.document.clear_selection();
        }
        let sources = if subcurve { Vec::new() } else { inputs };
        self.curve_offset_session = Some(Session {
            postselected,
            selecting: Some(sources),
            sources: Vec::new(),
            owners: Vec::new(),
            portions: Vec::new(),
            curves: Arc::default(),
            kind,
            settings,
            plane,
            menu,
            distance_start: None,
            analytic: false,
            rendered: None,
            worker: None,
        });
        self.drafting_plane = Some(plane);
        self.push_log(format!("> {input}"));
        if selecting {
            let prompt = self
                .commands
                .object_selection_prompt(kind.name())
                .unwrap()
                .unwrap();
            let mut pending = PendingObjectCommand::selecting(prompt);
            pending.postselected = postselected;
            self.object_prompt = Some(pending);
            if subcurve {
                self.begin_offset_subcurve_prompt(reference);
            } else {
                self.curve_offset_prompt();
            }
        } else {
            self.finish_curve_offset_sources();
        }
        true
    }

    pub(in crate::app) fn picking_curve_offset_edges(&self) -> bool {
        self.subcurve_prompt.is_none()
            && self
                .curve_offset_session
                .as_ref()
                .is_some_and(|s| s.selecting.is_some() && s.menu.is_none())
    }

    pub(in crate::app) fn curve_offset_distance_input(&self) -> bool {
        self.curve_offset_session
            .as_ref()
            .is_some_and(|s| s.selecting.is_some() && s.menu == Some(OptionName::Distance))
    }

    pub(in crate::app) fn record_curve_offset_whole_sources(&mut self) {
        let Some(inputs) = self
            .curve_offset_session
            .as_mut()
            .and_then(|s| s.selecting.as_mut())
        else {
            return;
        };
        if self.subcurve_prompt.is_some() {
            return;
        }
        let selected: BTreeSet<_> = self.document.selected_object_ids().collect();
        inputs.retain(|input| !input.source.whole || selected.contains(&input.source.object));
        let existing = inputs
            .iter()
            .filter(|input| input.source.edge.is_none())
            .map(|input| input.source.object)
            .collect::<BTreeSet<_>>();
        inputs.extend(
            self.document
                .selected_objects()
                .filter(|object| !existing.contains(&object.id()))
                .filter_map(Input::whole),
        );
    }

    pub(super) fn continue_curve_offset_sources(&mut self, text: &str, enter: bool) -> bool {
        if enter {
            self.record_curve_offset_whole_sources();
            self.finish_curve_offset_sources();
            return true;
        }
        let token = text.trim_start_matches(['_', '-']);
        if token.eq_ignore_ascii_case("SubCrv") {
            self.begin_offset_subcurve_prompt(None);
            return true;
        }
        if let Some((key, value)) = token.split_once('=')
            && key.eq_ignore_ascii_case("Edge")
        {
            if let Some((object, edge)) = parse_edge(value) {
                self.pick_curve_offset_edges([EdgePick { object, edge }], ComponentAction::Add);
            } else {
                self.push_log("Enter Edge=object-id,edge-index".into());
            }
            return true;
        }
        if token.eq_ignore_ascii_case("SelNone") {
            self.document.clear_selection();
            self.curve_offset_session
                .as_mut()
                .unwrap()
                .selecting
                .as_mut()
                .unwrap()
                .clear();
            self.curve_offset_session.as_mut().unwrap().portions.clear();
            self.curve_offset_prompt();
            return true;
        }
        if token.eq_ignore_ascii_case("SelLast") {
            let id = self
                .document
                .objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .filter(|o| self.document.is_object_selectable(o.id()))
                .last()
                .map(|o| o.id());
            self.select_prompt_objects(id, SelectionMode::Add);
            return true;
        }
        if let Some((key, value)) = token.split_once(char::is_whitespace)
            && key.eq_ignore_ascii_case("SelID")
        {
            match value.trim().parse::<ObjectId>() {
                Ok(id) => self.select_prompt_objects([id], SelectionMode::Add),
                Err(_) => self.push_log("SelID requires one object ID".into()),
            }
            return true;
        }
        false
    }

    pub(in crate::app) fn pick_curve_offset_edges(
        &mut self,
        picks: impl IntoIterator<Item = EdgePick>,
        action: ComponentAction,
    ) {
        self.record_curve_offset_whole_sources();
        for pick in picks {
            let inputs = self
                .curve_offset_session
                .as_mut()
                .unwrap()
                .selecting
                .as_mut()
                .unwrap();
            let before = inputs.iter().position(|input| {
                input.source.object == pick.object && input.source.edge == Some(pick.edge)
            });
            if let Some(index) = before {
                if matches!(action, ComponentAction::Remove | ComponentAction::Toggle) {
                    inputs.remove(index);
                }
            } else if !matches!(action, ComponentAction::Remove) {
                match CapturedCurve::capture(&self.document, pick.object, Some(pick.edge)) {
                    Ok(reference) => inputs.push(Input::edge(&reference)),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
        }
        self.sync_offset_portion_guides();
        let session = self.curve_offset_session.as_ref().unwrap();
        if session.kind == Kind::Offset && !session.selecting.as_ref().unwrap().is_empty() {
            self.finish_curve_offset_sources();
        } else {
            self.curve_offset_prompt();
        }
    }

    pub(in crate::app) fn add_curve_offset_portion(
        &mut self,
        reference: &CapturedCurve,
        curve: Curve3,
    ) {
        let session = self.curve_offset_session.as_mut().unwrap();
        let inputs = session.selecting.as_mut().unwrap();
        let mut source = Source::capture(reference);
        source.portion = true;
        source.guide = curve.as_ref().to_nurbs().ok();
        if let Some(input) = inputs.iter_mut().find(|input| {
            input.source.object == source.object
                && input.source.edge == source.edge
                && input.source.whole
        }) {
            *input = Input { source, curve };
        } else {
            inputs.push(Input { source, curve });
        }
        self.sync_offset_portion_guides();
        self.finish_offset_subcurve_input();
        if self.curve_offset_session.as_ref().unwrap().kind == Kind::Offset {
            self.finish_curve_offset_sources();
        } else {
            self.curve_offset_prompt();
        }
    }

    pub(in crate::app) fn finish_offset_subcurve_input(&mut self) {
        self.subcurve_prompt = None;
        self.active_command = None;
        self.point_filter = None;
        self.point_constraint = None;
    }

    pub(in crate::app) fn return_to_curve_offset_sources(&mut self) {
        self.finish_offset_subcurve_input();
        self.curve_offset_prompt();
    }

    fn finish_curve_offset_sources(&mut self) {
        let session = self.curve_offset_session.as_mut().unwrap();
        let Some(inputs) = session.selecting.as_ref() else {
            return;
        };
        if inputs.is_empty() {
            self.curve_offset_prompt();
            return;
        }
        if !all_current(&self.document, inputs.iter().map(|input| &input.source)) {
            self.cancel_interactive_command(false);
            self.push_log("Offset sources changed. Select the curves again.".into());
            return;
        }
        let (sources, curves): (Vec<_>, Vec<_>) = session
            .selecting
            .take()
            .unwrap()
            .into_iter()
            .map(|input| (input.source, input.curve))
            .unzip();
        let (owners, curves) = match prepare_offset_inputs(
            session.kind,
            &sources,
            curves,
            self.document.tolerance(),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        if session.kind == Kind::Offset && curves.len() > 1 {
            self.document.clear_selection();
            let owner_ids = sources
                .iter()
                .filter(|source| source.edge.is_some())
                .map(|source| source.object)
                .collect::<Vec<_>>();
            self.component_selection.release_objects(&owner_ids);
            session.postselected = true;
            session.selecting = Some(Vec::new());
            session.portions.clear();
            let prompt = self
                .commands
                .object_selection_prompt(Kind::Offset.name())
                .unwrap()
                .unwrap();
            self.object_prompt = Some(PendingObjectCommand::selecting(prompt));
            self.curve_offset_prompt();
            return;
        }
        session.analytic = curves.len() <= 8
            && curves
                .iter()
                .all(|curve| matches!(curve, Curve3::Line(_) | Curve3::Circle(_) | Curve3::Arc(_)));
        if session.postselected {
            self.document.mark_object_selection_temporary(
                sources
                    .iter()
                    .filter(|source| source.whole)
                    .map(|source| source.object),
            );
        }
        session.sources = sources;
        session.owners = owners;
        session.curves = Arc::new(curves);
        self.object_prompt = None;
        self.curve_offset_prompt();
    }

    fn sync_offset_portion_guides(&mut self) {
        let session = self.curve_offset_session.as_mut().unwrap();
        session.portions = session
            .selecting
            .iter()
            .flatten()
            .filter_map(|input| input.source.guide.clone())
            .collect();
    }

    pub(in crate::app) fn curve_offset_edge_highlights(
        &self,
    ) -> impl Iterator<Item = ComponentPick> + '_ {
        self.curve_offset_session
            .iter()
            .flat_map(|session| {
                session.sources.iter().chain(
                    session
                        .selecting
                        .iter()
                        .flatten()
                        .map(|input| &input.source),
                )
            })
            .filter(|source| !source.portion)
            .filter_map(|source| {
                Some(ComponentPick {
                    object: source.object,
                    index: source.edge?,
                    kind: ComponentSelectionKind::BrepEdge,
                })
            })
    }
}

// Native single Offset joins connected preselected curve and edge segments
// before choosing a side. OffsetMultiple offsets each selected input.
fn prepare_offset_inputs(
    kind: Kind,
    sources: &[Source],
    curves: Vec<Curve3>,
    tolerance: Tolerance,
) -> Result<(Vec<SourceOwner>, Vec<Curve3>), viboceros_geometry::GeometryError> {
    if kind == Kind::Multiple || curves.len() < 2 {
        return Ok((sources.iter().map(Source::owner).collect(), curves));
    }
    use viboceros_geometry::{CurveJoinOptions, CurveJoinStyle, join_curves};
    let joined = join_curves(
        &curves,
        CurveJoinOptions {
            tolerance: tolerance.absolute(),
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        tolerance,
    )?;
    Ok(joined
        .into_iter()
        .map(|joined| {
            let members = joined.source_indices();
            let mut owner = sources[members[0]].owner();
            owner.replaceable_curve &= members.len() == 1;
            (owner, joined.into_curve())
        })
        .unzip())
}
