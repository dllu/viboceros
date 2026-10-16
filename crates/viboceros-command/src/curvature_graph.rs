//! Persistent, nonmutating curvature combs with source-snapshot caches.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{CurvatureComb, curvature_comb};

mod extract;
#[cfg(test)]
mod tests;
pub(super) use extract::ExtractCommand;

pub const USAGE: &str = "CurvatureGraph [Add|Remove] [Scale=0..300] [Density=0..10] [Count=2..64] [U=Yes|No] [V=Yes|No] [Color=r,g,b] [UColor=r,g,b] [VColor=r,g,b] | CurvatureGraphOff";
const MAX_SAMPLES: usize = 200_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Curve,
    U,
    V,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub scale: u16,
    pub density: u8,
    pub count: u8,
    pub u: bool,
    pub v: bool,
    pub color: [u8; 3],
    pub u_color: [u8; 3],
    pub v_color: [u8; 3],
}
impl Default for Options {
    fn default() -> Self {
        Self {
            scale: 100,
            density: 1,
            count: 3,
            u: true,
            v: true,
            color: [180, 60, 220],
            u_color: [230, 70, 70],
            v_color: [50, 180, 80],
        }
    }
}
impl Options {
    pub fn color(&self, kind: Kind) -> [u8; 3] {
        match kind {
            Kind::Curve => self.color,
            Kind::U => self.u_color,
            Kind::V => self.v_color,
        }
    }
    fn sampling(&self) -> (u8, u8, bool, bool) {
        (self.density, self.count, self.u, self.v)
    }
    pub fn factor(&self) -> Real {
        ((self.scale as Real - 100.) / 2.).exp2()
    }
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub object: ObjectId,
    pub kind: Kind,
    pub comb: Arc<CurvatureComb>,
}
#[derive(Clone, Debug)]
pub struct View {
    pub options: Options,
    pub graphs: Arc<[Graph]>,
    pub sources: usize,
    pub singular_samples: usize,
    factor: Real,
}
impl View {
    pub fn tip(&self, hair: &viboceros_geometry::CurvatureHair) -> Result<Point3, GeometryError> {
        hair.tip(self.factor)
    }
}
#[derive(Clone)]
struct Cache {
    snapshot: GeometrySnapshot,
    sampling: (u8, u8, bool, bool),
    tolerance: Tolerance,
    graphs: Arc<[Graph]>,
}
#[derive(Clone, Default)]
struct State {
    enabled: bool,
    options: Options,
    sources: BTreeSet<ObjectId>,
    cache: BTreeMap<ObjectId, Cache>,
    flat: Arc<[Graph]>,
    flat_sources: Vec<ObjectId>,
    validated_scale: Option<u16>,
}
#[derive(Default)]
pub(super) struct Session(Mutex<State>);

