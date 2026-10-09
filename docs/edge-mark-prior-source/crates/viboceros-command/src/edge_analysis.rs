//! Non-destructive edge analysis state, shared by console and viewport UI.
use super::*;
#[cfg(test)]
mod tests;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    All,
    #[default]
    Naked,
    NonManifold,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Naked => "Naked",
            Self::NonManifold => "NonManifold",
        }
    }
    fn accepts(self, edge: &Edge) -> bool {
        match self {
            Self::All => edge.all,
            Self::Naked => edge.naked,
            Self::NonManifold => edge.non_manifold,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub object: ObjectId,
    pub index: usize,
    pub curve: NurbsCurve,
    pub endpoints: [Point3; 2],
    pub all: bool,
    pub naked: bool,
    pub non_manifold: bool,
}
#[derive(Clone, Debug)]
pub struct View {
    pub mode: Mode,
    pub color: [u8; 3],
    pub edges: Arc<[Edge]>,
    pub sources: usize,
    pub current: Option<usize>,
    pub focus_all: bool,
    pub zoom_requested: bool,
}
impl View {
    pub fn displayed(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(|e| self.mode.accepts(e))
    }
    pub fn focused(&self) -> Vec<&Edge> {
        let edges = self.displayed().collect::<Vec<_>>();
        if self.focus_all {
            edges
        } else {
            self.current
                .and_then(|i| edges.get(i).copied())
                .into_iter()
                .collect()
        }
    }
    pub fn bounds(&self) -> Option<BoundingBox3> {
        self.focused()
            .into_iter()
            .map(|e| e.curve.control_point_bounds())
            .reduce(|a, b| a.union(b).expect("finite edge bounds"))
    }
}
#[derive(Clone, Debug)]
struct Cache {
    snapshot: GeometrySnapshot,
    tolerance: Tolerance,
    edges: Arc<[Edge]>,
}
#[derive(Clone, Debug)]
struct State {
    enabled: bool,
    mode: Mode,
    color: [u8; 3],
    sources: BTreeSet<ObjectId>,
    cache: BTreeMap<ObjectId, Cache>,
    flat: Arc<[Edge]>,
    flat_sources: Vec<ObjectId>,
    current: Option<usize>,
    focus_all: bool,
    zoom_requested: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Mode::Naked,
            color: [255, 0, 255],
            sources: BTreeSet::new(),
            cache: BTreeMap::new(),
            flat: Arc::from([]),
            flat_sources: Vec::new(),
            current: None,
            focus_all: true,
            zoom_requested: false,
        }
    }
}
#[derive(Default)]
pub(super) struct Session(Mutex<State>);
const MAX_EDGES: usize = 1_000_000;
pub const USAGE: &str = "ShowEdges [Show=All|Naked|NonManifold] [Color=r,g,b] [Add|Remove] [Zoom All|Current|Next|Previous|Mark] | ShowEdgesOff | ZoomNaked | ZoomNonManifold";
fn gather(
    geometry: &Geometry,
    object: ObjectId,
    tolerance: Tolerance,
) -> Result<Vec<Edge>, CommandError> {
    match geometry {
        Geometry::Mesh(mesh) => mesh
            .diagnostic_edges()
            .into_iter()
            .map(|e| {
                let curve = LineSegment::try_new(
                    e.points[0],
                    e.points[1],
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .to_nurbs()?;
                Ok(Edge {
                    object,
                    index: e.index,
                    curve,
                    endpoints: e.points,
                    all: e.face_count == 1 || e.unwelded,
                    naked: e.face_count == 1,
                    non_manifold: e.face_count > 2,
                })
            })
            .collect(),
        Geometry::NurbsSurface(surface) => gather(
            &Geometry::Brep(Brep::try_surface_face_with_native_edge_parameters(
                surface.clone(),
                tolerance,
            )?),
            object,
            tolerance,
        ),
        Geometry::Brep(brep) => {
            let counts = brep.edge_use_counts();
            brep.edges()
                .iter()
                .enumerate()
                .map(|(index, e)| {
                    let curve = e.curve().clone();
                    let domain = curve.domain();
                    let endpoints = [
                        curve.evaluate(*domain.start())?,
                        curve.evaluate(*domain.end())?,
                    ];
                    Ok(Edge {
                        object,
                        index,
                        curve,
                        endpoints,
                        all: true,
                        naked: counts[index] == 1,
                        non_manifold: counts[index] > 2,
                    })
                })
                .collect()
        }
        _ => Err(CommandError::Usage(
            "ShowEdges requires surfaces, polysurfaces or meshes",
        )),
    }
}
impl Session {
    fn resolve(state: &mut State, doc: &Document) -> Result<View, CommandError> {
        state.sources.retain(|id| doc.object(*id).is_some());
        state.cache.retain(|id, _| state.sources.contains(id));
        let mut dirty = false;
        let mut ids = Vec::new();
        for object in doc.objects().filter(|o| state.sources.contains(&o.id())) {
            ids.push(object.id());
            let stale = state.cache.get(&object.id()).is_none_or(|c| {
                c.tolerance != doc.tolerance()
                    || !c.snapshot.shares_storage_with(object.geometry_snapshot())
            });
            if stale {
                dirty = true;
                let value = gather(object.geometry(), object.id(), doc.tolerance())?;
                state.cache.insert(
                    object.id(),
                    Cache {
                        snapshot: object.geometry_snapshot().clone(),
                        tolerance: doc.tolerance(),
                        edges: value.into(),
                    },
                );
            }
        }
        if dirty || state.flat_sources != ids {
            let mut edges = Vec::new();
            for id in &ids {
                if edges.len().saturating_add(state.cache[id].edges.len()) > MAX_EDGES {
                    return Err(CommandError::Usage("edge analysis exceeds 1000000 edges"));
                }
                edges.extend(state.cache[id].edges.iter().cloned());
            }
            state.flat = edges.into();
            state.flat_sources = ids;
        }
        let count = state.flat.iter().filter(|e| state.mode.accepts(e)).count();
        state.current = if count == 0 {
            None
        } else {
            Some(state.current.unwrap_or(0).min(count - 1))
        };
        Ok(View {
            mode: state.mode,
            color: state.color,
            edges: state.flat.clone(),
            sources: state.sources.len(),
            current: state.current,
            focus_all: state.focus_all,
            zoom_requested: state.zoom_requested,
        })
    }
    pub(super) fn acknowledge_zoom(&self) {
        self.0.lock().expect("edge analysis state").zoom_requested = false;
    }
    pub(super) fn view(&self, doc: &Document) -> Result<Option<View>, CommandError> {
        let mut state = self.0.lock().expect("edge analysis state");
        if !state.enabled {
            return Ok(None);
        }
        Ok(Some(Self::resolve(&mut state, doc)?))
    }
}
pub(super) struct AnalysisCommand {
    pub session: Arc<Session>,
    pub name: &'static str,
}
impl Command for AnalysisCommand {
    fn name(&self) -> &'static str {
        self.name
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(
            (self.name != "ShowEdgesOff" && args.is_empty()).then(|| ObjectSelectionPrompt {
                command: self.name,
                filter: ObjectSelectionFilter::EdgeAnalysis,
                options: Vec::new(),
                menus: Vec::new(),
                choices: Vec::new(),
                workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            }),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut proposed = self.session.0.lock().expect("edge analysis state").clone();
        if self.name == "ShowEdgesOff" {
            require_consumed(args, 0, USAGE)?;
            proposed.enabled = false;
            proposed.sources.clear();
            proposed.cache.clear();
            proposed.flat = Arc::from([]);
            proposed.flat_sources.clear();
            *self.session.0.lock().unwrap() = proposed;
            return Ok("Edge display off".into());
        }
        if self.name == "ZoomNaked" {
            proposed.mode = Mode::Naked;
            proposed.focus_all = false;
        }
        if self.name == "ZoomNonManifold" {
            proposed.mode = Mode::NonManifold;
            proposed.focus_all = false;
        }
        let mut action = "replace";
        let mut navigation = None;
        let mut mark = false;
        let mut seen = BTreeSet::new();
        for arg in args {
            let arg = arg.trim_start_matches('_');
            if let Some((key, value)) = arg.split_once('=') {
                if !seen.insert(key.to_ascii_lowercase()) {
                    return Err(CommandError::Usage(USAGE));
                }
                match key.to_ascii_lowercase().as_str() {
                    "show" => {
                        proposed.mode =
                            match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                                "all" => Mode::All,
                                "naked" => Mode::Naked,
                                "nonmanifold" | "non-manifold" => Mode::NonManifold,
                                _ => return Err(CommandError::Usage(USAGE)),
                            }
                    }
                    "color" => {
                        let values = value
                            .split(',')
                            .map(str::parse::<u8>)
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| CommandError::Usage(USAGE))?;
                        proposed.color =
                            values.try_into().map_err(|_| CommandError::Usage(USAGE))?;
                    }
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            } else {
                match arg.to_ascii_lowercase().as_str() {
                    "add" => {
                        if action != "replace" {
                            return Err(CommandError::Usage(USAGE));
                        }
                        action = "add";
                    }
                    "remove" => {
                        if action != "replace" {
                            return Err(CommandError::Usage(USAGE));
                        }
                        action = "remove";
                    }
                    "zoom" => {}
                    "all" | "current" | "next" | "previous" => {
                        if navigation.replace(arg.to_ascii_lowercase()).is_some() {
                            return Err(CommandError::Usage(USAGE));
                        }
                    }
                    "mark" => mark = true,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
        }
        let selected = doc
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::EdgeAnalysis.accepts_object(o))
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        if action != "replace" && selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if action == "remove" {
            for id in selected {
                proposed.sources.remove(&id);
            }
        } else if action == "add" {
            proposed.sources.extend(selected);
        } else if !proposed.enabled || args.is_empty() || self.name.starts_with("Zoom") {
            if !selected.is_empty() {
                proposed.sources = selected;
            } else if proposed.sources.is_empty() {
                return Err(CommandError::NoObjectsSelected);
            }
        }
        proposed.zoom_requested = self.name.starts_with("Zoom") || navigation.is_some();
        proposed.enabled = true;
        let mut view = Session::resolve(&mut proposed, doc)?;
        let count = view.displayed().count();
        if let Some(nav) = navigation {
            match nav.as_str() {
                "all" => proposed.focus_all = true,
                "current" => proposed.focus_all = false,
                "next" => {
                    proposed.focus_all = false;
                    if count > 0 {
                        proposed.current = Some((proposed.current.unwrap_or(0) + 1) % count);
                    }
                }
                "previous" => {
                    proposed.focus_all = false;
                    if count > 0 {
                        proposed.current =
                            Some((proposed.current.unwrap_or(0) + count - 1) % count);
                    }
                }
                _ => unreachable!(),
            };
            view = Session::resolve(&mut proposed, doc)?;
        }
        if mark {
            let mut points = BTreeMap::new();
            for edge in view.focused() {
                for p in edge.endpoints {
                    points
                        .entry(p.to_array().map(|v| if v == 0. { 0 } else { v.to_bits() }))
                        .or_insert(p);
                }
            }
            for point in points.into_values() {
                doc.add_geometry(Geometry::Point(point))?;
            }
        }
        *self.session.0.lock().expect("edge analysis state") = proposed;
        Ok(format!(
            "Edge analysis: {} {} edges on {} objects",
            count,
            view.mode.label(),
            view.sources
        ))
    }
}
