//! Persistent curvature analysis with shared, immutable differential samples.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{CurvatureColorMapping, CurvatureMesh, CurvatureStyle};
mod options;
#[cfg(test)]
mod tests;
pub use options::Options;
use options::{Action, USAGE, parse};

pub type Meshes = Arc<BTreeMap<ObjectId, Arc<CurvatureMesh>>>;
pub struct View {
    pub options: Options,
    pub objects: Arc<BTreeSet<ObjectId>>,
    pub meshes: Meshes,
    pub sources: usize,
    pub unavailable: usize,
}
impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurvatureAnalysisView")
            .field("options", &self.options)
            .field("sources", &self.sources)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}
struct Cache {
    source: GeometrySnapshot,
    tolerance: Tolerance,
    mesh: Option<Arc<CurvatureMesh>>,
}
#[derive(Default)]
struct State {
    enabled: bool,
    options: Options,
    objects: Arc<BTreeSet<ObjectId>>,
    cache: BTreeMap<ObjectId, Cache>,
    meshes: Meshes,
}
#[derive(Default)]
pub(crate) struct Session(Mutex<State>);
impl Session {
    pub(crate) fn remove(&self, ids: &BTreeSet<ObjectId>) {
        if ids.is_empty() {
            return;
        }
        let mut state = self.0.lock().unwrap();
        let objects = Arc::make_mut(&mut state.objects);
        for id in ids {
            objects.remove(id);
        }
    }

    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        let s = self.0.lock().unwrap();
        s.enabled && s.objects.contains(&id)
    }
    pub(crate) fn view(&self, doc: &Document) -> Option<View> {
        let mut state = self.0.lock().unwrap();
        if !state.enabled {
            return None;
        }
        refresh(&mut state, doc);
        Some(View {
            options: state.options,
            objects: state.objects.clone(),
            meshes: state.meshes.clone(),
            sources: state
                .objects
                .iter()
                .filter(|id| doc.object(**id).is_some())
                .count(),
            unavailable: state.cache.values().filter(|c| c.mesh.is_none()).count(),
        })
    }
}
fn refresh(state: &mut State, doc: &Document) {
    let mut meshes = BTreeMap::new();
    for id in state.objects.iter().copied() {
        let Some(object) = doc.object(id) else {
            continue;
        };
        if !object.attributes().is_visible()
            || !doc
                .layer(object.attributes().layer_id())
                .is_some_and(|l| l.is_visible())
        {
            continue;
        }
        if state.cache.get(&id).is_none_or(|c| {
            c.tolerance != doc.tolerance()
                || !c.source.shares_storage_with(object.geometry_snapshot())
        }) {
            let mesh = match object.geometry() {
                Geometry::NurbsSurface(s) => s.curvature_mesh(8, doc.tolerance()).ok(),
                Geometry::Brep(b) => b.curvature_mesh(8, doc.tolerance()).ok(),
                Geometry::SubD(s) => s.curvature_mesh().ok(),
                _ => None,
            }
            .map(Arc::new);
            state.cache.insert(
                id,
                Cache {
                    source: object.geometry_snapshot().clone(),
                    tolerance: doc.tolerance(),
                    mesh,
                },
            );
        }
        if let Some(mesh) = &state.cache[&id].mesh {
            meshes.insert(id, mesh.clone());
        }
    }
    state
        .cache
        .retain(|id, _| state.objects.contains(id) && doc.object(*id).is_some());
    if state.meshes.len() != meshes.len()
        || meshes
            .iter()
            .any(|(id, m)| state.meshes.get(id).is_none_or(|old| !Arc::ptr_eq(m, old)))
    {
        state.meshes = Arc::new(meshes);
    }
}

pub(crate) struct AnalysisCommand {
    pub session: Arc<Session>,
    pub draft: Arc<draft_angle_analysis::Session>,
    pub off: bool,
}
impl Command for AnalysisCommand {
    fn name(&self) -> &'static str {
        if self.off {
            "CurvatureAnalysisOff"
        } else {
            "CurvatureAnalysis"
        }
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if self.off {
            require_consumed(args, 0, USAGE)?;
            return Ok(None);
        }
        let action = parse(&mut Options::default(), args)?;
        let enabled = self.session.0.lock().unwrap().enabled;
        Ok(
            (matches!(action, Action::Start | Action::Add | Action::Remove) || !enabled).then(
                || ObjectSelectionPrompt {
                    command: "CurvatureAnalysis",
                    filter: ObjectSelectionFilter::CurvatureAnalysis,
                    options: vec![],
                    menus: vec![],
                    choices: vec![],
                    workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
                },
            ),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl AnalysisCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let mut state = self.session.0.lock().unwrap();
        if self.off {
            require_consumed(args, 0, USAGE)?;
            let options = state.options;
            *state = State {
                options,
                ..Default::default()
            };
            return Ok("Curvature analysis off".into());
        }
        let mut options = state.options;
        let action = parse(&mut options, args)?;
        let mut targets = BTreeSet::new();
        if matches!(action, Action::Start | Action::Add | Action::Remove) || !state.enabled {
            targets = doc
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::CurvatureAnalysis.accepts_object(o))
                .map(|o| o.id())
                .collect();
            if targets.is_empty() {
                return Err(CommandError::NoObjectsSelected);
            }
            let ids = Arc::make_mut(&mut state.objects);
            if action == Action::Remove {
                ids.retain(|id| !targets.contains(id));
            } else {
                ids.extend(targets.iter().copied());
            }
            if postselected {
                doc.clear_selection();
            }
        }
        state.enabled = true;
        state.options = options;
        if matches!(action, Action::MaxRange | Action::AutoRange) {
            refresh(&mut state, doc);
            let samples = state
                .meshes
                .values()
                .flat_map(|mesh| mesh.principal_samples());
            let range = if action == Action::AutoRange {
                options.style.automatic_range(samples)
            } else {
                Some(
                    samples
                        .filter_map(|k| options.style.statistics_value(k))
                        .fold([Real::INFINITY, Real::NEG_INFINITY], |r, k| {
                            [r[0].min(k), r[1].max(k)]
                        }),
                )
            };
            let i = state.options.index();
            if let Some(range) = range {
                state.options.ranges[i] = if range[0] < range[1] {
                    range
                } else {
                    Options::default().ranges[i]
                };
            }
        }
        let count = state.objects.len();
        drop(state);
        if action != Action::Remove {
            self.draft.remove(&targets);
        }
        Ok(format!("Curvature analysis on {count} object(s)"))
    }
}