fn gather(
    geometry: &Geometry,
    object: ObjectId,
    options: &Options,
    tolerance: Tolerance,
) -> Result<Vec<Graph>, CommandError> {
    let mut graphs = Vec::new();
    let mut remaining = MAX_SAMPLES;
    let mut append = |curve: &NurbsCurve, kind| -> Result<(), CommandError> {
        let comb = curvature_comb(curve, 8usize << options.density, remaining)?;
        remaining -= comb.hairs.len() + comb.singular_samples;
        graphs.push(Graph {
            object,
            kind,
            comb: Arc::new(comb),
        });
        Ok(())
    };
    if let Some(curve) = geometry.curve_ref() {
        append(&curve.to_nurbs()?, Kind::Curve)?;
        return Ok(graphs);
    }
    let mut surface =
        |source: &NurbsSurface, face: Option<&BrepFace>| -> Result<(), CommandError> {
            for (enabled, direction, kind, domain) in [
                (
                    options.u,
                    SurfaceKnotDirection::U,
                    Kind::U,
                    source.domain_v(),
                ),
                (
                    options.v,
                    SurfaceKnotDirection::V,
                    Kind::V,
                    source.domain_u(),
                ),
            ] {
                if !enabled {
                    continue;
                }
                for i in 0..options.count {
                    let f = i as Real / (options.count - 1) as Real;
                    let fixed = if i == 0 {
                        *domain.start()
                    } else if i + 1 == options.count {
                        *domain.end()
                    } else {
                        domain.start() * (1. - f) + domain.end() * f
                    };
                    let pieces = if let Some(face) = face {
                        face.isocurve_segments_with_extractors(
                            direction,
                            fixed,
                            tolerance,
                            &mut surface_segments::isocurve,
                            &mut curve_segments::trim,
                        )?
                    } else {
                        vec![surface_segments::isocurve(source, direction, fixed)?]
                    };
                    for curve in pieces {
                        append(&curve, kind)?;
                    }
                }
            }
            Ok(())
        };
    match geometry {
        Geometry::NurbsSurface(s) => surface(s, None)?,
        Geometry::Brep(b) => {
            for face in b.faces() {
                surface(face.surface(), Some(face))?;
            }
        }
        _ => {
            return Err(CommandError::Usage(
                "CurvatureGraph requires curves, surfaces or polysurfaces",
            ));
        }
    }
    Ok(graphs)
}

impl Session {
    fn resolve(state: &mut State, doc: &Document) -> Result<View, CommandError> {
        Self::resolve_with_budget(state, doc, MAX_SAMPLES)
    }
    fn resolve_with_budget(
        state: &mut State,
        doc: &Document,
        maximum: usize,
    ) -> Result<View, CommandError> {
        state.sources.retain(|id| doc.object(*id).is_some());
        state.cache.retain(|id, _| state.sources.contains(id));
        let mut dirty = false;
        let mut ids = Vec::new();
        let mut total_samples = 0usize;
        for object in doc.objects().filter(|o| state.sources.contains(&o.id())) {
            ids.push(object.id());
            if state.cache.get(&object.id()).is_none_or(|c| {
                c.sampling != state.options.sampling()
                    || c.tolerance != doc.tolerance()
                    || !c.snapshot.shares_storage_with(object.geometry_snapshot())
            }) {
                // A later aggregate-budget error must not make this updated
                // cache appear compatible with the old flattened display.
                state.flat_sources.clear();
                let graphs = gather(
                    object.geometry(),
                    object.id(),
                    &state.options,
                    doc.tolerance(),
                )?;
                state.cache.insert(
                    object.id(),
                    Cache {
                        snapshot: object.geometry_snapshot().clone(),
                        sampling: state.options.sampling(),
                        tolerance: doc.tolerance(),
                        graphs: graphs.into(),
                    },
                );
                dirty = true;
            }
            for graph in state.cache[&object.id()].graphs.iter() {
                total_samples = total_samples
                    .saturating_add(graph.comb.hairs.len() + graph.comb.singular_samples);
                if total_samples > maximum {
                    return Err(GeometryError::CurvatureCombLimit.into());
                }
            }
        }
        if dirty || state.flat_sources != ids {
            let mut graphs = Vec::new();
            let mut samples = 0usize;
            for id in &ids {
                for graph in state.cache[id].graphs.iter() {
                    samples = samples
                        .saturating_add(graph.comb.hairs.len() + graph.comb.singular_samples);
                    if samples > maximum {
                        return Err(GeometryError::CurvatureCombLimit.into());
                    }
                    graphs.push(graph.clone());
                }
            }
            state.flat = graphs.into();
            state.flat_sources = ids;
            state.validated_scale = None;
        }
        let view = View {
            options: state.options.clone(),
            factor: state.options.factor(),
            graphs: state.flat.clone(),
            sources: state.flat_sources.len(),
            singular_samples: state.flat.iter().map(|g| g.comb.singular_samples).sum(),
        };
        // Validate display arithmetic before accepting an option or new source.
        if state.validated_scale != Some(state.options.scale) {
            for graph in view.graphs.iter() {
                for hair in &graph.comb.hairs {
                    view.tip(hair)?;
                }
            }
            state.validated_scale = Some(state.options.scale);
        }
        Ok(view)
    }
    pub(super) fn view(&self, doc: &Document) -> Result<Option<View>, CommandError> {
        let mut state = self.0.lock().expect("curvature graph state");
        if !state.enabled {
            return Ok(None);
        }
        Ok(Some(Self::resolve(&mut state, doc)?))
    }
}

pub(super) struct GraphCommand {
    pub session: Arc<Session>,
    pub off: bool,
}
fn parse(options: &mut Options, args: &[&str]) -> Result<Option<bool>, CommandError> {
    let mut action = None;
    let mut seen = BTreeSet::new();
    for arg in args {
        let arg = arg.trim_start_matches('_');
        if let Some((key, value)) = arg.split_once('=') {
            if !seen.insert(key.to_ascii_lowercase()) {
                return Err(CommandError::Usage(USAGE));
            }
            let number = || value.parse::<u16>().map_err(|_| CommandError::Usage(USAGE));
            let yes = || match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => Err(CommandError::Usage(USAGE)),
            };
            let color = || {
                value
                    .split(',')
                    .map(str::parse::<u8>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| CommandError::Usage(USAGE))?
                    .try_into()
                    .map_err(|_| CommandError::Usage(USAGE))
            };
            match key.to_ascii_lowercase().as_str() {
                "scale" => {
                    let n = number()?;
                    if n > 300 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    options.scale = n;
                }
                "density" => {
                    let n = number()?;
                    if n > 10 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    options.density = n as u8;
                }
                "count" => {
                    let n = number()?;
                    if !(2..=64).contains(&n) {
                        return Err(CommandError::Usage(USAGE));
                    }
                    options.count = n as u8;
                }
                "u" => options.u = yes()?,
                "v" => options.v = yes()?,
                "color" => options.color = color()?,
                "ucolor" => options.u_color = color()?,
                "vcolor" => options.v_color = color()?,
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else if action.is_none()
            && (arg.eq_ignore_ascii_case("Add") || arg.eq_ignore_ascii_case("Remove"))
        {
            action = Some(arg.eq_ignore_ascii_case("Add"));
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(action)
}
impl Command for GraphCommand {
    fn name(&self) -> &'static str {
        if self.off {
            "CurvatureGraphOff"
        } else {
            "CurvatureGraph"
        }
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
        Ok(
            (args.is_empty() || action.is_some() || !self.session.0.lock().unwrap().enabled).then(
                || ObjectSelectionPrompt {
                    command: self.name(),
                    filter: ObjectSelectionFilter::Parametric,
                    options: vec![],
                    menus: vec![],
                    choices: vec![],
                    workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
                },
            ),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut state = self.session.0.lock().expect("curvature graph state");
        if self.off {
            require_consumed(args, 0, USAGE)?;
            *state = State {
                options: state.options.clone(),
                ..State::default()
            };
            return Ok("Curvature graphs off".into());
        }
        let mut proposed = state.clone();
        let action = parse(&mut proposed.options, args)?;
        let selected = doc
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Parametric.accepts_object(o))
            .map(|o| o.id())
            .collect::<BTreeSet<_>>();
        if action.is_some() && selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        match action {
            Some(true) => proposed.sources.extend(selected),
            Some(false) => proposed.sources.retain(|id| !selected.contains(id)),
            None if !state.enabled || args.is_empty() => {
                if selected.is_empty() {
                    return Err(CommandError::NoObjectsSelected);
                }
                proposed.sources = selected;
            }
            _ => {}
        }
        proposed.enabled = true;
        let view = Session::resolve(&mut proposed, doc)?;
        let message = format!(
            "Curvature graphs: {} curves on {} objects; {} singular samples omitted",
            view.graphs.len(),
            view.sources,
            view.singular_samples
        );
        *state = proposed;
        Ok(message)
    }
}
